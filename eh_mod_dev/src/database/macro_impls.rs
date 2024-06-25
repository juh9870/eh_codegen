use super::{DatabaseHolder, DatabaseIdLike, DbItem};
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
            }
        )*
    };
}

apply_items!(item_impls);
