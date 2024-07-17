use eh_schema::schema::{Loot, Quest, Ship, ShipBuild};

use crate::database::Database;

pub fn add_vanilla_mappings(db: &Database) {
    db.set_id::<Quest>("eh:local_pirates", 5);
    db.set_id::<Quest>("eh:capture_starbase", 9);
    db.set_id::<Quest>("eh:scavenger_trade", 105);
    db.set_id::<Quest>("eh:scavenger_distress", 106);
    db.set_id::<Quest>("eh:scavenger_harbor", 107);
    db.set_id::<Quest>("eh:jansalo_into", 100);
    db.set_id::<Quest>("eh:jansalo_fuel", 101);
    db.set_id::<Quest>("eh:jansalo_combat", 102);
    db.set_id::<Quest>("eh:escapepod", 4);
    db.set_id::<Quest>("eh:freestuff", 2);
    db.set_id::<Quest>("eh:merchant", 6);
    db.set_id::<Quest>("eh:pirates", 3);
    db.set_id::<Quest>("eh:ship_out_of_fuel", 8);
    db.set_id::<Quest>("eh:wormship", 7);
    db.set_id::<Quest>("eh:fac_pirates", 20);
    db.set_id::<Quest>("eh:fac_resources", 21);
    db.set_id::<Quest>("eh:fac_delivery", 22);
    db.set_id::<Quest>("eh:easter", 10);
    db.set_id::<Quest>("eh:pandemic", 200);
    db.set_id::<Quest>("eh:tutorial", 1);

    db.set_id::<Loot>("eh:civilian_ship_reward", 17);
    db.set_id::<Loot>("eh:covid_loot", 21);
    db.set_id::<Loot>("eh:merchant_goods", 6);
    db.set_id::<Loot>("eh:merchant_loot", 5);
    db.set_id::<Loot>("eh:random_resources", 8);
    db.set_id::<Loot>("eh:random_stuff", 3);
    db.set_id::<Loot>("eh:scavenger_goods", 16);
    db.set_id::<Loot>("eh:some_money", 1);
    db.set_id::<Loot>("eh:some_money_x5", 10);
    db.set_id::<Loot>("eh:worm_boss_loot", 7);

    veniri(db);
}

fn veniri(db: &Database) {
    db.set_id::<Ship>("eh:scout", 17);
    db.set_id::<ShipBuild>("eh:scout", 39);
    db.set_id::<ShipBuild>("eh:scout_x", 106);
    db.set_id::<ShipBuild>("eh:scout_x2", 107);

    db.set_id::<Ship>("eh:scout_mk2", 18);
    db.set_id::<ShipBuild>("eh:scout_mk2", 40);
    db.set_id::<ShipBuild>("eh:scout_mk2_x", 108);
    db.set_id::<ShipBuild>("eh:scout_mk2_xx", 235);

    db.set_id::<Ship>("eh:paladin", 19);
    db.set_id::<ShipBuild>("eh:paladin", 41);
    db.set_id::<ShipBuild>("eh:paladin_x", 109);
    db.set_id::<ShipBuild>("eh:paladin_x2", 194);
    db.set_id::<ShipBuild>("eh:paladin_xx", 163);

    db.set_id::<Ship>("eh:javelin", 20);
    db.set_id::<ShipBuild>("eh:javelin", 42);
    db.set_id::<ShipBuild>("eh:javelin_x", 110);

    db.set_id::<Ship>("eh:excalibur", 21);
    db.set_id::<ShipBuild>("eh:excalibur", 43);
    db.set_id::<ShipBuild>("eh:excalibur_x", 111);
    db.set_id::<ShipBuild>("eh:excalibur_xx", 164);

    db.set_id::<Ship>("eh:dart", 22);
    db.set_id::<ShipBuild>("eh:dart", 44);
    db.set_id::<ShipBuild>("eh:dart_x", 112);
}
