use std::ops::DerefMut;

use itertools::Itertools;

use eh_mod_cli::dev::database::{Database, Remember};
use eh_mod_cli::dev::mapping::DatabaseIdLike;
use eh_mod_cli::dev::schema::schema::{
    FleetId, Loot, LootContent, LootContentRandomItems, LootId, QuestItem, QuestType,
    StartCondition,
};
use quests::{MSG_CANCEL, MSG_CONTINUE};
use quests::quests::{NodeId, QuestContext, QuestContextData};
use quests::quests::branch::dialog::SmartDialog;

use crate::roguelite::events::{Event, EventKind, Events, WeightedVec};
use crate::roguelite::MSG_GONE_WRONG;

type Ctx<'a> = &'a mut QuestContextData;

pub fn core_quest(db: &Database) {
    let mut ctx = QuestContext::new(db, "rgl:core", "init");

    init_chapter_event_items(db);
    init_cleaning_items(db);

    init_raw(ctx.deref_mut());

    let mut quest = ctx.into_quest();
    quest.quest_type = QuestType::Singleton;
    quest.start_condition = StartCondition::GameStart;
    quest.remember(db);
}

const CHAPTERS: usize = 5;

const ALL_EVENT_ITEMS_100: &str = "rgl:all_event_items";
const ALL_SHIPS_100: &str = "rgl:all_ships_100x";
const ALL_COMPONENTS_1000: &str = "rgl:all_components_1000x";

const LOOT_CHAPTER_EVENT: &str = "rgl:event_chapter_";

const ITEM_CHAPTER: &str = "rgl:chapter_indicator";
const LOOT_ITEM_CHAPTER: &str = "rgl:chapter_indicator";

fn loot_chapter(chapter: usize) -> impl DatabaseIdLike<Loot> {
    LOOT_CHAPTER_EVENT.to_string() + &chapter.to_string()
}

fn init_raw(ctx: Ctx) {
    ctx.branch()
        .dialog("init", "Welcome to the <MODNAME>", |d| d.next(MSG_CONTINUE))
        .remove_item("init_clean_event_items", ALL_EVENT_ITEMS_100)
        .remove_item("init_clean_ships", ALL_SHIPS_100)
        .remove_item("init_clean_components", ALL_COMPONENTS_1000)
        .receive_item("init_chapter_indicator", LOOT_ITEM_CHAPTER)
        .goto(path_choices_init)
        .entrypoint();
}

fn init(ctx: Ctx) -> NodeId {
    ctx.id("init")
}

fn path_choices_init(ctx: Ctx) -> NodeId {
    ctx.cached("path_choices_init", |ctx| {
        ctx.branch()
            .random_end("path_choices_init", |mut r| {
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

fn path_choice(ctx: Ctx) -> NodeId {
    ctx.cached("path_choice", |ctx| {
        let db = ctx.db.clone();
        ctx.branch()
            .dialog_end("path_choice", "Select a path", |mut d| {
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

fn win_combat(ctx: Ctx) -> NodeId {
    // TODO: rewards
    ctx.cached("win_combat", path_choices_init)
}

fn lose_combat(ctx: Ctx) -> NodeId {
    ctx.cached("lose_combat", |ctx| {
        ctx.branch()
            .dialog(
                "lose_combat",
                "This branch has no conclusion. Try again",
                |d| d.next(MSG_CONTINUE),
            )
            .goto(init)
            .entrypoint()
    })
}

fn something_gone_wrong(ctx: Ctx, error: &str) -> NodeId {
    let id = format!("something_gone_wrong_{}", error);
    ctx.cached(id.clone(), |ctx| {
        ctx.branch()
            .dialog(id, MSG_GONE_WRONG.to_string() + &error, |d| {
                d.next("Start new run")
            })
            .goto(init)
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
            .dialog(format!("{}_confirm", event_id), event.description(), |d| {
                d.next(MSG_CONTINUE).action(MSG_CANCEL, path_choice)
            })
            .random_end(format!("{}_random", event_id), |mut r| {
                for fleet in fleets {
                    r = r.transition(fleet.weight, fleet.req.clone(), |ctx| {
                        ctx.branch()
                            .attack_fleet_end(
                                format!("{}_combat", event_id),
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
