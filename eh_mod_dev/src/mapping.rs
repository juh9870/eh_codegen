use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ops::Range;

use ahash::{AHashMap, AHashSet};
use regex::Regex;
use tracing::error_span;

use eh_schema::schema::{DatabaseItem, DatabaseItemId};

pub type IdMappingSerialized = BTreeMap<Cow<'static, str>, BTreeMap<String, i32>>;

pub type IdIter<'a> =
    std::iter::Flatten<std::option::IntoIter<std::collections::hash_set::Iter<'a, String>>>;

#[derive(Debug, Clone, Default)]
pub struct IdMapping {
    ids: BTreeMap<Cow<'static, str>, BTreeMap<String, i32>>,
    used_ids: AHashMap<Cow<'static, str>, AHashSet<String>>,
    occupied_ids: AHashMap<Cow<'static, str>, AHashSet<i32>>,
    available_ids: AHashMap<Cow<'static, str>, Vec<Range<i32>>>,
    default_ids: Vec<Range<i32>>,
}

impl IdMapping {
    pub fn new(mappings: IdMappingSerialized) -> Self {
        let occupied_ids = mappings
            .iter()
            .map(|(k, v)| (k.clone(), v.values().copied().collect::<AHashSet<i32>>()))
            .collect();

        Self {
            occupied_ids,
            used_ids: Default::default(),
            ids: mappings,
            available_ids: Default::default(),
            default_ids: Default::default(),
        }
    }

    pub fn as_serializable(&self) -> &IdMappingSerialized {
        &self.ids
    }
    pub fn into_serializable(self) -> IdMappingSerialized {
        self.ids
    }

    /// Adds another ID range to use for all entries
    pub fn add_id_range(&mut self, range: Range<i32>) {
        for ids in self.available_ids.values_mut() {
            ids.push(range.clone());
        }
        self.default_ids.push(range);
    }

    /// Adds another ID range to use for one specified kind
    pub fn add_id_range_for(&mut self, kind: impl Into<Cow<'static, str>>, range: Range<i32>) {
        self.available_ids
            .entry(kind.into())
            .or_insert_with(|| self.default_ids.clone())
            .push(range)
    }

    /// Clears allocated ID ranges for the specified type, preventing obtaining
    /// of new IDs until [add_id_range] or [add_id_range_for] are used to
    /// allocate new ID space for this type
    pub fn clear_id_ranges_for(&mut self, kind: impl Into<Cow<'static, str>>) {
        self.available_ids
            .entry(kind.into())
            .and_modify(|e| e.clear());
    }

    /// Converts string ID into database item ID
    ///
    /// Panics if generating ID is not possible
    pub fn get_id_raw(&mut self, kind: impl Into<Cow<'static, str>>, id: impl Into<String>) -> i32 {
        let id_str = id.into();

        let kind = kind.into();
        let mapping = self.ids.entry(kind.clone()).or_default();

        match mapping.get(&id_str) {
            None => {
                let id = self.next_id_raw(kind.clone());
                self.ids
                    .get_mut(&kind)
                    .expect("ID entry should be present at this point")
                    .insert(id_str, id);
                id
            }
            Some(id) => *id,
        }
    }

    /// Returns an unstable numeric ID.
    ///
    /// IDs obtained this way are unstable and may change between runs, so
    /// they should not be used for any kind of savefile-persistent data
    pub fn get_unstable_id(&mut self, kind: impl Into<Cow<'static, str>>) -> i32 {
        self.next_id_raw(kind)
    }

    /// Converts string ID into database item ID
    ///
    /// Panics if ID is missing
    pub fn existing_id<'a>(&'a self, kind: impl Into<Cow<'a, str>>, id: &str) -> i32 {
        let kind = kind.into();

        let _guard = error_span!("Getting item ID", id, ty = %kind).entered();

        if !self.used_ids.get(&kind).is_some_and(|ids| ids.contains(id)) {
            panic!("ID is not present in the database")
        }

