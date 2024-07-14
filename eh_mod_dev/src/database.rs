use std::any::Any;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::{Debug, Formatter};
use std::ops::{DerefMut, Range};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use flate2::Compression;
use parking_lot::{Mutex, RwLock};
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use tracing::{error, error_span, info};

use diagnostic::context::DiagnosticContext;
use eh_schema::schema::{DatabaseItem, DatabaseItemId, DatabaseSettings, Item};

use crate::builder::{ModBuilderData, ModBuilderInfo};
pub use crate::database::db_item::DbItem;
pub use crate::database::iters::{DatabaseItemIter, DatabaseItemIterMut};
pub use crate::database::stored_db_item::StoredDbItem;
pub use crate::mapping::DatabaseIdLike;
use crate::mapping::{IdMapping, IdMappingSerialized, KindProvider};
use crate::utils::{compress, decompress, sha256};

pub mod db_item;
pub mod iters;
pub mod stored_db_item;

mod macro_impls;

pub fn database(
    output_path: impl AsRef<Path>,
    output_mod_file_path: Option<impl AsRef<Path>>,
) -> Database {
    DatabaseHolder::new(
        output_path.as_ref().to_path_buf(),
        output_mod_file_path.map(|p| p.as_ref().to_path_buf()),
    )
}

const MAPPINGS_NAME: &str = "id_mappings.json5";
const MAPPINGS_BACKUP_NAME: &str = "id_mappings.json5.backup";
const HASHES_NAME: &str = ".hashes";

pub type Database = Arc<DatabaseHolder>;

pub struct DatabaseHolder {
    inner: Mutex<DatabaseInner>,
}

impl Debug for DatabaseHolder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let lock = self.inner.lock();
        f.debug_struct("DatabaseHolder")
            .field("path", &lock.output_path)
            .finish()
    }
}

type SharedItem = Arc<RwLock<Item>>;
type ItemsMap = Arc<RwLock<HashMap<Option<i32>, SharedItem>>>;

pub struct DatabaseInner {
    output_path: PathBuf,
    output_file_path: Option<PathBuf>,
    ids: IdMapping,
    other_ids: HashMap<Cow<'static, str>, Arc<RwLock<IdMapping>>>,
    items: HashMap<&'static str, ItemsMap>,
    // items: Vec<Item>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct MappingsSerde {
    ids: IdMappingSerialized,
    #[serde(flatten)]
    others: BTreeMap<Cow<'static, str>, IdMappingSerialized>,
}

fn check_no_backup(path: &Path) {
    let _guard =
        error_span!("Checking for mapping backup file presence", path=%path.display()).entered();
    if path.exists() {
        panic!("Mappings backup file exists, this means that there was an error during the previous invocation, please manually check your ID files to avoid data corruption. Remove the file after data integrity is ensured.")
    }
}

impl DatabaseHolder {
    /// Constructs a new database builder. Don't forget to allocate ID space
    /// via [add_id_range] or [add_id_range_for] methods
    ///
    /// # Panics
    /// Will panic if output path contains a mappings file but it can't be read or invalid
    ///
    /// Will panic if mappings backup exists
    pub fn new(output_path: PathBuf, output_mod_file_path: Option<PathBuf>) -> Database {
        let cur_dir = std::env::current_dir()
            .expect("Should be able to get current directory info from process env");
        let output_path = cur_dir.join(output_path);

        let output_mod_file_path = output_mod_file_path.map(|data| cur_dir.join(data));

        let _guard =
            error_span!("Creating a new database", output_path=%output_path.display()).entered();
        if !output_path.exists() {
            panic!("Target directory does not exist")
        }

        let mappings_path = output_path.join(MAPPINGS_NAME);
        let mappings: MappingsSerde = mappings_path
            .exists()
            .then(|| {
                let data = fs_err::read_to_string(output_path.join(MAPPINGS_NAME))
                    .expect("Should be able to read mappings file");
                serde_json5::from_str(&data).expect("Should be able to deserialize mappings file")
            })
            .unwrap_or_default();

        check_no_backup(&output_path.join(MAPPINGS_BACKUP_NAME));
        let other_ids = mappings
            .others
            .into_iter()
            .map(|(kind, ids)| (kind, Arc::new(RwLock::new(IdMapping::new(ids)))))
            .collect();

        let db = Self {
            inner: Mutex::new(DatabaseInner {
                output_path,
                output_file_path: output_mod_file_path,
                ids: IdMapping::new(mappings.ids),
                other_ids,
                items: Default::default(),
            }),
        };
        Arc::new(db)
    }

