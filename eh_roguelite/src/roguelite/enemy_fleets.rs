use std::ops::DerefMut;

use eh_mod_cli::dev::database::{Database, DbItem, Remember};
use eh_mod_cli::dev::mapping::DatabaseIdLike;
use eh_mod_cli::dev::schema::schema::{
    CombatRules, CombatRulesId, Fleet, PlayerShipSelectionMode, RewardCondition, ShipBuild,
    TimeOutMode,
};

use crate::roguelite::events::{Event, EventKind};
use crate::roguelite::Events;

pub fn create_fleets(db: &Database) {
    rules(db);

    let events = db.extra::<Events>();
    let mut events = events.write();

    chapter_1(db, events.deref_mut())
}

fn rules(db: &Database) {
    let basic_rules = CombatRules {
        id: db.new_id("rgl:basic"),
        initial_enemy_ships: "RANDOM_INT(2, 5)".to_string(),
        max_enemy_ships: "12".to_string(),
        battle_map_size: 200,
        time_limit: "30".to_string(),
        time_out_mode: TimeOutMode::CallNextEnemy,
        loot_condition: RewardCondition::Default,
        exp_condition: RewardCondition::Default,
        ship_selection: PlayerShipSelectionMode::NoRetreats,
        disable_skill_bonuses: false,
        disable_random_loot: true,
        disable_asteroids: false,
        disable_planet: false,
        next_enemy_button: true,
        kill_them_all_button: false,
        custom_soundtrack: vec![],
    }
    .remember(db);

    basic_rules
        .new_clone()
        .set_id(db.new_id("rgl:gang"))
        .set_initial_enemy_ships("100")
        .set_max_enemy_ships("100")
        .set_time_limit("120")
        .set_time_out_mode(TimeOutMode::DrainPlayerHp);

    basic_rules
        .new_clone()
        .set_id(db.new_id("rgl:blitz"))
        .set_time_limit("10");
}

fn chapter_1(db: &Database, events: &mut Events) {
    let scouts = vec![db.id::<ShipBuild>("eh:scout"); 5];
    let fleet = fleet(db, "rgl:scouts", 0, scouts, None);

    events.push(
        Event::new(
            db,
            "rgl:scouts",
            EventKind::Combat(vec![fleet.id.into()], None),
        )
        .with_chapters(1..=1),
    )
}

fn fleet(
    db: &Database,
    id: impl DatabaseIdLike<Fleet>,
    level: i32,
    ships: Vec<impl DatabaseIdLike<ShipBuild>>,
    rules: impl Into<Option<CombatRulesId>>,
) -> DbItem<Fleet> {
    Fleet {
        id: db.new_id(id),
        factions: Default::default(),
        level_bonus: 100 + level,
        no_random_ships: true,
        combat_time_limit: 0,
        loot_condition: RewardCondition::Never,
        exp_condition: RewardCondition::Always,
        specific_ships: ships.into_iter().map(|s| db.id(s)).collect(),
        no_ship_changing: true,
        player_has_one_ship: false,
        combat_rules: Some(rules.into().unwrap_or_else(|| db.id("rgl:basic"))),
    }
    .remember(db)
}
