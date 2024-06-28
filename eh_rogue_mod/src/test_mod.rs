use crate::Args;
use eh_mod_dev::database::{database, Database, Remember};
use eh_mod_dev::json;
use eh_mod_dev::schema::schema::{
    Loot, LootContentAllItems, LootContentMoney, LootContentQuestItem, LootContentStarMap,
    LootItem, Node, NodeAction, NodeCompleteQuest, NodeFailQuest, NodeReceiveItem, NodeShowDialog,
    Quest, QuestItem, QuestType, Requirement, RequirementAll, RequirementHaveQuestItem,
    RequirementNone, StartCondition,
};
use std::collections::HashSet;
use tracing::info;

pub fn build_mod(args: Args) {
    let db = database(args.output_dir);

    db.load_from_dir(args.vanilla_dir);

    db.add_id_range(9870000..9999999);

    permadeath(&db);
    // debug(&db);

    db.save();
}

fn permadeath(db: &Database) {
    let death_item = QuestItem {
        id: db.id("permadeath:marker"),
        name: "Death mark".to_string(),
        description: "Game over".to_string(),
        icon: "scull".to_string(),
        color: "#000000".to_string(),
        price: 0,
    }
    .remember(db);

    let death_loot = Loot {
        id: db.id("permadeath:loot"),
        loot: LootContentQuestItem {
            item_id: death_item.id,
            min_amount: 1,
            max_amount: 1,
        }
        .into(),
    }
    .remember(db);

    let permadeath_quest = Quest {
        id: db.id("permadeath:lock_quest"),
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
        let mut processed_amount = 0;
        for mut quest in i {
            if quest.id == permadeath_quest.id || quest.id.0 == 1 {
                continue;
            }
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

            let nodes: HashSet<i32> = quest.nodes.iter().map(|n| n.id()).copied().collect();
            let mut last_id = 0;

            let mut next_id = move || {
                while last_id < 999999 {
                    last_id += 1;
                    if !nodes.contains(&last_id) {
                        return last_id;
                    }
                }
                panic!("Out of IDs")
            };

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
                info!(quest_id = quest.id.0, "Adding death paths");
                quest.nodes.extend(nodes)
            }
            processed_amount += 1;
        }
    });
}

fn debug(db: &Database) {
    let debug_loot = Loot {
        id: db.id("debug:starting_loot"),
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
        id: db.id("debug:starting_boost"),
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