    /// Adds another ID range to use for all types
    pub fn add_id_range(&self, range: Range<i32>) {
        self.lock(|db| db.ids.add_id_range(range));
    }

    /// Adds another ID range to use for one specified type
    pub fn add_id_range_for<T: 'static + DatabaseItem>(&self, range: Range<i32>) {
        self.lock(|db| db.ids.add_id_range_for(T::type_name(), range));
    }

    /// Clears allocated ID ranges for the specified type, preventing obtaining
    /// of new IDs until [add_id_range] or [add_id_range_for] are used to
    /// allocate new ID space for this type
    pub fn clear_id_ranges_for<T: 'static + DatabaseItem>(&mut self) {
        self.lock(|db| db.ids.clear_id_ranges_for(T::type_name()));
    }

    /// Converts string ID into database item ID
    ///
    /// Aborts the execution if generating ID is not possible
    pub fn id<T: 'static + DatabaseItem>(&self, id: impl DatabaseIdLike<T>) -> DatabaseItemId<T> {
        DatabaseItemId::new(self.lock(|db| id.into_id(&db.ids)))
    }

    /// Converts string ID into new database item ID
    ///
    /// Aborts the execution if generating ID is not possible
    pub fn new_id<T: 'static + DatabaseItem>(
        &self,
        id: impl DatabaseIdLike<T>,
    ) -> DatabaseItemId<T> {
        DatabaseItemId::new(self.lock(|db| id.into_new_id(&mut db.ids)))
    }

    pub fn get_id_raw<T: 'static + DatabaseItem>(
        &self,
        id: impl Into<String>,
    ) -> DatabaseItemId<T> {
        DatabaseItemId::new(self.lock(|db| db.ids.get_id_raw(T::type_name(), id)))
    }

    /// Forcefully assigns numeric ID to a string
    pub fn set_id<T: 'static + DatabaseItem>(
        &self,
        string_id: impl Into<String>,
        numeric_id: i32,
    ) -> DatabaseItemId<T> {
        DatabaseItemId::new(self.lock(|db| db.ids.set_id(T::type_name(), string_id, numeric_id)))
    }

    pub fn get_id_name<T: 'static + DatabaseItem>(&self, id: DatabaseItemId<T>) -> Option<String> {
        self.lock(|db| db.ids.get_inverse_id(T::type_name(), id.0))
    }

    /// Adds an item to the database, returns a mutable handle to the inserted item
    ///
    /// All returned handles **must** be dropped before saving the database, otherwise a panic will occur.
    ///
    /// # Panics
    /// All items are stored behind a [Mutex], so regular runtime borrowing rules apply
    pub fn add_item<T: Into<Item> + DatabaseItem>(self: &Arc<Self>, item: T) -> DbItem<T> {
        DbItem::new(item, self.clone())
    }

    pub fn get_mappings<T: KindProvider>(&self) -> Arc<RwLock<IdMapping>> {
        self.lock(|db| db.other_ids.entry(T::kind()).or_default().clone())
    }

    /// Gets the item that was saved to the database previously
    ///
    /// All returned handles **must** be dropped before saving the database, otherwise a panic will occur.
    ///
    /// # Panics
    /// Each item is individually stored behind a [RwLock], so regular runtime borrowing rules apply
    pub fn get_item<T: Into<Item> + DatabaseItem + Any>(
        self: &Arc<Self>,
        id: impl DatabaseIdLike<T>,
    ) -> Option<StoredDbItem<T>> {
        let mut db = self.inner.lock();
        let db = db.deref_mut();
        let id = id.into_id(&db.ids);

        let item = db
            .items
            .get_mut(T::type_name())
            .and_then(|i| i.read().get(&Some(id)).cloned())
            .map(|i| StoredDbItem::new(i, self.clone()));

        item
    }

    pub fn get_singleton<T: Into<Item> + DatabaseItem + Any>(
        self: &Arc<Self>,
    ) -> Option<StoredDbItem<T>> {
        let mut db = self.inner.lock();
        let db = db.deref_mut();

        let item = db
            .items
            .get_mut(T::type_name())
            .and_then(|i| i.read().get(&None).cloned())
            .map(|i| StoredDbItem::new(i, self.clone()));

        item
    }

    /// Adds an item to the database immediately
    ///
    /// It is not possible to get back an item added this way, if you want to
    /// reference or modify the added item, use [add_item]
    pub(crate) fn consume_item<T: Into<Item>>(&self, item: T) {
        let mut db = self.inner.lock();
        let db = db.deref_mut();

        let item = item.into();
        let type_name = item.inner_type_name();
        let id = item.id();
        let map = db.items.entry(type_name).or_default();
        if map
            .write()
            .insert(id, Arc::new(RwLock::new(item)))
            .is_some()
        {
            if let Some(id) = id {
                error!(id, ty = type_name, "Item ID collision detected")
            } else {
                error!(ty = type_name, "Duplicate setting detected")
            }
        }
    }

    /// Saves database to the file system, overriding old files
    pub fn save(self: Arc<Self>) -> DiagnosticContext {
        const ERR_DANGLING_DATABASE: &str = "Should not have dangling references to the database before saving. Check your item handles for leakage";
        const ERR_DANGLING_COLLECTION: &str = "Should not have dangling references to the database collections before saving. Check your iterator usage for leaking";
        const ERR_DANGLING_ITEM: &str = "Should not have dangling references to the database item before saving. Check your item handles for leakage";
        const ERR_DANGLING_MAPPINGS: &str = "Should not have dangling references to the database mappings before saving. Check your contexts handles for leakage";

        let settings = self
            .get_singleton::<DatabaseSettings>()
            .map(|s| s.new_clone().forget());

        let guard_a = error_span!("Saving database").entered();
        let db = Arc::into_inner(self).expect(ERR_DANGLING_DATABASE);
        let db = db.inner.into_inner();
        let path = db.output_path;
        drop(guard_a);
        let _guard = error_span!("Saving database", path=%path.display()).entered();

        let path = path
            .canonicalize()
            .expect("Should be able to canonicalize path");

        if !path.is_dir() {
            panic!("Output path is not a directory");
        }

        let mut saved_files = HashSet::new();

        let mappings_path = path.join(MAPPINGS_NAME);
        let mappings_bk_path = path.join(MAPPINGS_BACKUP_NAME);
        let hashes_path = path.join(HASHES_NAME);
        check_no_backup(&mappings_bk_path);

        let mappings = MappingsSerde {
            ids: db.ids.as_serializable().clone(),
            others: db
                .other_ids
                .into_iter()
                .map(|(k, v)| {
                    (
                        k,
                        Arc::into_inner(v)
                            .expect(ERR_DANGLING_MAPPINGS)
                            .into_inner()
                            .into_serializable(),
                    )
                })
                .collect(),
        };

        let code =
            serde_json::to_string_pretty(&mappings).expect("Should be able to serialize mappings");

        if mappings_path.exists() {
            fs_err::copy(&mappings_path, &mappings_bk_path)
                .expect("Should be able to create mappings backup");
            fs_err::write(&mappings_path, code).expect("Should be able to write mappings file");
        } else {
            fs_err::write(&mappings_path, code).expect("Should be able to write mappings file");
            fs_err::copy(&mappings_path, &mappings_bk_path)
                .expect("Should be able to create mappings backup");
        }

        let hashes = if hashes_path.exists() {
            let data = fs_err::read(&hashes_path).expect("Should be able to read hashes file");
            let data = decompress(&data);
            bitcode::decode(&data).expect("Should be able to decode hashes file")
        } else {
            BTreeMap::<String, Vec<u8>>::default()
        };

        saved_files.insert(mappings_path);
        saved_files.insert(mappings_bk_path.clone());
        saved_files.insert(hashes_path.clone());

        let inverse_ids = db.ids.get_inverse_ids();

        let (mut build_data, info) = if let Some(path) = db.output_file_path {
            let info = ModBuilderInfo::from_settings(
                path,
                &settings.expect("Building a mod file requires DatabaseSettings"),
            );
            (ModBuilderData::new(), Some(info))
        } else {
            (ModBuilderData::dummy(), None)
        };

        let mut ctx = DiagnosticContext::default();

        let mut files_to_write = vec![];

        let mut parent_dirs = BTreeSet::default();

        for item in db.items.into_values().flat_map(|m| {
            Arc::into_inner(m)
                .expect(ERR_DANGLING_COLLECTION)
                .into_inner()
                .into_values()
        }) {
            let item_handle = item.read();
            let type_name = item_handle.inner_type_name();
            let id = item_handle.id();
            drop(item_handle);

            let guard_early = error_span!("Saving item", ty = type_name, id).entered();
            let item = Arc::into_inner(item).expect(ERR_DANGLING_ITEM).into_inner();
            let type_name = item.inner_type_name();
            let file_name = item
                .id()
                .map(|id| {
                    inverse_ids
                        .get(type_name)
                        .and_then(|ids| ids.get(&id).cloned())
                        .map(|id| {
                            let id = id.replace(':', "/");
                            format!("{id}_{type_name}.json")
                        })
                        .unwrap_or_else(|| format!("auto/{type_name}_{id}.json"))
                })
                .unwrap_or_else(|| format!("settings/{type_name}.json"));

            let path = path.join(&file_name);

            drop(guard_early);
            let _guard = error_span!("Saving item", ty = type_name, id, file_name).entered();

            item.validate(ctx.enter_new(file_name));

            let _save_file_guard = error_span!("Writing file", path=%path.display()).entered();

            if saved_files.contains(&path) {
                panic!("File with this name was already saved");
            }

            let json = serde_json::ser::to_string_pretty(&item)
                .expect("Should be able to serialize the item");

            build_data.add_file(path.clone(), json.as_bytes());

            saved_files.insert(path.clone());
            let mut p: &Path = &path;
            while let Some(parent) = p.parent() {
                saved_files.insert(parent.to_path_buf());
                p = parent;
            }

            parent_dirs.insert(path.parent().unwrap().to_path_buf());
            files_to_write.push((path, json));
        }

        parent_dirs.into_par_iter().for_each(|p| {
            fs_err::create_dir_all(p).expect("Should be able to create parent dir for a file");
        });

        let updated_count = Arc::new(AtomicUsize::new(0));
        let total_to_write = files_to_write.len();

        let new_hashes = files_to_write
            .into_par_iter()
            .filter_map(|(path, json)| {
                if let Some(path) = path.as_os_str().to_str() {
                    let hash = sha256(json.as_bytes());

                    let old_hash = hashes.get(path).cloned();

                    if !old_hash.is_some_and(|old_hash| old_hash == hash) {
                        fs_err::write(path, json).expect("Should be able to write the file");
                        updated_count.fetch_add(1, Ordering::Release);
                    }

                    Some((path.to_string(), hash))
                } else {
                    fs_err::write(path, json).expect("Should be able to write the file");
                    updated_count.fetch_add(1, Ordering::Release);
                    None
                }
            })
            .collect::<HashMap<String, Vec<u8>>>();

        fs_err::write(
            hashes_path,
            compress(&bitcode::encode(&new_hashes), Compression::best()),
        )
        .expect("Should be able to write hashes file");

        let updated_count = updated_count.load(Ordering::Acquire);

        let mut cleaned = 0;

        let files = walkdir::WalkDir::new(path);
        // let files = fs_err::read_dir(path).expect("Should be able to read output directory");
        for file in files {
            let file = file.expect("Should be able to read a dir entry");
            let _guard = error_span!("Checking entry", path=%file.path().display()).entered();
            // if !file.path().is_file() {
            //     panic!("Output directory contains a non-file entry");
            // }

            if saved_files.contains(file.path()) {
                continue;
            }

            let _guard = error_span!("Cleaning up old file", path=%file.path().display()).entered();

            fs_err::remove_file(file.path()).expect("Should be able to delete old file");
            cleaned += 1;
        }

        fs_err::remove_file(mappings_bk_path).expect("Should remove mappings backup file");

        if let Some(info) = info {
            build_data
                .build(&info)
                .expect("Should be able to build mod file");
        }

        info!(
            updated_files = updated_count,
            skipped_files = total_to_write - updated_count,
            cleaned_files = cleaned,
            "Database saved successfully!"
        );

        ctx
    }

    fn lock<T>(&self, actions: impl FnOnce(&mut DatabaseInner) -> T) -> T {
        let mut db = self.inner.lock();
        actions(db.deref_mut())
    }
}

