use eh_mod_cli::dev::schema::schema::Quest;
use quests::quests::NodeId;

use crate::roguelite::core::new_game::end_game_start_new;
use crate::roguelite::core::{quest, BBExt, Ctx};
use crate::roguelite::MSG_GONE_WRONG;

pub fn something_gone_wrong(ctx: Ctx, error: &str) -> NodeId {
    let id = format!("something_gone_wrong_{}", error);
    ctx.cached(id.clone(), |ctx| {
        ctx.branch()
            .goto_quest("init", |db| {
                db.cached::<Quest>(&id.clone(), || {
                    let msg = MSG_GONE_WRONG.to_string() + error;
                    quest(db, id.clone(), msg.clone(), |ctx| {
                        ctx.branch()
                            .dialog(id, msg, |d| d.next("Start new run"))
                            .goto_quest("end_game", end_game_start_new)
                            .entrypoint()
                    })
                    .id
                })
            })
            .entrypoint()
    })
}
