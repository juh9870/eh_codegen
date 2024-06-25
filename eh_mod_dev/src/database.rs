use eh_schema::schema::{DatabaseItem, DatabaseItemId, Item};
use std::any::TypeId;

use std::collections::{HashMap, HashSet};

use std::ops::{Deref, DerefMut, Range};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tracing::{error_span, info};

mod macro_impls;

pub fn database(output_path: impl AsRef<Path>) -> Database {
    DatabaseHolder::new(output_path)
}

const MAPPINGS_NAME: &str = "id_mappings.json5";
const MAPPINGS_BACKUP_NAME: &str = "id_mappings.json5.backup";

pub type Database = Arc<DatabaseHolder>;
pub struct DatabaseHolder {
    inner: Mutex<DatabaseInner>,
}
pub struct DatabaseInner {
    path: PathBuf,
    ids: HashMap<String, HashMap<String, i32>>,
    used_ids: HashMap<String, HashSet<i32>>,
    available_ids: HashMap<TypeId, Vec<Range<i32>>>,
    default_ids: Vec<Range<i32>>,
    items: Vec<Item>,
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
    pub fn new(output_path: impl AsRef<Path>) -> Database {
        let output_path = std::env::current_dir()
            .expect("Should be able to get current directory info from process env")
            .join(output_path);

        let _guard =
            error_span!("Creating a new database", output_path=%output_path.display()).entered();
        if !output_path.exists() {
            panic!("Target directory does not exist")
        }

        let mappings_path = output_path.join(MAPPINGS_NAME);
        let mappings: HashMap<String, HashMap<String, i32>> = mappings_path
            .exists()
            .then(|| {
                let data = fs_err::read_to_string(output_path.join(MAPPINGS_NAME))
                    .expect("Should be able to read mappings file");
                serde_json5::from_str(&data).expect("Should be able to deserialize mappings file")
            })
            .unwrap_or_default();

        check_no_backup(&output_path.join(MAPPINGS_BACKUP_NAME));

        let used_ids = mappings
            .iter()
            .map(|(k, v)| (k.clone(), v.values().copied().collect::<HashSet<i32>>()))
            .collect();

        let db = Self {
            inner: Mutex::new(DatabaseInner {
                path: output_path.to_path_buf(),
                used_ids,
                ids: mappings,
                available_ids: Default::default(),
                default_ids: Default::default(),
                items: Default::default(),
            }),
        };
        Arc::new(db)
    }

    /// Adds another ID range to use for all types
    pub fn add_id_range(&self, range: Range<i32>) {
        let mut db = self.inner.lock().unwrap();
        let db = db.deref_mut();
        for ids in db.available_ids.values_mut() {
            ids.push(range.clone());
        }
        db.default_ids.push(range);
    }

