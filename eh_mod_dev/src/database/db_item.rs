use crate::database::DatabaseHolder;
use eh_schema::schema::Item;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

pub struct DbItem<T: Into<Item>> {
    item: Option<T>,
    db: Arc<DatabaseHolder>,
}

impl<T: Into<Item>> DbItem<T> {
    pub(crate) fn new(item: T, db: Arc<DatabaseHolder>) -> Self {
        Self {
            item: Some(item),
            db,
        }
    }
}

impl<T: Into<Item>> DbItem<T> {
    /// Prevents item from getting written into the database
    pub fn forget(mut self) -> T {
        std::mem::take(&mut self.item).unwrap()
    }

    /// Saves the item to the database
    ///
    /// This is basically a more convenient `drop`
    pub fn save(self) {}

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
