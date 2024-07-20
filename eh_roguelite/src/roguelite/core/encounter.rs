use eh_mod_cli::caching::loot_content::LootContentExt;
use eh_mod_cli::dev::database::Database;
use eh_mod_cli::dev::schema::schema::{FleetId, LootId, Quest, QuestId, QuestItem};
use quests::quests::branch::dialog::SmartDialog;
use quests::quests::NodeId;
use quests::{MSG_CANCEL, MSG_CONTINUE};

use crate::roguelite::core::gone_wrong::something_gone_wrong;
use crate::roguelite::core::new_game::end_game_start_new;
use crate::roguelite::core::{
    goto, item, loot_chapter, quest, BBExt, Ctx, ALL_EVENT_ITEMS_100, CHAPTERS, ITEM_CHAPTER,
    ITEM_PLAYER_DID_MOVE,
};
use crate::roguelite::events::{Event, EventKind, Events, WeightedVec};

const QUEST_ENCOUNTER_INIT: &str = "rgl:encounter_init";
const QUEST_ENCOUNTER_CHOICE: &str = "rgl:encounter_choice";
const QUEST_ENCOUNTER_CANCEL_BUTTON: &str = "rgl:encounter_cancel_button";
const QUEST_ENCOUNTER_REWARDS: &str = "rgl:encounter_rewards";

const QUEST_ENCOUNTER_COMBAT_: &str = "rgl:encounter_combat_";

const ITEM_RESUME_BUTTON_INDICATOR: &str = "rgl:resume_button_indicator";

pub fn new_encounter(db: &Database) -> QuestId {
    db.cached::<Quest>(QUEST_ENCOUNTER_INIT, || {
        db.new_quest_item(ITEM_RESUME_BUTTON_INDICATOR)
            .with(|i| i.with_name("Editing loadout"));

        quest(
            db,
            QUEST_ENCOUNTER_INIT,
            "Encounter initialization",
            |ctx| {
                ctx.branch()
                    .remove_item("init", ALL_EVENT_ITEMS_100)
                    .random_end("path_events_item_init", |mut r| {
                        let db = r.ctx().db.clone();
                        for chapter in 1..=CHAPTERS {
                            r = r.transition(
                                1.0,
                                db.id::<QuestItem>(ITEM_CHAPTER).req_amount(chapter as i32),
                                |ctx| {
                                    ctx.branch()
                                        .receive_item(
                                            "path_choices_init_chapter_".to_string()
                                                + &chapter.to_string(),
                                            loot_chapter(chapter),
                                        )
                                        .goto_quest("goto_path_choice", path_choice)
                                        .entrypoint()
                                },
                            )
                        }
                        r.default(|ctx| something_gone_wrong(ctx, "Current chapter out of range"))
                    })
                    .entrypoint()
            },
        )
        .id
    })
}

fn path_choice(db: &Database) -> QuestId {
    fn edit_loadout(ctx: Ctx) -> NodeId {
        let db = ctx.db.clone();
        let player_moved_item = item(&db, ITEM_PLAYER_DID_MOVE);
        let resume_button_item = item(&db, ITEM_RESUME_BUTTON_INDICATOR);
        let cancel_button = loadout_cancel_button(&db);
        ctx.branch()
            .remove_item(
                "loadout_clear_moved",
                player_moved_item.as_loot(1000).loot(&db),
            )
            .remove_item(
                "loadout_clear_resume_button",
                resume_button_item.as_loot(1000).loot(&db),
            )
            .start_quest("init_cancel_button", cancel_button)
            .switch_end("loadout_edit", |s| {
                s
                    // .message("Editing loadout. Fly to the target to continue your adventure")
                    // .transition(
                    //     1.0,
                    //     RequirementRandomStarSystem {
                    //         min_value: 1,
                    //         max_value: 1,
                    //         ..Default::default()
                    //     },
                    //     goto("init"),
                    // )
                    .transition(1.0, player_moved_item.req_at_least(1), goto("init"))
                    .transition(
                        1.0,
                        resume_button_item.req_at_least(1) & !cancel_button.req_active(),
                        goto("init"),
                    )
            })
            .entrypoint()
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

    fn win_combat(ctx: Ctx) -> NodeId {
        // TODO: rewards
        ctx.cached("win_combat", |ctx| {
            ctx.branch()
                .goto_quest("win_combat", new_encounter)
                .entrypoint()
        })
    }

    fn lose_combat(ctx: Ctx) -> NodeId {
        ctx.cached("lose_combat", |ctx| {
            ctx.branch()
                .dialog(
                    "lose_combat",
                    "This branch has no conclusion. Try again",
                    |d| d.next(MSG_CONTINUE),
                )
                .goto_quest("end_run", end_game_start_new)
                .entrypoint()
        })
    }

    fn path_choice(ctx: Ctx) -> NodeId {
        ctx.cached("init", |ctx| {
            let db = ctx.db.clone();
            ctx.branch()
                .dialog_end("init", "Select a path", |mut d| {
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

    db.cached::<Quest>(QUEST_ENCOUNTER_CHOICE, || {
        quest(db, QUEST_ENCOUNTER_CHOICE, "Encounter Choice", |ctx| {
            path_choice(ctx)
        })
        .with(|q| q.with_name(""))
        .id
    })
}

fn loadout_cancel_button(db: &Database) -> QuestId {
    db.cached(QUEST_ENCOUNTER_CANCEL_BUTTON, || {
        let player_moved_item = item(db, ITEM_PLAYER_DID_MOVE);
        quest(db, QUEST_ENCOUNTER_CANCEL_BUTTON, "Loadout Done", |ctx| {
            ctx.branch()
                .receive_item(
                    "init",
                    item(db, ITEM_RESUME_BUTTON_INDICATOR).as_loot(1).loot(db),
                )
                .wait_for(
                    "waiter",
                    "Cancel this quest or move to any star to continue your journey",
                    player_moved_item.req_at_least(1),
                )
                .cancel_quest()
                .entrypoint()
        })
        .id
    })
}