        *self
            .ids
            .get(&kind)
            .expect("This kind should be present, based on used_id check")
            .get(id)
            .expect("This ID should be present based on used_id check")
    }

    /// Converts string ID into new database item ID
    ///
    /// Panics if generating ID is not possible, or if ID is already used
    pub fn new_id(&mut self, kind: impl Into<Cow<'static, str>>, id: impl Into<String>) -> i32 {
        let id_str = id.into();
        let kind = kind.into();
        {
            let _guard = error_span!("Creating new item ID", id = id_str, ty = %kind).entered();

            let used_ids = self.used_ids.entry(kind.clone()).or_default();

            if !used_ids.insert(id_str.clone()) {
                panic!("ID is already in use")
            }
        }

        self.get_id_raw(kind, id_str)
    }

    pub fn is_used(&self, kind: impl Into<Cow<'static, str>>, id: &str) -> bool {
        self.used_ids
            .get(&kind.into())
            .is_some_and(|ids| ids.contains(id))
    }

    /// Forcefully assigns numeric ID to a string
    pub fn set_id(
        &mut self,
        kind: impl Into<Cow<'static, str>>,
        string_id: impl Into<String>,
        numeric_id: i32,
    ) -> i32 {
        let kind = kind.into();
        let string_id = string_id.into();
        self.ids
            .entry(kind.clone())
            .or_default()
            .insert(string_id.clone(), numeric_id);
        self.occupied_ids
            .entry(kind.clone())
            .or_default()
            .insert(numeric_id);
        self.used_ids.entry(kind).or_default().insert(string_id);
        numeric_id
    }

    pub fn forget_used_id(&mut self, kind: impl Into<Cow<'static, str>>, id: &str) {
        let kind = kind.into();
        self.used_ids.entry(kind).or_default().remove(id);
    }

    pub fn get_inverse_id<'a>(&'a self, kind: impl Into<Cow<'a, str>>, id: i32) -> Option<String> {
        let kind = kind.into();

        self.ids.get(&kind).and_then(|i| {
            i.iter()
                .find_map(|(k, v)| if *v == id { Some(k.clone()) } else { None })
        })
    }

    pub fn get_inverse_ids(&self) -> AHashMap<Cow<'static, str>, AHashMap<i32, String>> {
        self.ids
            .iter()
            .map(|(ty, ids)| {
                let ids: AHashMap<_, _> = ids.iter().map(|(k, v)| (*v, k.clone())).collect();
                (ty.clone(), ids)
            })
            .collect()
    }

    // Iterator of all used string ids for the given kind
    pub fn used_ids<'a>(&'a self, kind: impl Into<Cow<'a, str>>) -> IdIter {
        self.used_ids
            .get(&kind.into())
            .map(|h| h.iter())
            .into_iter()
            .flatten()
    }

    pub fn used_ids_filtered<'a>(
        &'a self,
        filter: &str,
        kind: impl Into<Cow<'a, str>>,
    ) -> RegexIter {
        RegexIter {
            regex: Regex::new(filter).unwrap(),
            items: self.used_ids(kind),
        }
    }

    fn next_id_raw(&mut self, kind: impl Into<Cow<'static, str>>) -> i32 {
        let kind = kind.into();

        let ids = self
            .available_ids
            .entry(kind.clone())
            .or_insert_with(|| self.default_ids.clone());

        if ids.is_empty() {
            let _guard = error_span!("Getting next item ID", kind = %kind).entered();
            panic!(
                "No ID range were given for Database to assign or all ids were exhausted, please use `add_id_range` method"
            )
        }

        let mappings = self.occupied_ids.entry(kind).or_default();

        while let Some(id) = ids.iter_mut().find_map(|range| range.next()) {
            // Check that ID is not already occupied
            if !mappings.contains(&id) {
                mappings.insert(id);
                return id;
            }
        }

        panic!("No free IDs are left for this kind");
    }
}

pub trait KindProvider {
    fn kind() -> Cow<'static, str>;
}

impl<T: DatabaseItem> KindProvider for T {
    fn kind() -> Cow<'static, str> {
        Cow::Borrowed(T::type_name())
    }
}

pub trait DatabaseIdLike<T: KindProvider> {
    fn into_id(self, ids: &IdMapping) -> i32;
    fn into_new_id(self, ids: &mut IdMapping) -> i32;
}

impl<T: 'static + DatabaseItem> DatabaseIdLike<T> for DatabaseItemId<T> {
    fn into_id(self, _ids: &IdMapping) -> i32 {
        self.0
    }
    fn into_new_id(self, _ids: &mut IdMapping) -> i32 {
        self.0
    }
}

impl<T: KindProvider> DatabaseIdLike<T> for &str {
    fn into_id(self, ids: &IdMapping) -> i32 {
        ids.existing_id(T::kind(), self)
    }
    fn into_new_id(self, ids: &mut IdMapping) -> i32 {
        ids.new_id(T::kind(), self)
    }
}

impl<T: KindProvider> DatabaseIdLike<T> for String {
    fn into_id(self, ids: &IdMapping) -> i32 {
        ids.existing_id(T::kind(), &self)
    }
    fn into_new_id(self, ids: &mut IdMapping) -> i32 {
        ids.new_id(T::kind(), self)
    }
}

pub trait OptionalDatabaseIdLike<K: KindProvider, T: DatabaseIdLike<K>> {
    fn into_opt(self) -> Option<T>;
}

impl<K: KindProvider, T: DatabaseIdLike<K>> OptionalDatabaseIdLike<K, T> for T {
    fn into_opt(self) -> Option<T> {
        Some(self)
    }
}

impl<K: KindProvider, T: DatabaseIdLike<K>> OptionalDatabaseIdLike<K, T> for Option<T> {
    fn into_opt(self) -> Option<T> {
        self
    }
}

impl<K: KindProvider> OptionalDatabaseIdLike<K, String> for () {
    fn into_opt(self) -> Option<String> {
        None
    }
}

#[derive(Debug)]
pub struct RegexIter<'a> {
    regex: regex::Regex,
    items: IdIter<'a>,
}

impl<'a> Iterator for RegexIter<'a> {
    type Item = <IdIter<'a> as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        for next in self.items.by_ref() {
            if !self.regex.is_match(next) {
                continue;
            }
            return Some(next);
        }

        None
    }
}
