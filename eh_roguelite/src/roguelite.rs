use tracing::instrument;

use eh_mod_cli::Args;
use eh_mod_cli::db_vanilla::load_vanilla;
use eh_mod_cli::dev::database::{database, Database};
use eh_mod_cli::dev::schema::schema::{
    DatabaseSettings, GalaxySettings, NodeCancelQuest, NodeRetreat, Quest, StartCondition,
};
use eh_mod_cli::dev::validators::validate_settings;

use crate::roguelite::core::core_quest;
use crate::roguelite::enemy_fleets::create_fleets;
use crate::roguelite::events::Events;

mod core;
mod enemy_fleets;
mod events;

#[instrument]
pub fn build_mod(args: Args) {
    let db = database(args.output_dir.clone(), args.output_mod.clone());

    load_vanilla(&db);

    db.add_id_range(10000..999999999);

    patch_vanilla(&db);

    settings(&db);

    create_fleets(&db);

    core_quest(&db);

    db.save();
}

fn patch_vanilla(db: &Database) {
    db.faction_iter_mut(|f| {
        for mut faction in f {
            faction.hidden = true;
            faction.hide_research_tree = true;
        }
    });

    db.get_item::<Quest>("eh:local_pirates").unwrap().edit(|q| {
        q.nodes = vec![
            NodeRetreat {
                id: 1,
                default_transition: 2,
            }
            .into(),
            NodeCancelQuest { id: 2 }.into(),
        ];
    });

    db.quest_iter_mut(|q| {
        for mut q in q {
            if matches!(q.start_condition, StartCondition::GameStart) {
                q.start_condition = StartCondition::Manual;
            }
        }
    })
}

#[instrument]
fn settings(db: &Database) {
    db.get_singleton::<DatabaseSettings>().unwrap().edit(|s| {
        s.mod_name = "ScrapLite".to_string();
        s.mod_id = "scraplite_dev".to_string();
        s.mod_version = 1;
    });

    db.get_singleton::<GalaxySettings>().unwrap().edit(|s| {
        s.enemy_level = "MAX(distance - 100, 0)".to_string();
        s.max_enemy_ships_level = 500;
    });

    db.new_factions_settings();

    db.init_extra::<Events>();

    validate_settings(db);
}

const MSG_GONE_WRONG:&str = "Something gone wrong.\nPlease screenshot this error and send it to juh9870 on Discord\nError:\n";
