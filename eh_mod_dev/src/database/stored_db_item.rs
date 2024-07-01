use crate::database::db_item::DbItem;
use crate::database::{DatabaseHolder, SharedItem};
use eh_schema::schema::Item;
use parking_lot::lock_api::RwLockReadGuard;
use parking_lot::{MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLockWriteGuard};
use std::any::Any;
use std::marker::PhantomData;
use std::ops::DerefMut;
use std::sync::Arc;

/// Database item temporarily taken from the database
pub struct StoredDbItem<T: Any> {
    item: SharedItem,
    db: Arc<DatabaseHolder>,
    _item: PhantomData<T>,
}

impl<T: Any> StoredDbItem<T> {
    pub(crate) fn new(item: SharedItem, db: Arc<DatabaseHolder>) -> Self {
        Self {
            item,
            db,
            _item: Default::default(),
        }
    }
}

impl<T: Any> StoredDbItem<T> {
    /// Runs a range of actions in a convenient closure
    ///
    /// Value returned from closure is ignored, to simplify one-liners (no ; needed)
    pub fn edit(&self, actions: impl FnOnce(&mut T)) -> &Self {
        let mut lock = self.write();
        actions(lock.deref_mut());
        self
    }

    /// Runs a range of actions on an owned instance of an item, that must be
    /// returned back
    pub fn with(&self, actions: impl FnOnce(T) -> T) -> &Self {
        let mut lock = self.write();
        replace_with::replace_with_or_abort(lock.deref_mut(), actions);
        self
    }

    /// Provides write access to the underlying data
    pub fn write(&self) -> MappedRwLockWriteGuard<'_, T> {
        RwLockWriteGuard::map(self.item.write(), |i| {
            i.as_inner_any_mut().downcast_mut::<T>().unwrap()
        })
    }

    /// Provides access to the underlying data
    pub fn read(&self) -> MappedRwLockReadGuard<'_, T> {
        RwLockReadGuard::map(self.item.read(), |i| {
            i.as_inner_any_ref().downcast_ref::<T>().unwrap()
        })
    }
}

impl<T: Any + Clone + Into<Item>> StoredDbItem<T> {
    /// Creates a new database item that is a clone of the current one
    ///
    /// Don't forget to change ID, otherwise the app will panic
    pub fn new_clone(&self) -> DbItem<T> {
        DbItem::new(transmogrify(self.item.read().clone()), self.db.clone())
    }
}

fn transmogrify<T: Any>(item: Item) -> T {
    *item.into_inner_any().downcast::<T>().unwrap()
}
// fn transmogrify_mut<T:Any>(item: &mut Item) -> &mut T {
//     item.as_inner_any_mut().downcast_mut().unwrap()
// }
