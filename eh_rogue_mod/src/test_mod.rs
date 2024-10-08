use std::time::Instant;

use ahash::AHashMap;
use pretty_duration::pretty_duration;
use tracing::{debug, error_span, instrument};

use eh_mod_cli::db_vanilla::load_vanilla;
use eh_mod_cli::dev::database::{database, Database, Remember};
use eh_mod_cli::dev::helpers::from_json_string;
use eh_mod_cli::dev::json;
use eh_mod_cli::dev::reporting::report_diagnostics;
use eh_mod_cli::dev::schema::schema::{
    DatabaseSettings, Loot, LootContent, LootContentAllItems, LootContentMoney,
    LootContentQuestItem, LootContentStarMap, LootId, LootItem, Node, NodeAction,
    NodeCompleteQuest, NodeFailQuest, NodeReceiveItem, NodeShowDialog, Quest, QuestId, QuestItem,
    QuestType, Requirement, RequirementAll, RequirementHaveQuestItem, RequirementNone,
    StartCondition, Technology,
};
use eh_mod_cli::Args;

use crate::test_mod::quest_surgeon::next_id;

pub mod quest_surgeon;

#[instrument]
pub fn build_mod(args: Args) {
    let db = database(args.output_dir, args.output_mod);

    let start = Instant::now();

    load_vanilla(&db);

    debug!(
        time = pretty_duration(&start.elapsed(), None),
        "Loaded in base database"
    );

    db.get_singleton::<DatabaseSettings>().unwrap().edit(|i| {
        i.mod_name = "Rogue Horizon".to_string();
        i.mod_id = "rogue_horizon_v2".to_string();
        i.mod_version = 1;
    });

    db.add_id_range(9870000..9999999);

    let start = Instant::now();

    permadeath(&db);
    cheap_tech(&db);
    bonus_loot(&db);
    encounter_patches(&db);
    // debug(&db);

    debug!(
        time = pretty_duration(&start.elapsed(), None),
        "Applied mod changes"
    );

    let start = Instant::now();
    report_diagnostics(db.save());
    debug!(
        time = pretty_duration(&start.elapsed(), None),
        "Saved the resulting mod"
    );
}

#[instrument]
fn permadeath(db: &Database) {
    let death_item = QuestItem {
        id: db.new_id("roguelike:marker"),
        name: "Death mark".to_string(),
        description: "Game over".to_string(),
        icon: "scull".to_string(),
        color: "#000000".to_string(),
        price: 0,
    }
    .remember(db);

    let death_loot = Loot {
        id: db.new_id("roguelike:loot"),
        loot: LootContentQuestItem {
            item_id: death_item.id,
            min_amount: 1,
            max_amount: 1,
        }
        .into(),
    }
    .remember(db);

    let permadeath_quest = Quest {
        id: db.new_id("roguelike:lock_quest"),
        name: "Death".to_string(),
        quest_type: QuestType::Temporary,
        start_condition: StartCondition::LocalEncounter,
        weight: 1.0,
        origin: Default::default(),
        requirement: RequirementHaveQuestItem {
            item_id: Some(death_item.id),
            min_value: 1,
        }
        .into(),
        level: 0,
        use_random_seed: false,
        nodes: vec![
            NodeShowDialog {
                id: 1,
                required_view: Default::default(),
                message: "You are dead.\nStart new game".to_string(),
                enemy: None,
                loot: None,
                character: None,
                actions: vec![NodeAction {
                    target_node: 2,
                    requirement: Default::default(),
                    button_text: "$ACTION_Continue".to_string(),
                }],
            }
            .into(),
            json!(Node {
                "Id": 2,
                "Type": 30,
                "DefaultTransition": 3
            }),
            json!(Node {
                "Id": 3,
                "Type": 41
            }),
        ],
    }
    .remember(db);

    db.quest_iter_mut(|i| {
        for mut quest in i {
            if quest.id == permadeath_quest.id {
                continue;
            }
            if quest.id != db.id("eh:tutorial") {
                let req_no_marker = RequirementNone {
                    requirements: vec![RequirementHaveQuestItem {
                        item_id: Some(death_item.id),
                        min_value: 1,
                    }
                    .into()],
                }
                .into();
                if matches!(quest.requirement, Requirement::Empty(_)) {
                    quest.requirement = req_no_marker;
                } else {
                    let original_req = std::mem::take(&mut quest.requirement);
                    quest.requirement = RequirementAll {
                        requirements: vec![original_req, req_no_marker],
                    }
                    .into()
                }
            }

            let mut next_id = next_id(&quest);

            let mut extra_nodes = None::<(i32, Vec<Node>)>;

            let mut death_transition_id = || {
                if let Some((id, _)) = &extra_nodes {
                    return *id;
                }

                let dialog_node_id = next_id();
                let loot_node_id = next_id();
                let fail_node_id = next_id();
                let nodes: Vec<Node> = vec![
                    NodeShowDialog {
                        id: dialog_node_id,
                        required_view: Default::default(),
                        message: "You Died".to_string(),
                        enemy: None,
                        loot: Some(death_loot.id),
                        character: None,
                        actions: vec![NodeAction {
                            target_node: loot_node_id,
                            requirement: Default::default(),
                            button_text: "$ACTION_Continue".to_string(),
                        }],
                    }
                    .into(),
                    NodeReceiveItem {
                        id: loot_node_id,
                        default_transition: fail_node_id,
                        loot: Some(death_loot.id),
                    }
                    .into(),
                    NodeFailQuest { id: fail_node_id }.into(),
                ];

                extra_nodes = Some((dialog_node_id, nodes));

                dialog_node_id
            };

            for node in &mut quest.nodes {
                match node {
                    Node::AttackFleet(attack) => {
                        attack.failure_transition = death_transition_id();
                    }
                    Node::AttackOccupants(attack) => {
                        attack.failure_transition = death_transition_id();
                    }
                    Node::AttackStarbase(attack) => {
                        attack.failure_transition = death_transition_id();
                    }
                    _ => {}
                }
            }
            if let Some((_, nodes)) = extra_nodes {
                // info!(quest_id = quest.id.0, "Adding death paths");
                quest.nodes.extend(nodes)
            }
        }
    });
}

