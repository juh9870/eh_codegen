use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};
use std::any::Any;
use std::marker::PhantomData;
use std::ops::DerefMut;
use std::sync::Arc;

pub struct ExtraItem<T: Any + Send + Sync>(Arc<RwLock<dyn Any>>, PhantomData<T>);

impl<T: Any + Send + Sync> ExtraItem<T> {
    pub fn new(item: Arc<RwLock<dyn Any>>) -> Self {
        Self(item, Default::default())
    }
}

impl<T: Any + Send + Sync> ExtraItem<T> {
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

    pub fn read(&self) -> MappedRwLockReadGuard<T> {
        RwLockReadGuard::map(self.0.read(), |i| i.downcast_ref().unwrap())
    }

    pub fn write(&self) -> MappedRwLockWriteGuard<T> {
        RwLockWriteGuard::map(self.0.write(), |i| i.downcast_mut().unwrap())
    }
}
