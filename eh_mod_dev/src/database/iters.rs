use crate::database::{DatabaseHolder, SharedItem};
use eh_schema::schema::{DatabaseItem, Item};
use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLockReadGuard, RwLockWriteGuard,
};
use std::any::Any;
use std::marker::PhantomData;

impl DatabaseHolder {
    pub fn iter<T: Into<Item> + DatabaseItem + Any, U>(
        &self,
        func: impl Fn(DatabaseItemIter<'_, T>) -> U,
    ) -> U {
        let mut db_lock = self.inner.lock();
        let items = db_lock.items.entry(T::type_name()).or_default().clone();
        drop(db_lock);
        let items = items.read();
        let values = DatabaseItemIter {
            values: items.values(),
            _type: Default::default(),
        };

        func(values)
    }

    pub fn iter_mut<T: Into<Item> + DatabaseItem + Any, U>(
        &self,
        func: impl Fn(DatabaseItemIterMut<'_, T>) -> U,
    ) -> U {
        let mut db_lock = self.inner.lock();
        let items = db_lock.items.entry(T::type_name()).or_default().clone();
        drop(db_lock);
        let mut items = items.write();
        let values = DatabaseItemIterMut {
            values: items.values_mut(),
            _type: Default::default(),
        };

        func(values)
    }
}

pub struct DatabaseItemIter<'a, T: Into<Item> + DatabaseItem + Any> {
    values: std::collections::hash_map::Values<'a, Option<i32>, SharedItem>,
    _type: PhantomData<T>,
}

impl<'a, T: Into<Item> + DatabaseItem + Any> Iterator for DatabaseItemIter<'a, T> {
    type Item = MappedRwLockReadGuard<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        let next_value = self.values.next()?;

        return Some(RwLockReadGuard::map(next_value.read(), |lock| {
            lock.as_inner_any_ref().downcast_ref::<T>().unwrap()
        }));
    }
}

pub struct DatabaseItemIterMut<'a, T: Into<Item> + DatabaseItem + Any> {
    values: std::collections::hash_map::ValuesMut<'a, Option<i32>, SharedItem>,
    _type: PhantomData<T>,
}

impl<'a, T: Into<Item> + DatabaseItem + Any> Iterator for DatabaseItemIterMut<'a, T> {
    type Item = MappedRwLockWriteGuard<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        let next_value = self.values.next()?;

        return Some(RwLockWriteGuard::map(next_value.write(), |lock| {
            lock.as_inner_any_mut().downcast_mut::<T>().unwrap()
        }));
    }
}
