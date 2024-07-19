use eh_mod_dev::schema::schema::{Node, Requirement};

use crate::quests::{Contextual, IntoNodeId, NodeId, QuestContextData};
use crate::quests::branch::dialog::{BakedDialog, SmartDialog};
use crate::quests::branch::switch::{new_smart_switch, SmartSwitch};

pub mod combat;
pub mod dialog;
pub mod switch;

pub mod nodes;

pub type BranchBuilder<'a> = Contextual<'a, BranchBuilderData>;

#[derive(Default)]
pub struct BranchBuilderData {
    last_transitional: Option<Box<dyn TransitionalNode>>,
    finalized: bool,
    entrypoint: Option<NodeId>,
}

impl<'a> BranchBuilder<'a> {
    /// Pushes a transitional node to the branch
    pub fn node<T: TransitionalNode + 'static>(mut self, node: T) -> Self {
        self.push_transitional(node);
        self
    }

    /// Pushes a dialog node to the branch
    ///
    /// Dialogs can be either transitional or final nodes:
    /// - Dialogs which call to `.next(...)` are counted as transitional nodes, and so the branch continues
    /// - Dialogs which do not call to `.next(...)` are counted as final nodes, and so the branch ends
    pub fn dialog_raw<T, F: DialogFn<T>>(
        self,
        id: impl IntoNodeId,
        message: impl Into<String>,
        dialog: F,
    ) -> F::Result<'a> {
        dialog.dialog(self, id, message)
    }

    /// Dialog that continues the branch
    pub fn dialog(
        mut self,
        id: impl IntoNodeId,
        message: impl Into<String>,
        dialog: impl FnOnce(SmartDialog<false>) -> SmartDialog<true>,
    ) -> Self {
        let d = SmartDialog::new(self.ctx(), id, message);
        let baked = dialog(d).bake();

        self.node(baked)
    }

    /// Dialog that ends the branch
    pub fn dialog_end(
        mut self,
        id: impl IntoNodeId,
        message: impl Into<String>,
        dialog: impl FnOnce(SmartDialog<false>) -> SmartDialog<false>,
    ) -> BranchDone {
        let d = SmartDialog::new(self.ctx(), id, message);
        let out = dialog(d).into_node();

        self.push_final(out)
    }

    /// Switch node that continues the branch
    pub fn switch<const HAS_DEFAULT: bool>(
        mut self,
        id: impl IntoNodeId,
        func: impl FnOnce(SmartSwitch<false, false>) -> SmartSwitch<true, HAS_DEFAULT>,
    ) -> BranchBuilder<'a> {
        let s = new_smart_switch(self.ctx(), id);
        let out = func(s).bake_switch();

        self.node(out)
    }

    /// Random node that continues the branch
    pub fn random<const HAS_DEFAULT: bool>(
        mut self,
        id: impl IntoNodeId,
        func: impl FnOnce(SmartSwitch<false, false>) -> SmartSwitch<true, HAS_DEFAULT>,
    ) -> BranchBuilder<'a> {
        let s = new_smart_switch(self.ctx(), id);
        let out = func(s).bake_random();

        self.node(out)
    }

    /// Condition node that continues the branch
    pub fn condition(
        mut self,
        id: impl IntoNodeId,
        func: impl FnOnce(SmartSwitch<false, false>) -> SmartSwitch<true, false>,
    ) -> BranchBuilder<'a> {
        let s = new_smart_switch(self.ctx(), id);
        let out = func(s).bake_condition();

        self.node(out)
    }

    /// Switch node that ends the branch
    pub fn switch_end<const HAS_DEFAULT: bool>(
        mut self,
        id: impl IntoNodeId,
        func: impl FnOnce(SmartSwitch<false, false>) -> SmartSwitch<false, HAS_DEFAULT>,
    ) -> BranchDone {
        let s = new_smart_switch(self.ctx(), id);
        let out = Contextual::into_inner(func(s)).into_switch();

        self.push_final(out)
    }

    /// Random node that ends the branch
    pub fn random_end<const HAS_DEFAULT: bool>(
        mut self,
        id: impl IntoNodeId,
        func: impl FnOnce(SmartSwitch<false, false>) -> SmartSwitch<false, HAS_DEFAULT>,
    ) -> BranchDone {
        let s = new_smart_switch(self.ctx(), id);
        let out = Contextual::into_inner(func(s)).into_random();

        self.push_final(out)
    }

    /// Condition node that ends the branch
    pub fn condition_end(
        mut self,
        id: impl IntoNodeId,
        func: impl FnOnce(SmartSwitch<false, false>) -> SmartSwitch<false, false>,
    ) -> BranchDone {
        let s = new_smart_switch(self.ctx(), id);
        let out = Contextual::into_inner(func(s)).into_condition();

        self.push_final(out)
    }

    pub fn wait_for(
        self,
        id: impl IntoNodeId,
        quest_log_message: impl Into<String>,
        requirement: impl Into<Requirement>,
    ) -> BranchBuilder<'a> {
        self.condition(id, |c| c.message(quest_log_message).next(1.0, requirement))
    }

    pub fn into_transitional(self) -> impl TransitionalNode {
        Self::into_inner(self)
    }

    /// Pushes CompleteQuest node
    pub fn complete_quest(mut self) -> BranchDone {
        let id = self.ctx().add_complete();
        self.set_next(id);
        self.done()
    }

    /// Pushes FailQuest node
    pub fn fail_quest(mut self) -> BranchDone {
        let id = self.ctx().add_fail();
        self.set_next(id);
        self.done()
    }

    /// Pushes CancelQuest node
    pub fn cancel_quest(mut self) -> BranchDone {
        let id = self.ctx().add_cancel();
        self.set_next(id);
        self.done()
    }

    /// Pushes closes the branch with transition to the given node ID
    pub fn goto(mut self, node: impl FnOnce(&mut QuestContextData) -> NodeId) -> BranchDone {
        let next = node(self.ctx());
        self.set_next(next);
        self.done()
    }
}

