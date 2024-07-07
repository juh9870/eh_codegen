use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;

use tracing::error_span;

use eh_schema::schema::{DatabaseItem, DatabaseItemId};

pub type IdMappingSerialized = BTreeMap<Cow<'static, str>, BTreeMap<String, i32>>;

#[derive(Debug, Clone)]
pub struct IdMapping {
    ids: BTreeMap<Cow<'static, str>, BTreeMap<String, i32>>,
    used_ids: HashMap<Cow<'static, str>, HashSet<String>>,
    occupied_ids: HashMap<Cow<'static, str>, HashSet<i32>>,
    available_ids: HashMap<Cow<'static, str>, Vec<Range<i32>>>,
    default_ids: Vec<Range<i32>>,
}

impl IdMapping {
    pub fn new(mappings: IdMappingSerialized) -> Self {
        let occupied_ids = mappings
            .iter()
            .map(|(k, v)| (k.clone(), v.values().copied().collect::<HashSet<i32>>()))
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

            if self
                .used_ids
                .entry(kind.clone())
                .or_default()
                .contains(&id_str)
            {
                panic!("ID is already in use")
            }
        }

        self.get_id_raw(kind, id_str)
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

    pub fn get_inverse_id<'a, T: 'static + DatabaseItem>(
        &'a self,
        kind: impl Into<Cow<'a, str>>,
        id: DatabaseItemId<T>,
    ) -> Option<String> {
        let kind = kind.into();

        self.ids.get(&kind).and_then(|i| {
            i.iter()
                .find_map(|(k, v)| if *v == id.0 { Some(k.clone()) } else { None })
        })
    }

    pub fn get_inverse_ids(&self) -> HashMap<Cow<'static, str>, HashMap<i32, String>> {
        self.ids
            .iter()
            .map(|(ty, ids)| {
                let ids: HashMap<_, _> = ids.iter().map(|(k, v)| (*v, k.clone())).collect();
                (ty.clone(), ids)
            })
            .collect()
    }

    fn next_id_raw(&mut self, kind: impl Into<Cow<'static, str>>) -> i32 {
        if self.default_ids.is_empty() {
            panic!(
                "No ID range were given for Database to assign, please use `add_id_range` method"
            )
        }

        let kind = kind.into();
        let ids = self
            .available_ids
            .entry(kind.clone())
            .or_insert_with(|| self.default_ids.clone());

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
