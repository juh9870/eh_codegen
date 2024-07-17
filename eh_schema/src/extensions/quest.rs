use crate::schema::{QuestId, Requirement};

impl QuestId {
    pub fn req_active(self) -> Requirement {
        Requirement::quest_active().with_item_id(self).wrap()
    }

    pub fn req_completed(self) -> Requirement {
        Requirement::quest_completed().with_item_id(self).wrap()
    }
}