impl TransitionalNode for BranchBuilderData {
    fn consume(mut self: Box<Self>, ctx: &mut QuestContextData, next: NodeId) {
        self.set_next_ctx(ctx, next);
        self.done_inner();
    }

    fn entrypoint_id(&self) -> NodeId {
        self.entrypoint
            .expect("Should have an entrypoint for a branch when used as a Transitional node")
    }
}

#[derive(Debug)]
pub struct BranchDone(NodeId);

impl BranchDone {
    pub fn entrypoint(self) -> NodeId {
        self.0
    }
}

impl BranchBuilderData {
    fn set_next_ctx(&mut self, ctx: &mut QuestContextData, next: NodeId) {
        if self.entrypoint.is_none() {
            self.entrypoint = Some(next);
        } else if let Some(last) = std::mem::take(&mut self.last_transitional) {
            last.consume(ctx, next)
        } else {
            unreachable!("BranchBuilder can never be left loose")
        }
    }

    fn done_inner(self) -> BranchDone {
        if !self.finalized {
            panic!("Quest builder dropped an unfinished branch")
        }
        BranchDone(
            self.entrypoint
                .expect("Should have entrypoint for finalized branch"),
        )
    }
}

impl<'a> BranchBuilder<'a> {
    fn set_next(&mut self, next: NodeId) {
        self.data.set_next_ctx(self.context, next);
    }

    fn push_transitional(&mut self, node: impl TransitionalNode + 'static) {
        let id = node.entrypoint_id();
        self.set_next(id);
        self.last_transitional = Some(Box::new(node))
    }

    fn push_final(mut self, node: impl Into<Node>) -> BranchDone {
        let node = node.into();
        self.set_next(NodeId(*node.id()));
        self.ctx().add_node(node);
        self.done()
    }

    fn done(mut self) -> BranchDone {
        self.finalized = true;
        Contextual::into_inner(self).done_inner()
    }
}

impl Drop for BranchBuilderData {
    fn drop(&mut self) {
        if !self.finalized {
            panic!("Quest builder dropped an unfinished branch")
        }

        if self.last_transitional.is_some() {
            unreachable!("Branch builder was finalized with unfinished transitional node")
        }
    }
}

pub trait TransitionalNode {
    fn consume(self: Box<Self>, ctx: &mut QuestContextData, next: NodeId);
    fn entrypoint_id(&self) -> NodeId;
}

duplicate::duplicate! {
    [
        ty;
        [NodeRetreat];
        [NodeDestroyOccupants];
        [NodeSuppressOccupants];
        [NodeReceiveItem];
        [NodeRemoveItem];
        [NodeTrade];
        [NodeStartQuest];
        [NodeChangeFactionRelations];
        [NodeSetFactionRelations];
        [NodeChangeCharacterRelations];
        [NodeSetCharacterRelations];
        [NodeOpenShipyard];
        [NodeOpenWorkshop];
        [NodeChangeFaction];
        [NodeCaptureStarBase];
        [NodeLiberateStarBase];
        [NodeSetFactionStarbasePower];
        [NodeChangeFactionStarbasePower];
    ]
    impl TransitionalNode for eh_mod_dev::schema::schema::ty {
        fn consume(mut self: Box<Self>, ctx: &mut QuestContextData, next: NodeId) {
            self.default_transition = next.0;
            ctx.add_node(*self);
        }

        fn entrypoint_id(&self) -> NodeId {
            NodeId(self.id)
        }
    }
}

pub trait DialogFn<T> {
    type Result<'a>;

    #[allow(clippy::needless_lifetimes)]
    fn dialog<'a>(
        self,
        b: BranchBuilder<'a>,
        id: impl IntoNodeId,
        message: impl Into<String>,
    ) -> Self::Result<'a>;
}

impl<T: Into<BakedDialog>, F: Fn(SmartDialog<'_, false>) -> T> DialogFn<SmartDialog<'_, true>>
    for F
{
    type Result<'a> = BranchBuilder<'a>;

    #[allow(clippy::needless_lifetimes)]
    fn dialog<'a>(
        self,
        mut b: BranchBuilder<'a>,
        id: impl IntoNodeId,
        message: impl Into<String>,
    ) -> Self::Result<'a> {
        let d = SmartDialog::new(b.ctx(), id, message);
        let baked = self(d).into();

        b.node(baked)
    }
}

impl<F: Fn(SmartDialog<'_, false>) -> SmartDialog<'_, false>> DialogFn<SmartDialog<'_, false>>
    for F
{
    type Result<'a> = ();

    #[allow(clippy::needless_lifetimes)]
    fn dialog<'a>(
        self,
        mut b: BranchBuilder<'a>,
        id: impl IntoNodeId,
        message: impl Into<String>,
    ) -> Self::Result<'a> {
        let d = SmartDialog::new(b.ctx(), id, message);
        let out = self(d).into_node();

        b.push_final(out);
    }
}
