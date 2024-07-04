use eh_schema::apply_all_settings;

macro_rules! all_settings_impls {
    ($($name:ident : $ty:ty),*) => {
        /// Ensures the presence of all settings
        pub fn validate_settings(db: &crate::database::Database) {
            use eh_schema::schema::*;
            $(
                db.get_singleton::<$ty>().expect(concat!("Setting ", stringify!($name), " should be present in the database"));
            )*
        }

        /// Transfer settings from the source database to the target
        ///
        /// This function will skip transfering settings that are not present
        /// in the database. Use [validate_settings] if you need to ensure the
        /// presence of all settings
        pub fn transfer_settings(source: &crate::database::Database, target: &crate::database::Database) {
            use eh_schema::schema::*;
            use crate::database::Remember;
            $(
                source.get_singleton::<$ty>().map(|s|s.new_clone().forget().remember(target));
            )*
        }
    }
}

apply_all_settings!(all_settings_impls);
