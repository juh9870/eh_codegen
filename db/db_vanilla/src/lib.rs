use eh_mod_dev::database::Database;
use eh_mod_dev::vanilla_mappings::add_vanilla_mappings;

pub fn load_vanilla(db: &Database) {
    static DB: include_dir::Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/vanilla");
    db.load_from_included_dir(&DB);

    add_vanilla_mappings(db);
}
