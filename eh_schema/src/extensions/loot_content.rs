use crate::helpers::MinMax;
use crate::schema::{LootContent, LootContentRandomItems, LootItem};

impl LootContent {
    pub fn wrap_item(self, weight: f32) -> LootItem {
        LootItem { weight, loot: self }
    }

    pub fn repeat(self, amount: impl MinMax<i32>) -> LootContent {
        let (min_amount, max_amount) = amount.into_min_max();
        LootContentRandomItems {
            min_amount,
            max_amount,
            items: vec![self.wrap_item(1.0)],
        }
        .wrap()
    }
}
