use super::{
    Database, DatabaseHolder, DatabaseIdLike, DatabaseItemIter, DatabaseItemIterMut, DbItem,
    Remember,
};
use eh_schema::apply_items;
use eh_schema::schema::*;
use std::sync::Arc;

macro_rules! process_arg_type {
    (DatabaseItemId<$ty:ty>) => {impl DatabaseIdLike<$ty>};
    ($ty:ty) => {impl Into<$ty>};
}
macro_rules! process_arg_conversion {
    (DatabaseItemId<$ty:ty>, $arg:ident, $target:ident) => {
        $arg.into_id(&$target)
    };
    ($ty:ty, $arg:ident, $target:ident) => {
        $arg.into()
    };
}

macro_rules! item_impls {
    ($($name:ident ( $($arg:ident : ($($arg_ty:tt)*)),* $(,)? ) -> $ty:ty),* $(,)?) => {
        $(
            impl DatabaseHolder {
                pub fn $name(self: &Arc<Self>, $($arg: process_arg_type!($($arg_ty)*)),*) -> DbItem::<$ty> {
                    self.add_item(<$ty>::new($(process_arg_conversion!($($arg_ty)*, $arg, self)),*))
                }
                paste::paste! {
                    pub fn [< $name  _iter >]<U>(self: &Self, func: impl Fn(DatabaseItemIter<'_, $ty>) -> U) -> U {
                        self.iter::<$ty, U>(func)
                    }
                    pub fn [< $name  _iter_mut >]<U>(self: &Self, func: impl Fn(DatabaseItemIterMut<'_, $ty>) -> U) -> U {
                        self.iter_mut::<$ty, U>(func)
                    }
                }
            }

            impl Remember for $ty {
                fn remember(self, db: &Database) -> DbItem<Self> {
                    db.add_item(self)
                }
            }
        )*
    };
}

apply_items!(item_impls);
