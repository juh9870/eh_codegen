use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::ops::{DerefMut, Range};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ahash::AHashMap;
use parking_lot::{Mutex, RwLock};
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use tracing::{error, error_span, info};

use crate::builder::{ModBuilderData, ModBuilderInfo};
pub use crate::database::db_item::DbItem;
use crate::database::extra_item::ExtraItem;
pub use crate::database::iters::{DatabaseItemIter, DatabaseItemIterMut};
pub use crate::database::stored_db_item::StoredDbItem;
pub use crate::mapping::DatabaseIdLike;
use crate::mapping::{IdIter, IdMapping, IdMappingSerialized, KindProvider, RegexIter};
use diagnostic::context::DiagnosticContext;
use eh_schema::schema::{DatabaseItem, DatabaseItemId, DatabaseSettings, Item};
use smart_output::SmartOutput;

pub mod db_item;
pub mod extra_item;
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
type ItemsMap = Arc<RwLock<AHashMap<Option<i32>, SharedItem>>>;

pub struct DatabaseInner {
    output_path: PathBuf,
    output_file_path: Option<PathBuf>,
    ids: IdMapping,
    other_ids: AHashMap<Cow<'static, str>, Arc<RwLock<IdMapping>>>,
    items: AHashMap<&'static str, ItemsMap>,
    images: AHashMap<String, Arc<image::DynamicImage>>,
    extras: AHashMap<TypeId, Arc<RwLock<dyn Any + Send + Sync>>>,
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
                images: Default::default(),
                extras: Default::default(),
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

    /// Returns raw ID without checking if it exists or marking it as existing
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

    pub fn forget_used_id<T: 'static + DatabaseItem>(&self, string_id: &str) {
        self.lock(|db| db.ids.forget_used_id(T::type_name(), string_id))
    }

    pub fn is_id_used<T: 'static + DatabaseItem>(&self, string_id: &str) -> bool {
        self.lock(|db| db.ids.is_used(T::type_name(), string_id))
    }

    pub fn iter_ids<T: 'static + DatabaseItem, U>(&self, func: impl FnOnce(IdIter) -> U) -> U {
        self.lock(|db| func(db.ids.used_ids(T::kind())))
    }

    pub fn iter_ids_filtered<T: 'static + DatabaseItem, U>(
        &self,
        pat: &str,
        func: impl FnOnce(RegexIter) -> U,
    ) -> U {
        self.lock(|db| func(db.ids.used_ids_filtered(pat, T::kind())))
    }

    pub fn get_id_name<T: 'static + DatabaseItem>(&self, id: DatabaseItemId<T>) -> Option<String> {
        self.lock(|db| db.ids.get_inverse_id(T::type_name(), id.0))
    }

    pub fn cached<T: 'static + DatabaseItem>(
        &self,
        id: &str,
        cb: impl FnOnce() -> DatabaseItemId<T>,
    ) -> DatabaseItemId<T> {
        if self.is_id_used::<T>(id) {
            return self.id(id);
        }

        cb()
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

    pub fn use_id_mappings<T>(&self, func: impl FnOnce(&mut IdMapping) -> T) -> T {
        self.lock(|db| func(&mut db.ids))
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

    pub fn insert_extra<T: Any + Send + Sync>(&self, extra: T) {
        self.lock(|db| {
            db.extras
                .insert(TypeId::of::<T>(), Arc::new(RwLock::new(extra)))
        });
    }

    pub fn init_extra<T: Any + Send + Sync + Default>(&self) {
        self.insert_extra(T::default())
    }

    pub fn extra<T: Any + Send + Sync>(&self) -> ExtraItem<T> {
        let item = self.lock(|db| db.extras.get(&TypeId::of::<T>()).unwrap().clone());
        ExtraItem::new(item)
    }

    pub fn extra_or_init<T: Any + Send + Sync + Default>(&self) -> ExtraItem<T> {
        let item = self.lock(|db| {
            db.extras
                .entry(TypeId::of::<T>())
                .or_insert_with(|| Arc::new(RwLock::new(T::default())))
                .clone()
        });
        ExtraItem::new(item)
    }

    /// Inserts an image, returning the previous image with the same name if it existed
    pub fn insert_image(
        &self,
        name: String,
        image: image::DynamicImage,
    ) -> Option<Arc<image::DynamicImage>> {
        self.lock(|db| db.images.insert(name, Arc::new(image)))
    }

    /// Gets an image by name
    pub fn get_image(&self, name: &str) -> Option<Arc<image::DynamicImage>> {
        self.lock(|db| db.images.get(name).cloned())
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
        let output_path = db.output_path;
        drop(guard_a);

        let _guard = error_span!("Saving database", path=%output_path.display()).entered();

        let output_path = output_path
            .canonicalize()
            .expect("Should be able to canonicalize path");

        if !output_path.is_dir() {
            panic!("Output path is not a directory");
        }

        let mut output =
            SmartOutput::init(output_path.clone()).expect("Should be able to init output");

        let mappings_path = output_path.join(MAPPINGS_NAME);
        let mappings_bk_path = output_path.join(MAPPINGS_BACKUP_NAME);
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

            let path = output_path.join(&file_name);

            drop(guard_early);
            let _guard = error_span!("Saving item", ty = type_name, id, file_name).entered();

            item.validate(ctx.enter_new(file_name));

            let _save_file_guard = error_span!("Writing file", path=%path.display()).entered();

            let json = serde_json::ser::to_string_pretty(&item)
                .expect("Should be able to serialize the item");

            build_data.add_file(path.clone(), json.as_bytes());

            output
                .add_file(path, json)
                .expect("Should be able to save the file");
        }

        output.flush().expect("Should be able to flush the output");

        fs_err::remove_file(mappings_bk_path).expect("Should remove mappings backup file");

        if let Some(info) = info {
            build_data
                .build(&info)
                .expect("Should be able to build mod file");
        }

        info!("Database saved successfully!");

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

                let ext = path.extension().and_then(|ext| ext.to_str())?;

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

                let ext = path.extension().and_then(|ext| ext.to_str())?;

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
