use eh_mod_dev::database::Database;
use include_dir::{include_dir, Dir};

static DB: Dir = include_dir!("$CARGO_MANIFEST_DIR");

pub fn min_mod(db: &Database) {
    db.load_from_included_dir(&DB);
}