#[instrument]
fn debug(db: &Database) {
    let debug_loot = Loot {
        id: db.new_id("debug:starting_loot"),
        loot: LootContentAllItems {
            items: vec![
                LootItem {
                    weight: 0.0,
                    loot: LootContentStarMap {}.into(),
                },
                LootItem {
                    weight: 0.0,
                    loot: LootContentMoney {
                        min_amount: 999999,
                        max_amount: 999999,
                    }
                    .into(),
                },
            ],
        }
        .into(),
    }
    .remember(db);
    let _ = Quest {
        id: db.new_id("debug:starting_boost"),
        name: "Debug".to_string(),
        quest_type: QuestType::Storyline,
        start_condition: StartCondition::GameStart,
        weight: 1.0,
        origin: Default::default(),
        requirement: Default::default(),
        level: 0,
        use_random_seed: false,
        nodes: vec![
            NodeShowDialog {
                id: 1,
                required_view: Default::default(),
                message: "DEBUG ITEMS".to_string(),
                enemy: None,
                loot: Some(debug_loot.id),
                character: None,
                actions: vec![NodeAction {
                    target_node: 2,
                    requirement: Default::default(),
                    button_text: "$ACTION_Continue".to_string(),
                }],
            }
            .into(),
            NodeReceiveItem {
                id: 2,
                default_transition: 3,
                loot: Some(debug_loot.id),
            }
            .into(),
            NodeCompleteQuest { id: 3 }.into(),
        ],
    }
    .remember(db);
}

#[instrument]
fn upgrade_loot(loot: &mut LootContent, multiplier: f32) {
    let times = |n: i32| -> i32 { (n as f32 * multiplier) as i32 };
    match loot {
        LootContent::None(_) => {}
        LootContent::SomeMoney(m) => {
            m.value_ratio *= multiplier * multiplier;
            m.value_ratio = m.value_ratio.min(1000.0);
        }
        LootContent::Fuel(_) => {}
        LootContent::Money(m) => {
            m.min_amount = times(m.min_amount);
            m.max_amount = times(m.max_amount);
        }
        LootContent::Stars(s) => {
            s.min_amount = times(s.min_amount);
            s.max_amount = times(s.max_amount);
        }
        LootContent::StarMap(_) => {}
        LootContent::RandomComponents(c) => {
            c.min_amount = times(c.min_amount);
            c.max_amount = times(c.max_amount);
            c.value_ratio *= multiplier * multiplier;
        }
        LootContent::RandomItems(i) => {
            // Only upgrade inner loot, not min/max amounts
            for item in &mut i.items {
                upgrade_loot(&mut item.loot, multiplier)
            }
        }
        LootContent::AllItems(i) => {
            for item in &mut i.items {
                upgrade_loot(&mut item.loot, multiplier)
            }
        }
        LootContent::ItemsWithChance(i) => {
            for item in &mut i.items {
                upgrade_loot(&mut item.loot, multiplier)
            }
        }
        LootContent::QuestItem(i) => {
            i.min_amount = times(i.min_amount);
            i.max_amount = times(i.max_amount);
        }
        LootContent::Ship(_) => {}
        LootContent::EmptyShip(_) => {}
        LootContent::Component(c) => {
            c.min_amount = times(c.min_amount);
            c.max_amount = times(c.max_amount);
        }
        LootContent::Blueprint(_) => {}
        LootContent::ResearchPoints(rp) => {
            rp.min_amount = times(rp.min_amount);
            rp.max_amount = times(rp.max_amount);
        }
        LootContent::Satellite(sat) => {
            sat.min_amount = times(sat.min_amount);
            sat.max_amount = times(sat.max_amount);
        }
    }
}

