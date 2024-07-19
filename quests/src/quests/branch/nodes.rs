use eh_mod_dev::mapping::DatabaseIdLike;
use eh_mod_dev::schema::schema::{Loot, Node, NodeReceiveItem, NodeRemoveItem, Quest};

use crate::quests::branch::BranchBuilder;
use crate::quests::IntoNodeId;

impl<'a> BranchBuilder<'a> {
    pub fn remove_item(
        mut self,
        id: impl IntoNodeId,
        loot: impl DatabaseIdLike<Loot>,
    ) -> BranchBuilder<'a> {
        let loot_id = self.ctx().db.id(loot);
        let id = self.ctx().new_id(id);
        self.node(NodeRemoveItem::new().with_id(id.0).with_loot(loot_id))
    }

    pub fn receive_item(
        mut self,
        id: impl IntoNodeId,
        loot: impl DatabaseIdLike<Loot>,
    ) -> BranchBuilder<'a> {
        let loot_id = self.ctx().db.id(loot);
        let id = self.ctx().new_id(id);
        self.node(NodeReceiveItem::new().with_id(id.0).with_loot(loot_id))
    }

    pub fn start_quest(
        mut self,
        id: impl IntoNodeId,
        quest: impl DatabaseIdLike<Quest>,
    ) -> BranchBuilder<'a> {
        let quest_id = self.ctx().db.id(quest);
        let id = self.ctx().new_id(id);
        self.node(Node::start_quest().with_id(id.0).with_quest(quest_id))
    }

    pub fn retreat(mut self, id: impl IntoNodeId) -> BranchBuilder<'a> {
        let id = self.ctx().new_id(id);
        self.node(Node::retreat().with_id(id.0))
    }
}