impl DatabaseHolder {
    pub fn load_from_dir(&self, dir: impl AsRef<Path>) {
        let path = dir.as_ref();
        let _guard = error_span!("Loading existing database files", path=%path.display()).entered();
        let walk: Vec<_> = walkdir::WalkDir::new(dir)
            .into_iter()
            .collect::<Result<_, _>>()
            .expect("Should be able to read all files in the directory");
        let items: Vec<_> = walk
            .into_par_iter()
            .filter_map(|entry| {
                if !entry.file_type().is_file() {
                    return None;
                }

                let path = entry.path();

                let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
                    return None;
                };

                if ext != "json" {
                    return None;
                }

                let _guard = error_span!("Loading file", path=%path.display()).entered();

                let data = fs_err::read(path).expect("Should be able to read a file");

                let data: Item = serde_json5::from_slice(&data).expect("Should be a valid json");

                Some((path.to_path_buf(), data))
            })
            .collect();

        for (path, data) in items {
            let _guard = error_span!("Registering file", path=%path.display()).entered();

            self.consume_item(data);
        }
    }

    pub fn load_from_included_dir(&self, dir: &include_dir::Dir) {
        fn walkdir<'a>(dir: &include_dir::Dir<'a>) -> Vec<include_dir::File<'a>> {
            let mut items = vec![];
            append_files(dir, &mut items);
            items
        }

        fn append_files<'a>(dir: &include_dir::Dir<'a>, files: &mut Vec<include_dir::File<'a>>) {
            for entry in dir.entries() {
                match entry {
                    include_dir::DirEntry::Dir(dir) => append_files(dir, files),
                    include_dir::DirEntry::File(file) => files.push(file.clone()),
                }
            }
        }

        let files = walkdir(dir);

        let items: Vec<_> = files
            .into_par_iter()
            .filter_map(|entry| {
                let path = entry.path();

                let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
                    return None;
                };

                if ext != "json" {
                    return None;
                }

                let _guard = error_span!("Loading file", path=%path.display()).entered();

                let data = entry.contents();

                let data: Item = serde_json5::from_slice(data).expect("Should be a valid json");

                Some((path.to_path_buf(), data))
            })
            .collect();

        for (path, data) in items {
            let _guard = error_span!("Registering file", path=%path.display()).entered();

            self.consume_item(data);
        }
    }
}

pub trait Remember: Into<Item> + DatabaseItem {
    fn remember(self, db: &Database) -> DbItem<Self>;
}
