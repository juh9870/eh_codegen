use std::ops::DerefMut;

use itertools::Itertools;

use eh_mod_cli::caching::loot_content::LootContentExt;
use eh_mod_cli::dev::database::{Database, Remember};
use eh_mod_cli::dev::mapping::DatabaseIdLike;
use eh_mod_cli::dev::schema::schema::{
    FleetId, Loot, LootContent, LootContentRandomItems, LootId, Quest, QuestItem, QuestItemId,
    QuestType, RequirementRandomStarSystem, StartCondition,
};
use quests::{MSG_CANCEL, MSG_CONTINUE};
use quests::quests::{NodeId, QuestContext, QuestContextData};
use quests::quests::branch::dialog::SmartDialog;

use crate::roguelite::events::{Event, EventKind, Events, WeightedVec};
use crate::roguelite::MSG_GONE_WRONG;

type Ctx<'a> = &'a mut QuestContextData;

pub fn core_quest(db: &Database) {
    init_chapter_event_items(db);
    init_cleaning_items(db);

    db.set_id::<QuestItem>(ITEM_RUN_ON_PROGRESS, 9999);
    db.forget_used_id::<QuestItem>(ITEM_RUN_ON_PROGRESS);

    db.new_quest_item(ITEM_RUN_ON_PROGRESS).with(|i| {
        i.with_name("run in progress")
            .with_description(
                "If your savefile is broken, sell this item and restart the game to end the run.",
            )
            .with_price(1)
    });

    db.new_quest_item(ITEM_PLAYER_DID_MOVE)
        .with(|i| i.with_name("ITEM_PLAYER_DID_MOVE"));

    encounter_quest(db);
    new_run_quest(db);
    startup_quest(db);
}

fn startup_quest(db: &Database) {
    let core_quest = db.id::<Quest>(QUEST_NEW_GAME);
    let mut ctx = QuestContext::new(db, "rgl:startup", "init");
    ctx.branch()
        .start_quest("init", core_quest)
        .cancel_quest()
        .entrypoint();

    let mut quest = ctx.into_quest();
    quest.quest_type = QuestType::Common;
    quest.start_condition = StartCondition::GameStart;
    quest.remember(db);
}

fn new_run_quest(db: &Database) {
    let mut ctx = QuestContext::new(db, QUEST_NEW_GAME, "init");

    new_game_quest(ctx.deref_mut());

    let mut quest = ctx.into_quest();
    quest.name = "Core Quest".to_string();
    quest.quest_type = QuestType::Common;
    quest.start_condition = StartCondition::Manual;
    quest.remember(db);
}

fn encounter_quest(db: &Database) {
    let mut ctx = QuestContext::new(db, QUEST_ENCOUNTER, "path_choices_init");
    path_choices_init(ctx.deref_mut());

    let mut quest = ctx.into_quest();
    quest.name = "Encounter".to_string();
    quest.quest_type = QuestType::Common;
    quest.start_condition = StartCondition::Manual;
    quest.use_random_seed = true;
    quest.remember(db);
}

fn item(db: &Database, id: impl DatabaseIdLike<QuestItem>) -> QuestItemId {
    db.id(id)
}

const CHAPTERS: usize = 5;

const ALL_EVENT_ITEMS_100: &str = "rgl:all_event_items";
const ALL_SHIPS_100: &str = "rgl:all_ships_100x";
const ALL_COMPONENTS_1000: &str = "rgl:all_components_1000x";

const LOOT_CHAPTER_EVENT: &str = "rgl:event_chapter_";

const ITEM_RUN_ON_PROGRESS: &str = "rgl:init_quest_lock";

pub const ITEM_PLAYER_DID_MOVE: &str = "rgl:player_did_move";

const ITEM_CHAPTER: &str = "rgl:chapter_indicator";
const LOOT_ITEM_CHAPTER: &str = "rgl:chapter_indicator";
const LOOT_ITEM_CHAPTER_100X: &str = "rgl:chapter_indicator_100x";

const QUEST_ENCOUNTER: &str = "rgl:encounter";
const QUEST_NEW_GAME: &str = "rgl:new_game";

fn loot_chapter(chapter: usize) -> impl DatabaseIdLike<Loot> {
    LOOT_CHAPTER_EVENT.to_string() + &chapter.to_string()
}