#[instrument]
fn bonus_loot(db: &Database) {
    let mults = vec![
        ("eh:civilian_ship_reward", 20.0),
        ("eh:covid_loot", 10.0),
        ("eh:merchant_goods", 10.0),
        ("eh:random_resources", 10.0),
        // ("eh:random_stuff", 20.0),
        ("eh:scavenger_goods", 10.0),
        ("eh:some_money", 20.0),
        ("eh:some_money_x5", 20.0),
        ("eh:worm_boss_loot", 20.0),
    ];

    for (id, mult) in mults {
        let _guard = error_span!("Loot", id, mult).entered();
        let loot = db.get_item::<Loot>(id).unwrap();
        let mut loot = loot.write();
        upgrade_loot(&mut loot.loot, mult);
    }

    let merchant_loot = db.get_item::<Loot>("eh:merchant_loot").unwrap();
    let mut merchant_loot = merchant_loot.write();
    merchant_loot.loot = from_json_string(include_str!("merchant_loot.json"));

    let random_stuff = db.get_item::<Loot>("eh:random_stuff").unwrap();
    let mut random_stuff = random_stuff.write();
    random_stuff.loot = from_json_string(include_str!("random_stuff.json"));
}

#[instrument]
fn cheap_tech(db: &Database) {
    db.iter_mut::<Technology, _>(|i| {
        for mut x in i {
            let price = x.price_mut();
            if *price > 0 {
                *price /= 5;
                *price = (*price).max(1);
            }

            let hidden = x.hidden_mut();
            if *hidden {
                *hidden = false;
                *x.price_mut() = 5;
            }
        }
    });
}

#[instrument]
fn encounter_patches(db: &Database) {
    let mut scavenger_loot = db.new_loot("roguelike:scavenger_loot");
    scavenger_loot.loot = from_json_string(include_str!("scav_loot.json"));

    let patch_combat_encounters = |quest: QuestId, reward: LootId| {
        let quest = db.get_item::<Quest>(quest).unwrap();
        let mut quest = quest.write();

        let mut next_id = next_id(&quest);

        let mut extra_nodes: Vec<Node> = vec![];

        let mut transitions = AHashMap::<i32, i32>::default();

        let mut reward_node = |transition: i32| {
            if let Some(&node_id) = transitions.get(&transition) {
                return node_id;
            }

            let dialog_node_id = next_id();
            let reward_node_id = next_id();
            extra_nodes.push(
                NodeShowDialog {
                    id: dialog_node_id,
                    required_view: Default::default(),
                    message: "$MessageCombatReward".to_string(),
                    enemy: None,
                    loot: Some(reward),
                    character: None,
                    actions: vec![NodeAction {
                        target_node: reward_node_id,
                        requirement: Default::default(),
                        button_text: "$ACTION_Continue".to_string(),
                    }],
                }
                .into(),
            );
            extra_nodes.push(
                NodeReceiveItem {
                    id: reward_node_id,
                    default_transition: transition,
                    loot: Some(reward),
                }
                .into(),
            );
            transitions.insert(transition, dialog_node_id);
            dialog_node_id
        };

        for node in &mut quest.nodes {
            match node {
                Node::AttackFleet(attack) => {
                    attack.default_transition = reward_node(attack.default_transition);
                }
                Node::DestroyOccupants(attack) => {
                    attack.default_transition = reward_node(attack.default_transition);
                }
                Node::AttackStarbase(attack) => {
                    attack.default_transition = reward_node(attack.default_transition);
                }
                _ => {}
            }
        }

        quest.nodes.extend(extra_nodes);
    };

    patch_combat_encounters(db.id("eh:scavenger_trade"), scavenger_loot.id);
    patch_combat_encounters(db.id("eh:local_pirates"), db.id("eh:random_stuff"));
}