    /// Adds another ID range to use for one specified type
    pub fn add_id_range_for<T: 'static + DatabaseItem>(&self, range: Range<i32>) {
        let mut db = self.inner.lock().unwrap();
        let db = db.deref_mut();
        let type_id = TypeId::of::<T>();
        db.available_ids
            .entry(type_id)
            .or_insert_with(|| db.default_ids.clone())
            .push(range)
    }

    /// Clears allocated ID ranges for the specified type, preventing obtaining
    /// of new IDs until [add_id_range] or [add_id_range_for] are used to
    /// allocate new ID space for this type
    pub fn clear_id_ranges_for<T: 'static + DatabaseItem>(&mut self) {
        let mut db = self.inner.lock().unwrap();
        let db = db.deref_mut();
        let type_id = TypeId::of::<T>();
        db.available_ids
            .entry(type_id)
            .or_insert_with(|| db.default_ids.clone())
            .clear()
    }

    /// Converts string ID into database item ID
    ///
    /// Aborts the execution if generating ID is not possible
    pub fn id<T: 'static + DatabaseItem>(&self, id: impl Into<String>) -> DatabaseItemId<T> {
        let mut db = self.inner.lock().unwrap();
        let db = db.deref_mut();
        let id_str = id.into();
        let _guard = error_span!("Getting item ID", id = id_str, ty = T::type_name()).entered();

        let type_name = T::type_name();
        let mapping = db.ids.entry(type_name.to_string()).or_default();

        match mapping.get(&id_str) {
            None => {
                let id = Self::next_id_raw::<T>(db);
                db.ids
                    .get_mut(type_name)
                    .expect("ID entry should be present at this point")
                    .insert(id_str, id);
                id.into()
            }
            Some(id) => (*id).into(),
        }
    }

    /// Forcefully assigns numeric ID to a string
    pub fn set_id<T: 'static + DatabaseItem>(
        &self,
        string_id: impl Into<String>,
        numeric_id: i32,
    ) -> DatabaseItemId<T> {
        let mut db = self.inner.lock().unwrap();
        let db = db.deref_mut();
        let type_name = T::type_name();
        db.ids
            .entry(type_name.to_string())
            .or_default()
            .insert(string_id.into(), numeric_id);
        db.used_ids
            .entry(type_name.to_string())
            .or_default()
            .insert(numeric_id);
        DatabaseItemId::new(numeric_id)
    }

    /// Adds an item to the database, returns a mutable handle to the inserted item
    ///
    /// All returned handles **must** be dropped before saving the database, otherwise a panic will occur.
    ///
    /// # Panics
    /// All items are stored behind a [RefCell], so regular runtime borrowing rules apply
    pub fn add_item<T: Into<Item>>(self: &Arc<Self>, item: T) -> DbItem<T> {
        DbItem {
            item: Some(item),
            db: self.clone(),
        }
    }

    /// Adds an item to the database immediately
    ///
    /// It is not possible to get back an item added this way, if you want to
    /// reference or modify the added item, use [add_item]
    pub fn consume_item<T: Into<Item>>(&self, item: T) {
        let mut db = self.inner.lock().unwrap();
        let db = db.deref_mut();
        db.items.push(item.into());
    }

    /// Saves database to the file system, overriding old files
    pub fn save(self: Arc<Self>) {
        let guard_a = error_span!("Saving database").entered();
        let db = Arc::into_inner(self)
            .expect("Should not have dangling references to the database before saving. Check your item handles for leakage");
        let db = db.inner.into_inner().unwrap();
        let path = db.path;
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
        check_no_backup(&mappings_bk_path);

        let code =
            serde_json::to_string_pretty(&db.ids).expect("Should be able to serialize mappings");

        if mappings_path.exists() {
            fs_err::copy(&mappings_path, &mappings_bk_path)
                .expect("Should be able to create mappings backup");
            fs_err::write(&mappings_path, code).expect("Should be able to write mappings file");
        } else {
            fs_err::write(&mappings_path, code).expect("Should be able to write mappings file");
            fs_err::copy(&mappings_path, &mappings_bk_path)
                .expect("Should be able to create mappings backup");
        }

        saved_files.insert(mappings_path);
        saved_files.insert(mappings_bk_path.clone());

        let inverse_ids: HashMap<_, _> = db
            .ids
            .iter()
            .map(|(ty, ids)| {
                let ids: HashMap<_, _> = ids.iter().map(|(k, v)| (*v, k.clone())).collect();
                (ty.clone(), ids)
            })
            .collect();

        for item in db.items {
            let type_name = item.inner_type_name();
            let file_name = item
                .id()
                .and_then(|id| {
                    inverse_ids.get(type_name).map(|ids| {
                        let id = ids
                            .get(&id)
                            .cloned()
                            .unwrap_or_else(|| format!("auto_{}", id))
                            .replace(':', "-");
                        format!("{id}_{type_name}.json")
                    })
                })
                .unwrap_or_else(|| format!("{type_name}.json"));

            let path = path.join(file_name);

            let _guard = error_span!("Saving file", path=%path.display()).entered();

            if saved_files.contains(&path) {
                panic!("File with this name was already saved");
            }

            let json = serde_json::ser::to_string_pretty(&item)
                .expect("Should be able to serialize the item");
            fs_err::write(&path, json).expect("Should be able to write the file");

            saved_files.insert(path);
        }

        let files = fs_err::read_dir(path).expect("Should be able to read output directory");
        for file in files {
            let file = file.expect("Should be able to read a dir entry");
            let _guard = error_span!("Checking entry", path=%file.path().display()).entered();
            if !file.path().is_file() {
                panic!("Output directory contains a non-file entry");
            }

            if saved_files.contains(&file.path()) {
                continue;
            }

            let _guard = error_span!("Cleaning up old file", path=%file.path().display()).entered();

            fs_err::remove_file(file.path()).expect("Should be able to delete old file");
        }

        fs_err::remove_file(mappings_bk_path).expect("Should remove mappings backup file");
        info!("Database saved successfully!")
    }

    fn next_id_raw<T: 'static + DatabaseItem>(db: &mut DatabaseInner) -> i32 {
        if db.default_ids.is_empty() {
            panic!(
                "No ID range were given for Database to assign, please use `add_id_range` method"
            )
        }

        let type_id = TypeId::of::<T>();
        let type_name = T::type_name();
        let ids = db
            .available_ids
            .entry(type_id)
            .or_insert_with(|| db.default_ids.clone());

        let mappings = db.used_ids.entry(type_name.to_string()).or_default();

        while let Some(id) = ids.iter_mut().find_map(|range| range.next()) {
            // Check that ID is not already in use
            if !mappings.contains(&id) {
                mappings.insert(id);
                return id;
            }
        }

        panic!("No free IDs are left for this type");
    }
}

