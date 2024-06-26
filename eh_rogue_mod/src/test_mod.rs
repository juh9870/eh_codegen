use crate::Args;
use eh_mod_dev::database::{database, Database};

pub fn build_mod(args: Args) {
    let db = database(args.output_dir);

    db.load_from_dir(args.vanilla_dir);

    db.add_id_range(9870000..9999999);

    parametric_ammo(&db);

    db.save();
}

fn parametric_ammo(_db: &Database) {}