fn new_game_quest(ctx: Ctx) {
    let db = ctx.db.clone();
    let lock = item(&db, ITEM_RUN_ON_PROGRESS);
    ctx.branch()
        .switch("init", |c| {
            c.transition(1.0, lock.req_at_least(1), |ctx| {
                ctx.branch().cancel_quest().entrypoint()
            })
            .next_default()
        })
        .dialog("init_dialog", "Welcome to the <MODNAME>", |d| {
            d.action(
                ("Continue (technical limitation)", lock.req_at_least(1)),
                |c| c.branch().cancel_quest().entrypoint(),
            )
            .next((MSG_CONTINUE, lock.req_at_most(0)))
        })
        .receive_item("init_get_lock_item", lock.as_loot(1).loot(&db))
        .remove_item("init_clean_event_items", ALL_EVENT_ITEMS_100)
        .remove_item("init_clean_ships", ALL_SHIPS_100)
        .remove_item("init_clean_components", ALL_COMPONENTS_1000)
        .remove_item("init_clean_chapter_indicator", LOOT_ITEM_CHAPTER_100X)
        .receive_item("init_chapter_indicator", LOOT_ITEM_CHAPTER)
        .start_quest("branching_quest", QUEST_ENCOUNTER)
        .complete_quest()
        .entrypoint();
}

fn path_choices_init(ctx: Ctx) -> NodeId {
    ctx.cached("path_choices_init", |ctx| {
        ctx.branch()
            .remove_item("path_choices_init", ALL_EVENT_ITEMS_100)
            .random_end("path_events_item_init", |mut r| {
                let db = r.ctx().db.clone();
                for chapter in 1..=CHAPTERS {
                    r = r.transition(
                        1.0,
                        db.id::<QuestItem>(ITEM_CHAPTER).req_amount(chapter as i32),
                        |ctx| {
                            ctx.branch()
                                .receive_item(
                                    "path_choices_init_chapter_".to_string() + &chapter.to_string(),
                                    loot_chapter(chapter),
                                )
                                .goto(path_choice)
                                .entrypoint()
                        },
                    )
                }
                r.default(|ctx| something_gone_wrong(ctx, "Current chapter out of range"))
            })
            .entrypoint()
    })
}
fn path_choices_next(ctx: Ctx) -> NodeId {
    ctx.branch()
        .start_quest("start_next_encounter", QUEST_ENCOUNTER)
        .complete_quest()
        .entrypoint()
}

fn path_choice(ctx: Ctx) -> NodeId {
    ctx.cached("path_choice", |ctx| {
        let db = ctx.db.clone();
        ctx.branch()
            .dialog_end("path_choice", "Select a path", |mut d| {
                d = d.action("Change Loadout", edit_loadout);
                for event in db.extra::<Events>().read().iter() {
                    d = match &event.kind {
                        EventKind::Combat(fleet, loot) => event_combat(d, event, fleet, *loot),
                    }
                }
                d
            })
            .entrypoint()
    })
}

fn edit_loadout(ctx: Ctx) -> NodeId {
    // fn finish(ctx: Ctx) -> NodeId {
    //     ctx.cached("loadout_retreat", |ctx| {
    //         ctx.branch()
    //             .retreat("loadout_retreat")
    //             .goto(path_choice)
    //             .entrypoint()
    //     })
    // }
    let db = ctx.db.clone();
    let player_moved_item = item(&db, ITEM_PLAYER_DID_MOVE);
    ctx.branch()
        .remove_item(
            "loadout_clear_triggers",
            player_moved_item.as_loot(1000).loot(&db),
        )
        .switch_end("loadout_edit", |s| {
            s.message("Editing loadout. Fly to the target to continue your adventure")
                .transition(
                    1.0,
                    RequirementRandomStarSystem {
                        min_value: 1,
                        max_value: 1,
                        ..Default::default()
                    },
                    path_choice,
                )
                .transition(1.0, player_moved_item.req_at_least(1), path_choice)
        })
        .entrypoint()
}

fn win_combat(ctx: Ctx) -> NodeId {
    // TODO: rewards
    ctx.cached("win_combat", path_choices_next)
}

fn lose_combat(ctx: Ctx) -> NodeId {
    ctx.cached("lose_combat", |ctx| {
        ctx.branch()
            .dialog(
                "lose_combat",
                "This branch has no conclusion. Try again",
                |d| d.next(MSG_CONTINUE),
            )
            .goto(start_new_run)
            .entrypoint()
    })
}

