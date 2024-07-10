use eh_mod_dev::schema::schema::Node;

use crate::quests::nodes::dialog::{BakedDialog, SmartDialog};
use crate::quests::{IntoNodeId, NodeId, QuestContextData};

pub mod dialog;

pub struct BranchBuilder<'a> {
    ctx: &'a mut QuestContextData,
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
    pub fn dialog<T, F: DialogFn<T>>(
        self,
        id: impl IntoNodeId,
        message: impl Into<String>,
        dialog: F,
    ) -> F::Result<'a> {
        dialog.dialog(self, id, message)
    }

    /// Pushes CompleteQuest node
    pub fn complete_quest(mut self) -> BranchDone {
        let id = self.ctx.add_complete();
        self.set_next(id);
        self.done()
    }

    /// Pushes FailQuest node
    pub fn fail_quest(mut self) -> BranchDone {
        let id = self.ctx.add_fail();
        self.set_next(id);
        self.done()
    }

    /// Pushes CancelQuest node
    pub fn cancel_quest(mut self) -> BranchDone {
        let id = self.ctx.add_cancel();
        self.set_next(id);
        self.done()
    }

    /// Pushes closes the branch with transition to the given node ID
    pub fn goto(mut self, node: NodeId) -> BranchDone {
        self.set_next(node);
        self.done()
    }
}

impl<'a> TransitionalNode for BranchBuilder<'a> {
    fn consume(self: Box<Self>, _ctx: &mut QuestContextData, next: NodeId) {
        self.goto(next);
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

impl<'a> BranchBuilder<'a> {
    pub fn new(ctx: &'a mut QuestContextData) -> Self {
        Self {
            ctx,
            last_transitional: None,
            finalized: false,
            entrypoint: None,
        }
    }

    fn set_next(&mut self, next: NodeId) {
        if self.entrypoint.is_none() {
            self.entrypoint = Some(next);
        } else if let Some(last) = std::mem::take(&mut self.last_transitional) {
            last.consume(self.ctx, next)
        } else {
            unreachable!("BranchBuilder can never be left loose")
        }
    }

    fn push_transitional(&mut self, node: impl TransitionalNode + 'static) {
        let id = node.entrypoint_id();
        self.set_next(id);
        self.last_transitional = Some(Box::new(node))
    }

    fn push_final(&mut self, node: impl Into<Node>) {
        let node = node.into();
        self.set_next(NodeId(*node.id()));
        self.ctx.add_node(node);
    }

    fn done(self) -> BranchDone {
        if !self.finalized {
            panic!("Quest builder dropped an unfinished branch")
        }
        BranchDone(
            self.entrypoint
                .expect("Should have entrypoint for finalized branch"),
        )
    }
}

impl<'a> Drop for BranchBuilder<'a> {
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
        b: BranchBuilder<'a>,
        id: impl IntoNodeId,
        message: impl Into<String>,
    ) -> Self::Result<'a> {
        let d = SmartDialog::new(b.ctx, id, message);
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
        let d = SmartDialog::new(b.ctx, id, message);
        let out = self(d).into_node();

        b.push_final(out);
    }
}
