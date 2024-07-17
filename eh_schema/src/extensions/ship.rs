use crate::schema::{LootContent, LootContentEmptyShip, ShipId};

impl ShipId {
    pub fn as_loot(self) -> LootContent {
        LootContentEmptyShip { item_id: self }.wrap()
    }
}
