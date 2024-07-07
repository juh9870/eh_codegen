use std::path::PathBuf;

use eh_mod_dev::database::{database, Database, DatabaseIdLike, DbItem};
use eh_schema::schema::{
    ActivationType, Ammunition, BulletBody, BulletController, BulletControllerParametric,
    BulletImpactType, BulletPrefab, BulletPrefabId, BulletTrigger, BulletTriggerCondition,
    CellType, ColorMode, ComponentStats, DamageType, ImpactEffect, ImpactEffectType, Weapon,
    WeaponClass, WeaponSlotType,
};

pub fn build_mod(mod_dir: PathBuf) {
    let db = database(mod_dir, None::<PathBuf>);

    db.add_id_range(9870000..9999999);
    db.set_id::<BulletPrefab>("eh:mine", 9);
    db.set_id::<BulletPrefab>("eh:rocket", 3);
    db.set_id::<BulletPrefab>("eh:proton_torpedo", 4);
    db.set_id::<ComponentStats>("eh:weapon", 1);

    parametric_ammo(&db);

    db.save();
}

fn parametric_ammo(db: &Database) {
    let (left, right) = sine_ammo(
        db,
        "juh9870:parametric",
        0.66,
        2.0,
        |ammo| {
            ammo.body = simple_body(db.id("eh:rocket"), 100.0);
            ammo.body.attached_to_parent = true;
            ammo.effects.push(damage(DamageType::Heat, 10.0));
        },
        |_c| {
            // c.set_size("(10.5 - t) / 10");
        },
    );

    let root = db.ammunition("juh9870:parametric_root").edit(|ammo| {
        ammo.body = simple_body(None, 10.0);
        ammo.body.velocity = 10.0;
        ammo.body.parent_velocity_effect = 0.0;

        ammo.impact_type = BulletImpactType::HitAllTargets;

        ammo.triggers.push(
            BulletTrigger::spawn_bullet()
                .with_condition(BulletTriggerCondition::Created)
                .with_ammunition(left.id)
                .with_quantity(1)
                .wrap(),
        );
        ammo.triggers.push(
            BulletTrigger::spawn_bullet()
                .with_condition(BulletTriggerCondition::Created)
                .with_ammunition(right.id)
                .with_quantity(1)
                .wrap(),
        );
    });

    let _repeater = db.ammunition("juh9870:parametric_repeater").edit(|ammo| {
        ammo.body = simple_body(None, 5.0);
        ammo.body.parent_velocity_effect = 0.0;

        ammo.impact_type = BulletImpactType::HitAllTargets;

        ammo.triggers.push(
            BulletTrigger::spawn_bullet()
                .with_condition(BulletTriggerCondition::Cooldown)
                .with_cooldown(0.1)
                .with_ammunition(root.id)
                .with_quantity(1)
                .wrap(),
        );
    });

    let boolet = db.ammunition("juh9870:boolet").with(|a| {
        a.with_body(simple_body(db.id("eh:proton_torpedo"), 1000.0).with_attached_to_parent(true))
            .with_effects(vec![damage(DamageType::Corrosive, 1.0)])
    });

    let square = db
        .ammunition("juh9870:square")
        .with(|a| a.with_body(simple_body(None, 20.0)))
        .edit(|a| {
            let w = 5;
            let h = 5;
            let size = 15;
            let sx = size / w;
            let sy = size / h;
            for y in 0..h {
                for x in 0..w {
                    if x != 0 && x != w - 1 && y != 0 && y != h - 1 {
                        continue;
                    }
                    let px = x * sx - size / 2 + sx / 2;
                    let py = y * sy - size / 2 + sy / 2;
                    a.triggers.push(
                        BulletTrigger::spawn_bullet()
                            .with_condition(BulletTriggerCondition::Created)
                            .with_ammunition(boolet.id)
                            .with_offset_x(px.to_string())
                            .with_offset_y(py.to_string())
                            .with_size(5.0)
                            .with_color("#FF0000")
                            .with_color_mode(ColorMode::UseMyOwn)
                            .with_rotation("0")
                            .wrap(),
                    )
                }
            }

            a.impact_type = BulletImpactType::HitAllTargets;

            a.controller = BulletController::parametric()
                .with_rotation("60 * t")
                .with_x("t * 10")
                .with_y("SIN(t * 2) * 10")
                .wrap();
        });

    db.component("juh9870:parametric", "eh:weapon").with(|c| {
        c.with_ammunition_id(square.id)
            .with_weapon_id(weapon(db, "juh9870:parametric", 1.0).id)
            .with_layout("1")
            .with_name("SineShooter")
            .with_cell_type(CellType::Weapon.to_string())
            .with_icon("gun1")
            .with_color("#FFFFFFFF")
            .with_weapon_slot_type(WeaponSlotType::Special.to_string())
    });
}

fn sine_ammo(
    db: &Database,
    id: &str,
    period: f32,
    magnitude: f32,
    edit: impl Fn(&mut Ammunition),
    param_edit: impl Fn(&mut BulletControllerParametric),
) -> (DbItem<Ammunition>, DbItem<Ammunition>) {
    let period = std::f32::consts::PI / period;
    let y = format!("SIN(t * {period}) * {magnitude}");
    let rotation = format!("COS(t * {period}) * {}", 180.0 / std::f32::consts::PI);
    let left = db.ammunition(format!("{id}_left")).edit(|ammo| {
        edit(ammo);
        let mut controller = BulletController::parametric()
            .with_y(y.clone())
            .with_rotation(rotation.clone());
        param_edit(&mut controller);
        ammo.controller = controller.into();
    });
    let right = db.ammunition(format!("{id}_right")).edit(|ammo| {
        edit(ammo);

        let mut controller = BulletController::parametric()
            .with_y(format!("-{y}"))
            .with_rotation(format!("-{rotation}"));
        param_edit(&mut controller);
        ammo.controller = controller.into();
    });
    (left, right)
}

fn damage(ty: DamageType, damage: impl Into<f32>) -> ImpactEffect {
    ImpactEffect {
        r#type: ImpactEffectType::Damage,
        damage_type: ty,
        power: damage.into(),
        factor: 0.0,
    }
}

fn simple_body(prefab: impl Into<Option<BulletPrefabId>>, lifetime: impl Into<f32>) -> BulletBody {
    BulletBody::new()
        .with_lifetime(lifetime)
        .with_color("#FFFFFFFF")
        .with_size(1.0)
        .with_bullet_prefab(prefab)
}

fn weapon(db: &Database, id: impl DatabaseIdLike<Weapon>, interval: f32) -> DbItem<Weapon> {
    let w = Weapon {
        id: id.into_id(db),
        weapon_class: WeaponClass::Common,
        fire_rate: 1.0 / interval,
        spread: 0.0,
        magazine: 0,
        activation_type: ActivationType::Manual,
        shot_sound: "shot_01".to_string(),
        charge_sound: "".to_string(),
        shot_effect_prefab: "FlashAdditive".to_string(),
        visual_effect: None,
        effect_size: 1.0,
        control_button_icon: "controls_shot".to_string(),
    };
    db.add_item(w)
}
