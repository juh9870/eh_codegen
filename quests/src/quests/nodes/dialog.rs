use eh_mod_dev::schema::schema::{
    CharacterId, FleetId, LootId, NodeAction, NodeShowDialog, RequiredViewMode, Requirement,
};

use crate::quests::nodes::{BranchBuilder, BranchDone, TransitionalNode};
use crate::quests::{IntoNodeId, NodeId, QuestContextData};

pub struct SmartDialog<'a, const HAS_NEXT: bool> {
    ctx: &'a mut QuestContextData,
    node: NodeShowDialog,
    next_transition: Option<usize>,
}

impl<const HAS_NEXT: bool> SmartDialog<'_, HAS_NEXT> {
    /// Adds an action that branches out
    pub fn action<T>(
        mut self,
        action: impl IntoDialogAction,
        branch: impl DialogActionTarget<T>,
    ) -> Self {
        let mut action = action.into_action();
        action.target_node = branch.call(self.ctx).0;
        self.node.actions.push(action);
        self
    }

    /// Sets the enemy fleet for the dialog
    pub fn enemy(mut self, enemy: impl Into<Option<FleetId>>) -> Self {
        self.node.set_enemy(enemy);
        self
    }

    /// Sets the character for the dialog
    pub fn character(mut self, character: impl Into<Option<CharacterId>>) -> Self {
        self.node.set_character(character);
        self
    }

    /// Sets the loot for the dialog
    pub fn loot(mut self, loot: impl Into<Option<LootId>>) -> Self {
        self.node.set_loot(loot);
        self
    }

    /// Sets the required view mode for the dialog
    pub fn required_view(mut self, mode: RequiredViewMode) -> Self {
        self.node.set_required_view(mode);
        self
    }
}

impl<'a> SmartDialog<'a, false> {
    /// Adds an action that continues onwards
    pub fn next(mut self, action: impl IntoDialogAction) -> SmartDialog<'a, true> {
        self.next_transition = Some(self.node.actions.len());
        self.node.actions.push(action.into_action());
        SmartDialog {
            ctx: self.ctx,
            node: self.node,
            next_transition: self.next_transition,
        }
    }

    /// Converts the dialog into a node. This is only possible before the next transition is set
    pub fn into_node(self) -> NodeShowDialog {
        self.node
    }

    pub fn new(
        ctx: &'a mut QuestContextData,
        id: impl IntoNodeId,
        message: impl Into<String>,
    ) -> Self {
        let node = NodeShowDialog {
            id: ctx.id(id).0,
            required_view: Default::default(),
            message: message.into(),
            enemy: None,
            loot: None,
            character: None,
            actions: vec![],
        };

        SmartDialog {
            ctx,
            node,
            next_transition: None,
        }
    }
}

impl<'a> SmartDialog<'a, true> {
    pub fn bake(self) -> BakedDialog {
        BakedDialog {
            node: self.node,
            next_transition: self
                .next_transition
                .expect("Next transition should always be present in a bake-able dialog"),
        }
    }
}

pub trait IntoDialogAction {
    fn into_action(self) -> NodeAction;
}

impl IntoDialogAction for String {
    fn into_action(self) -> NodeAction {
        NodeAction {
            target_node: 0,
            requirement: Default::default(),
            button_text: self,
        }
    }
}

impl<T: Into<Requirement>> IntoDialogAction for (String, T) {
    fn into_action(self) -> NodeAction {
        NodeAction {
            target_node: 0,
            requirement: self.1.into(),
            button_text: self.0,
        }
    }
}

pub struct BakedDialog {
    node: NodeShowDialog,
    next_transition: usize,
}

impl TransitionalNode for BakedDialog {
    fn consume(mut self: Box<Self>, ctx: &mut QuestContextData, next: NodeId) {
        self.node.actions[self.next_transition].target_node = next.0;
        ctx.add_node(self.node);
    }

    fn entrypoint_id(&self) -> NodeId {
        NodeId(self.node.id)
    }
}

impl From<SmartDialog<'_, true>> for BakedDialog {
    fn from(value: SmartDialog<'_, true>) -> Self {
        value.bake()
    }
}

pub trait DialogActionTarget<T> {
    fn call(self, ctx: &mut QuestContextData) -> NodeId;
}

impl DialogActionTarget<NodeId> for NodeId {
    fn call(self, _ctx: &mut QuestContextData) -> NodeId {
        self
    }
}

impl<F: Fn(&mut QuestContextData) -> NodeId> DialogActionTarget<()> for F {
    fn call(self, ctx: &mut QuestContextData) -> NodeId {
        self(ctx)
    }
}

impl<F: Fn(BranchBuilder) -> BranchDone> DialogActionTarget<BranchDone> for F {
    fn call(self, ctx: &mut QuestContextData) -> NodeId {
        self(BranchBuilder::new(ctx)).entrypoint()
    }
}
