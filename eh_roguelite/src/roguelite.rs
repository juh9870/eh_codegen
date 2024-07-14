use tracing::instrument;

use eh_mod_cli::db_minimal::load_minimal;
use eh_mod_cli::dev::database::{database, Database};
use eh_mod_cli::dev::schema::schema::DatabaseSettings;
use eh_mod_cli::dev::validators::validate_settings;
use eh_mod_cli::Args;

#[instrument]
pub fn build_mod(args: Args) {
    let db = database(args.output_dir, args.output_mod);

    load_minimal(&db);

    db.add_id_range(1..999999999);

    settings(&db);

    db.save();
}

#[instrument]
fn settings(db: &Database) {
    db.get_singleton::<DatabaseSettings>().unwrap().edit(|s| {
        s.mod_name = "ScrapLite".to_string();
        s.mod_id = "scraplite_dev".to_string();
        s.mod_version = 1;
    });

    db.new_factions_settings();

    validate_settings(db);
}