pub trait DatabaseIdLike<T: 'static + DatabaseItem> {
    fn into_id(self, database: &DatabaseHolder) -> DatabaseItemId<T>;
}

impl<T: 'static + DatabaseItem> DatabaseIdLike<T> for DatabaseItemId<T> {
    fn into_id(self, _database: &DatabaseHolder) -> DatabaseItemId<T> {
        self
    }
}

impl<T: 'static + DatabaseItem> DatabaseIdLike<T> for &str {
    fn into_id(self, database: &DatabaseHolder) -> DatabaseItemId<T> {
        database.id(self)
    }
}

impl<T: 'static + DatabaseItem> DatabaseIdLike<T> for String {
    fn into_id(self, database: &DatabaseHolder) -> DatabaseItemId<T> {
        database.id(self)
    }
}

pub struct DbItem<T: Into<Item>> {
    item: Option<T>,
    db: Arc<DatabaseHolder>,
}

impl<T: Into<Item>> DbItem<T> {
    /// Prevents item from getting written into the database
    pub fn forget(mut self) {
        self.item = None
    }

    /// Runs a range of actions in a convenient closure
    ///
    /// Value returned from closure is ignored, to simplify one-liners (no ; needed)
    pub fn edit(mut self, actions: impl FnOnce(&mut T)) -> Self {
        actions(self.deref_mut());
        self
    }

    /// Runs a range of actions on an owned instance of an item, that must be
    /// returned back
    pub fn with(mut self, actions: impl FnOnce(T) -> T) -> Self {
        let item = std::mem::take(&mut self.item);
        self.item = item.map(actions);
        self
    }
}

impl<T: Into<Item> + Clone> DbItem<T> {
    /// Creates a new database item that is a clone of the current one
    ///
    /// Don't forget to change ID, otherwise the app will panic
    pub fn new_clone(&self) -> Self {
        Self {
            item: self.item.clone(),
            db: self.db.clone(),
        }
    }
}

impl<T: Into<Item>> Deref for DbItem<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.item.as_ref().unwrap()
    }
}

impl<T: Into<Item>> DerefMut for DbItem<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.item.as_mut().unwrap()
    }
}

impl<T: Into<Item>> Drop for DbItem<T> {
    fn drop(&mut self) {
        if let Some(i) = std::mem::take(&mut self.item) {
            self.db.consume_item(i)
        }
    }
}
