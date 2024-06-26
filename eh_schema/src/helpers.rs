use serde::Deserializer;

pub trait DatabaseItem: serde::Serialize + for<'a> serde::Deserialize<'a> {
    fn validate(&mut self);
    fn type_name() -> &'static str;
}

pub trait DatabaseItemWithId: DatabaseItem + Sized {
    fn id(&self) -> DatabaseItemId<Self>;
}

pub struct DatabaseItemId<T: DatabaseItem>(pub i32, std::marker::PhantomData<T>);

impl<T: DatabaseItem> DatabaseItemId<T> {
    pub fn new(id: i32) -> Self {
        Self(id, Default::default())
    }
}

impl<T: DatabaseItem> From<i32> for DatabaseItemId<T> {
    fn from(x: i32) -> Self {
        Self::new(x)
    }
}

impl<T: DatabaseItem> From<DatabaseItemId<T>> for i32 {
    fn from(x: DatabaseItemId<T>) -> Self {
        x.0
    }
}

impl<T: DatabaseItem> serde::Serialize for DatabaseItemId<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, T: DatabaseItem> serde::Deserialize<'de> for DatabaseItemId<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(i32::deserialize(deserializer)?, Default::default()))
    }
}

impl<T: DatabaseItem> PartialEq for DatabaseItemId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: DatabaseItem> Eq for DatabaseItemId<T> {}

impl<T: DatabaseItem> Clone for DatabaseItemId<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: DatabaseItem> Copy for DatabaseItemId<T> {}

impl<T: DatabaseItem> std::fmt::Debug for DatabaseItemId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(&format!("DatabaseItemId::<{}>", T::type_name()))
            .field(&self.0)
            .field(&format_args!("_"))
            .finish()
    }
}
