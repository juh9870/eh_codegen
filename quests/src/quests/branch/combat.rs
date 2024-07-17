use duplicate::duplicate_item;

use eh_mod_dev::database::DatabaseIdLike;
use eh_mod_dev::mapping::OptionalDatabaseIdLike;
use eh_mod_dev::schema::schema::{
    Fleet, Loot, Node, NodeAttackFleet, NodeAttackOccupants, NodeAttackStarbase,
};

use crate::quests::branch::{BranchBuilder, BranchDone, TransitionalNode};
use crate::quests::{IntoNodeId, NodeId, QuestContextData};

#[derive(Debug)]
pub enum Combat<T: FnOnce(&mut QuestContextData) -> NodeId> {
    OnWin(T),
    OnLose(T),
}

impl<'a> BranchBuilder<'a> {
    pub fn attack_fleet<ID: DatabaseIdLike<Loot>, T: FnOnce(&mut QuestContextData) -> NodeId>(
        mut self,
        id: impl IntoNodeId,
        enemy: impl DatabaseIdLike<Fleet>,
        loot: impl OptionalDatabaseIdLike<Loot, ID>,
        branch_out: Combat<T>,
    ) -> BranchBuilder<'a> {
        let (win, fail, branch) = branch_out.win_lose_branch(self.ctx());
        let id = self.ctx().new_id(id).0;
        let enemy = self.ctx().db.id(enemy);
        let loot = loot.into_opt().map(|id| self.ctx().db.id(id));
        self.node(BakedCombat {
            node: NodeAttackFleet {
                id,
                default_transition: win,
                failure_transition: fail,
                enemy: Some(enemy),
                loot,
            },
            branch_on: branch,
        })
    }

    pub fn attack_fleet_end<ID: DatabaseIdLike<Loot>>(
        mut self,
        id: impl IntoNodeId,
        enemy: impl DatabaseIdLike<Fleet>,
        loot: impl OptionalDatabaseIdLike<Loot, ID>,
        win: impl FnOnce(&mut QuestContextData) -> NodeId,
        fail: impl FnOnce(&mut QuestContextData) -> NodeId,
    ) -> BranchDone {
        let node = NodeAttackFleet {
            id: self.ctx().new_id(id).0,
            default_transition: win(self.ctx()).0,
            failure_transition: fail(self.ctx()).0,
            enemy: Some(self.ctx().db.id(enemy)),
            loot: loot.into_opt().map(|id| self.ctx().db.id(id)),
        };
        self.push_final(node)
    }

    pub fn attack_occupants<T: FnOnce(&mut QuestContextData) -> NodeId>(
        mut self,
        id: impl IntoNodeId,
        branch_out: Combat<T>,
    ) -> BranchBuilder<'a> {
        let (win, fail, branch) = branch_out.win_lose_branch(self.ctx());
        let node = BakedCombat {
            node: NodeAttackOccupants {
                id: self.ctx().new_id(id).0,
                default_transition: win,
                failure_transition: fail,
            },
            branch_on: branch,
        };
        self.node(node)
    }

    pub fn attack_occupants_end<T: FnOnce(&mut QuestContextData) -> NodeId>(
        mut self,
        id: impl IntoNodeId,
        win: impl FnOnce(&mut QuestContextData) -> NodeId,
        fail: impl FnOnce(&mut QuestContextData) -> NodeId,
    ) -> BranchDone {
        let node = NodeAttackOccupants {
            id: self.ctx().new_id(id).0,
            default_transition: win(self.ctx()).0,
            failure_transition: fail(self.ctx()).0,
        };

        self.push_final(node)
    }

    pub fn attack_starbase<T: FnOnce(&mut QuestContextData) -> NodeId>(
        mut self,
        id: impl IntoNodeId,
        branch_out: Combat<T>,
    ) -> BranchBuilder<'a> {
        let (win, fail, branch) = branch_out.win_lose_branch(self.ctx());
        let node = BakedCombat {
            node: NodeAttackStarbase {
                id: self.ctx().new_id(id).0,
                default_transition: win,
                failure_transition: fail,
            },
            branch_on: branch,
        };
        self.node(node)
    }

    pub fn attack_starbase_end<T: FnOnce(&mut QuestContextData) -> NodeId>(
        mut self,
        id: impl IntoNodeId,
        win: impl FnOnce(&mut QuestContextData) -> NodeId,
        fail: impl FnOnce(&mut QuestContextData) -> NodeId,
    ) -> BranchDone {
        let node = NodeAttackStarbase {
            id: self.ctx().new_id(id).0,
            default_transition: win(self.ctx()).0,
            failure_transition: fail(self.ctx()).0,
        };

        self.push_final(node)
    }
}

impl<T: FnOnce(&mut QuestContextData) -> NodeId> Combat<T> {
    fn win_lose_branch(self, ctx: &mut QuestContextData) -> (i32, i32, BranchOutOn) {
        match self {
            Combat::OnWin(branch) => (branch(ctx).0, 0, BranchOutOn::Win),
            Combat::OnLose(branch) => (0, branch(ctx).0, BranchOutOn::Loss),
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum BranchOutOn {
    Win,
    Loss,
}

#[derive(Debug)]
struct BakedCombat<T: CombatNode + Into<Node>> {
    node: T,
    branch_on: BranchOutOn,
}

impl<T: CombatNode + Into<Node>> TransitionalNode for BakedCombat<T> {
    fn consume(mut self: Box<Self>, ctx: &mut QuestContextData, next: NodeId) {
        match self.branch_on {
            BranchOutOn::Win => self.node.set_failure_transition(next),
            BranchOutOn::Loss => self.node.set_win_transition(next),
        }
        ctx.add_node(self.node.into());
    }

    fn entrypoint_id(&self) -> NodeId {
        self.node.id()
    }
}

pub trait CombatNode {
    fn set_win_transition(&mut self, target: NodeId);
    fn set_failure_transition(&mut self, target: NodeId);
    fn id(&self) -> NodeId;
}

#[duplicate_item(
    ty;
    [NodeAttackFleet];
    [NodeAttackOccupants];
    [NodeAttackStarbase];
)]
impl CombatNode for ty {
    fn set_win_transition(&mut self, target: NodeId) {
        self.default_transition = target.0;
    }

    fn set_failure_transition(&mut self, target: NodeId) {
        self.failure_transition = target.0;
    }

    fn id(&self) -> NodeId {
        NodeId(self.id)
    }
}
