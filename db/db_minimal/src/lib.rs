use eh_mod_dev::database::Database;
use eh_mod_dev::schema::schema::{Device, Faction, GameObjectPrefab, Loot, Quest, Ship, ShipBuild};

pub fn load_minimal(db: &Database) {
    static DB: include_dir::Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/minimal");
    db.load_from_included_dir(&DB);

    add_minimal_mappings(db);
}

pub fn add_minimal_mappings(db: &Database) {
    db.set_id::<Device>("eh:toxic_waste", 18);
    db.set_id::<Faction>("eh:default", 1);
    db.set_id::<Faction>("eh:infected", 99);

    db.set_id::<GameObjectPrefab>("eh:worm_tail_segment", 1);
    db.set_id::<GameObjectPrefab>("eh:energy_shield", 2);
    db.set_id::<GameObjectPrefab>("eh:energy_shield_outline", 3);

    db.set_id::<Loot>("eh:starting_inventory", 1);
    db.set_id::<Quest>("eh:local_pirates", 2);
    db.set_id::<Quest>("eh:capture_starbase", 1);

    db.set_id::<Ship>("eh:outpost", 1);
    db.set_id::<Ship>("eh:turret", 2);
    db.set_id::<Ship>("eh:hive", 3);
    db.set_id::<Ship>("eh:supporter_pack_ship", 4);
    db.set_id::<Ship>("eh:starbase", 5);
    db.set_id::<Ship>("eh:mothership", 83);

    db.set_id::<ShipBuild>("eh:hive", 3);
    db.set_id::<ShipBuild>("eh:supporter_pack_ship", 4);
    db.set_id::<ShipBuild>("eh:starbase_default", 5);
    db.set_id::<ShipBuild>("eh:mothership", 220);
}
