use tracing::instrument;

use eh_mod_cli::dev::database::{database, Database};
use eh_mod_cli::dev::schema::schema::DatabaseSettings;
use eh_mod_cli::dev::validators::validate_settings;
use eh_mod_cli::dev::vanilla_mappings::add_vanilla_mappings;
use eh_mod_cli::Args;

#[instrument]
pub fn build_mod(args: Args) {
    let db = database(args.output_dir, args.output_mod);

    db.load_from_dir(args.vanilla_dir);

    db.add_id_range(1..999999999);

    add_vanilla_mappings(&db);

    settings(&db);

    db.save();
}

fn settings(db: &Database) {
    db.get_singleton::<DatabaseSettings>().unwrap().edit(|i| {
        i.mod_name = "ScrapLite".to_string();
        i.mod_id = "scraplite_dev".to_string();
        i.mod_version = 1;
    });

    db.factions_settings();

    validate_settings(db);
}