fn start_new_run(ctx: Ctx) -> NodeId {
    ctx.cached("clear_run_marker", |ctx| {
        let db = ctx.db.clone();
        ctx.branch()
            .remove_item(
                "clear_run_marker",
                item(&db, ITEM_RUN_ON_PROGRESS).as_loot(1000).loot(&db),
            )
            .start_quest("start_new_game", db.get_id_raw(QUEST_NEW_GAME))
            .cancel_quest()
            .entrypoint()
    })
}

fn something_gone_wrong(ctx: Ctx, error: &str) -> NodeId {
    let id = format!("something_gone_wrong_{}", error);
    ctx.cached(id.clone(), |ctx| {
        ctx.branch()
            .dialog(id, MSG_GONE_WRONG.to_string() + error, |d| {
                d.next("Start new run")
            })
            .goto(start_new_run)
            .entrypoint()
    })
}

fn event_combat<'a, const N: bool>(
    d: SmartDialog<'a, N>,
    event: &Event,
    fleets: &WeightedVec<FleetId>,
    loot: Option<LootId>,
) -> SmartDialog<'a, N> {
    let event_id = event.quest_id();
    d.action((event.name(), event.item.req_at_least(1)), |ctx| {
        ctx.branch()
            .random_end(format!("{}_random", event_id), |mut r| {
                for (idx, fleet) in fleets.iter().enumerate() {
                    r = r.transition(fleet.weight, fleet.req.clone(), |ctx| {
                        ctx.branch()
                            .dialog(
                                format!("{}_{}_confirm", event_id, idx),
                                event.description(),
                                |d| {
                                    d.enemy(fleet.item)
                                        .next(MSG_CONTINUE)
                                        .action(MSG_CANCEL, path_choice)
                                },
                            )
                            .attack_fleet_end(
                                format!("{}_{}_combat", event_id, idx),
                                fleet.item,
                                loot,
                                win_combat,
                                lose_combat,
                            )
                            .entrypoint()
                    })
                }

                r.default(|ctx| {
                    something_gone_wrong(ctx, &format!("No fleets for event `{}`", event_id))
                })
            })
            .entrypoint()
    })
}

fn init_cleaning_items(db: &Database) {
    let items_to_remove = db
        .extra::<Events>()
        .read()
        .iter()
        .sorted_by_cached_key(|c| c.quest_id())
        .map(|event| event.item.as_loot(100).wrap_item(1.0))
        .collect_vec();

    db.new_loot(ALL_EVENT_ITEMS_100)
        .set_loot(LootContent::all_items().with_items(items_to_remove));

    let ships_to_remove = db.ship_iter(|ships| {
        ships
            .sorted_by_key(|c| i32::from(c.id))
            .map(|ship| ship.id.as_loot().repeat(100).wrap_item(1.0))
            .collect_vec()
    });

    db.new_loot(ALL_SHIPS_100)
        .set_loot(LootContent::all_items().with_items(ships_to_remove));

    let components_to_remove = db.component_iter(|components| {
        components
            .sorted_by_key(|c| i32::from(c.id))
            .map(|c| c.id.as_loot(1000).wrap_item(1.0))
            .collect_vec()
    });

    db.new_loot(ALL_COMPONENTS_1000)
        .set_loot(LootContent::all_items().with_items(components_to_remove));
}

fn init_chapter_event_items(db: &Database) {
    let events = db.extra::<Events>();
    let events = events.read();

    let ch_item = db.new_quest_item(ITEM_CHAPTER).edit(|i| {
        i.set_name("Chapter indicator")
            .set_description("Indicates the current chapter")
            .set_price(0);
    });

    db.new_loot(LOOT_ITEM_CHAPTER)
        .set_loot(ch_item.id.as_loot(1));
    db.new_loot(LOOT_ITEM_CHAPTER_100X)
        .set_loot(ch_item.id.as_loot(100));
    drop(ch_item);

    for chapter in 1..=CHAPTERS {
        let chapter_events = events
            .iter()
            .filter(|evt| !evt.chapters.as_ref().is_some_and(|c| !c.contains(&chapter)))
            .map(|evt| evt.item.as_loot(1).wrap_item(evt.weight))
            .collect_vec();

        db.new_loot(loot_chapter(chapter))
            .set_loot(LootContentRandomItems {
                min_amount: 1,
                max_amount: 1,
                items: chapter_events,
            });
    }
}
