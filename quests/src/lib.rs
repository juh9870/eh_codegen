use eh_mod_dev::database::Database;

use crate::quests::QuestContext;

pub mod quests;

pub fn xquest(
    db: &Database,
    id: impl Into<String>,
    starting_node_id: impl Into<String>,
) -> QuestContext {
    QuestContext::new(db, id, starting_node_id)
}

pub const MSG_CONTINUE: &str = "$Continue";
pub const MSG_CANCEL: &str = "$Cancel";
pub const MSG_ATTACK: &str = "$Attack";
