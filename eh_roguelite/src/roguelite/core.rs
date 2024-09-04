use itertools::Itertools;

use eh_mod_cli::dev::database::{Database, DbItem, Remember};
use eh_mod_cli::dev::mapping::DatabaseIdLike;
use eh_mod_cli::dev::schema::schema::{
    Loot, LootContent, LootContentRandomItems, Quest, QuestId, QuestItem, QuestItemId, QuestType,
    StartCondition,
};
use quests::quests::branch::{BranchBuilder, BranchDone};
use quests::quests::{IntoNodeId, NodeId, QuestContext, QuestContextData};

use crate::roguelite::core::new_game::startup;
use crate::roguelite::events::Events;

type Ctx<'a> = &'a mut QuestContextData;

mod encounter;
mod gone_wrong;
mod new_game;

fn quest(
    db: &Database,
    id: impl Into<String>,
    name: impl Into<String>,
    func: impl FnOnce(Ctx) -> NodeId,
) -> DbItem<Quest> {
    let mut ctx = QuestContext::new(db, id, "init");
    func(&mut ctx);
    let mut quest = ctx.into_quest();
    quest.quest_type = QuestType::Common;
    quest.start_condition = StartCondition::Manual;
    quest.name = name.into();

    quest.remember(db)
}

fn goto(id: impl IntoNodeId) -> impl FnOnce(Ctx) -> NodeId {
    |ctx| ctx.id(id)
}

trait BBExt {
    fn goto_quest(
        self,
        id: impl Into<String>,
        quest: impl FnOnce(&Database) -> QuestId,
    ) -> BranchDone;
}

impl<'a> BBExt for BranchBuilder<'a> {
    fn goto_quest(
        self,
        id: impl Into<String>,
        quest: impl FnOnce(&Database) -> QuestId,
    ) -> BranchDone {
        self.goto(|ctx| {
            let id = id.into();
            ctx.cached(id.clone(), |ctx| {
                let qid = quest(&ctx.db);
                ctx.branch()
                    .start_quest(id, qid)
                    .cancel_quest()
                    .entrypoint()
            })
        })
    }
}

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

    startup(db);
}

fn item(db: &Database, id: impl DatabaseIdLike<QuestItem>) -> QuestItemId {
    db.id(id)
}

const CHAPTERS: usize = 5;

const ALL_EVENT_ITEMS_100: &str = "rgl:all_event_items";
const ALL_SHIPS_100: &str = "rgl:all_ships_100x";
const ALL_COMPONENTS_1000: &str = "rgl:all_components_1000x";

const LOOT_CHAPTER_EVENT: &str = "rgl:events/chapter_";

const ITEM_RUN_ON_PROGRESS: &str = "rgl:init_quest_lock";

pub const ITEM_PLAYER_DID_MOVE: &str = "rgl:player_did_move";

const ITEM_CHAPTER: &str = "rgl:chapter_indicator";
const LOOT_ITEM_CHAPTER: &str = "rgl:chapter_indicator";
const LOOT_ITEM_CHAPTER_100X: &str = "rgl:chapter_indicator_100x";

fn loot_chapter(chapter: usize) -> impl DatabaseIdLike<Loot> {
    LOOT_CHAPTER_EVENT.to_string() + &chapter.to_string()
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
