use crate::helpers::MinMax;
use crate::schema::{
    LootContent, LootContentQuestItem, QuestItemId, Requirement, RequirementHaveQuestItem,
};

impl QuestItemId {
    pub fn req_at_least(self, min: i32) -> Requirement {
        RequirementHaveQuestItem {
            item_id: Some(self),
            min_value: min,
        }
        .wrap()
    }

    pub fn req_at_most(self, max: i32) -> Requirement {
        !(RequirementHaveQuestItem {
            item_id: Some(self),
            min_value: max + 1,
        }
        .wrap())
    }

    pub fn req_amount(self, amount: impl MinMax<i32>) -> Requirement {
        let (min_amount, max_amount) = amount.into_min_max();
        let mut req = self.req_at_most(max_amount);

        if min_amount > 0 {
            req &= self.req_at_least(min_amount);
        }

        req
    }

    pub fn as_loot(self, amount: impl MinMax<i32>) -> LootContent {
        let (min_amount, max_amount) = amount.into_min_max();
        LootContentQuestItem {
            item_id: self,
            min_amount,
            max_amount,
        }
        .wrap()
    }
}
