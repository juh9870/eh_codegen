use crate::schema::{ComponentId, LootContent, LootContentComponent, MinMax};

impl ComponentId {
    pub fn as_loot(self, amount: impl MinMax<i32>) -> LootContent {
        let (min_amount, max_amount) = amount.into_min_max();
        LootContentComponent {
            item_id: self,
            min_amount,
            max_amount,
        }
        .wrap()
    }
}
