use eh_mod_cli::caching::loot_content::LootContentExt;
use eh_mod_cli::dev::database::Database;
use eh_mod_cli::dev::schema::schema::{Quest, QuestId, StartCondition};
use quests::MSG_CONTINUE;

use crate::roguelite::core::encounter::new_encounter;
use crate::roguelite::core::{
    item, quest, BBExt, ALL_COMPONENTS_1000, ALL_EVENT_ITEMS_100, ALL_SHIPS_100,
    ITEM_RUN_ON_PROGRESS, LOOT_ITEM_CHAPTER, LOOT_ITEM_CHAPTER_100X,
};

const QUEST_STARTUP: &str = "rgl:startup";
const QUEST_NEW_GAME: &str = "rgl:new_game";
const QUEST_END_GAME: &str = "rgl:end_game";

pub fn startup(db: &Database) -> QuestId {
    db.cached::<Quest>(QUEST_STARTUP, || {
        quest(db, QUEST_STARTUP, "Startup", |ctx| {
            ctx.branch().goto_quest("init", new_game_init).entrypoint()
        })
        .edit(|q| {
            q.start_condition = StartCondition::GameStart;
        })
        .id
    })
}

fn new_game_init(db: &Database) -> QuestId {
    db.cached::<Quest>(QUEST_NEW_GAME, || {
        quest(db, QUEST_NEW_GAME, "New Game Startup", |ctx| {
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
                .goto_quest("branching_quest", new_encounter)
                .entrypoint()
        })
        .id
    })
}

pub fn end_game_start_new(db: &Database) -> QuestId {
    db.cached::<Quest>(QUEST_END_GAME, || {
        quest(db, QUEST_END_GAME, "End of run", |ctx| {
            ctx.cached("init", |ctx| {
                let db = ctx.db.clone();
                ctx.branch()
                    .remove_item(
                        "init",
                        item(&db, ITEM_RUN_ON_PROGRESS).as_loot(1000).loot(&db),
                    )
                    .start_quest("start_new_game", db.get_id_raw(QUEST_NEW_GAME))
                    .cancel_quest()
                    .entrypoint()
            })
        })
        .id
    })
}
