use eh_mod_dev::schema::schema::{
    NodeCondition, NodeRandom, NodeSwitch, NodeTransition, Requirement,
};

use crate::quests::branch::TransitionalNode;
use crate::quests::{Contextual, IntoNodeId, NodeId, QuestContextData};

pub type SmartSwitch<'a, const HAS_NEXT: bool, const HAS_DEFAULT: bool> =
    Contextual<'a, SmartSwitchData<HAS_NEXT, HAS_DEFAULT>>;

#[derive(Debug)]
pub struct SmartSwitchData<const HAS_NEXT: bool, const HAS_DEFAULT: bool> {
    id: i32,
    message: String,
    default_transition: Option<i32>,
    transitions: Vec<NodeTransition>,
    next_transition: Option<NextTransition>,
}

#[derive(Debug, Copy, Clone)]
enum NextTransition {
    Next(usize),
    Default,
}

pub fn new_smart_switch(
    ctx: &mut QuestContextData,
    id: impl IntoNodeId,
) -> SmartSwitch<false, false> {
    let id = ctx.new_id(id);
    Contextual::new(
        ctx,
        SmartSwitchData {
            id: id.0,
            message: format!("QUEST {} NODE #{}", ctx.string_id, id.0),
            default_transition: None,
            transitions: vec![],
            next_transition: None,
        },
    )
}

impl<'a, const HAS_NEXT: bool, const HAS_DEFAULT: bool> SmartSwitch<'a, HAS_NEXT, HAS_DEFAULT> {
    pub fn transition(
        mut self,
        weight: f32,
        requirement: impl Into<Requirement>,
        branch: impl FnOnce(&mut QuestContextData) -> NodeId,
    ) -> Self {
        let target = branch(self.ctx()).0;
        self.transitions.push(NodeTransition {
            target_node: target,
            requirement: requirement.into(),
            weight,
        });

        self
    }

    /// Sets the message for the switch node
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }
}

impl<'a, const HAS_DEFAULT: bool> SmartSwitch<'a, false, HAS_DEFAULT> {
    pub fn next(
        mut self,
        weight: f32,
        requirement: impl Into<Requirement>,
    ) -> SmartSwitch<'a, true, HAS_DEFAULT> {
        self.next_transition = Some(NextTransition::Next(self.transitions.len()));
        self.transitions.push(NodeTransition {
            target_node: 0,
            requirement: requirement.into(),
            weight,
        });

        Contextual::map(self, |s| s.transmogrify())
    }
}

impl<'a> SmartSwitch<'a, false, false> {
    pub fn next_default(mut self) -> SmartSwitch<'a, true, true> {
        self.next_transition = Some(NextTransition::Default);

        Contextual::map(self, |s| s.transmogrify())
    }
}

impl<'a, const HAS_DEFAULT: bool> SmartSwitch<'a, true, HAS_DEFAULT> {
    pub fn bake_switch(self) -> BakedSwitch<HAS_DEFAULT> {
        BakedSwitch(Self::into_inner(self))
    }

    pub fn bake_random(self) -> BakedRandom<HAS_DEFAULT> {
        BakedRandom(Self::into_inner(self))
    }
}

impl<'a> SmartSwitch<'a, true, false> {
    pub fn bake_condition(self) -> BakedCondition {
        BakedCondition(Self::into_inner(self))
    }
}

impl SmartSwitchData<false, false> {
    pub fn into_condition(self) -> NodeCondition {
        NodeCondition {
            id: self.id,
            message: self.message,
            transitions: self.transitions,
        }
    }
}

impl SmartSwitchData<false, true> {}

impl<'a, const HAS_NEXT: bool> SmartSwitch<'a, HAS_NEXT, false> {
    pub fn default(
        mut self,
        branch: impl FnOnce(&mut QuestContextData) -> NodeId,
    ) -> SmartSwitch<'a, HAS_NEXT, true> {
        self.default_transition = Some(branch(self.ctx()).0);

        Contextual::map(self, |s| SmartSwitchData {
            id: s.id,
            message: s.message,
            default_transition: s.default_transition,
            transitions: s.transitions,
            next_transition: s.next_transition,
        })
    }
}

impl<const HAS_DEFAULT: bool> SmartSwitchData<false, HAS_DEFAULT> {
    pub fn into_random(self) -> NodeRandom {
        NodeRandom {
            id: self.id,
            message: self.message,
            default_transition: self.default_transition.unwrap_or(0),
            transitions: self.transitions,
        }
    }

    pub fn into_switch(self) -> NodeSwitch {
        NodeSwitch {
            id: self.id,
            message: self.message,
            default_transition: self.default_transition.unwrap_or(0),
            transitions: self.transitions,
        }
    }
}

impl<const HAS_DEFAULT: bool> SmartSwitchData<true, HAS_DEFAULT> {
    fn set_next(mut self, next: NodeId) -> SmartSwitchData<false, HAS_DEFAULT> {
        match self.next_transition {
            Some(NextTransition::Next(idx)) => {
                self.transitions[idx].target_node = next.0;
            }
            Some(NextTransition::Default) => {
                self.default_transition = Some(next.0);
            }
            None => unreachable!("Next transition not set"),
        }

        self.transmogrify()
    }
}

impl<const HAS_NEXT: bool, const HAS_DEFAULT: bool> SmartSwitchData<HAS_NEXT, HAS_DEFAULT> {
    fn transmogrify<const NEW_HAS_NEXT: bool, const NEW_HAS_DEFAULT: bool>(
        self,
    ) -> SmartSwitchData<NEW_HAS_NEXT, NEW_HAS_DEFAULT> {
        SmartSwitchData {
            id: self.id,
            message: self.message,
            default_transition: self.default_transition,
            transitions: self.transitions,
            next_transition: self.next_transition,
        }
    }
}

#[derive(Debug)]
pub struct BakedSwitch<const HAS_DEFAULT: bool>(SmartSwitchData<true, HAS_DEFAULT>);

#[derive(Debug)]
pub struct BakedRandom<const HAS_DEFAULT: bool>(SmartSwitchData<true, HAS_DEFAULT>);

#[derive(Debug)]
pub struct BakedCondition(SmartSwitchData<true, false>);

impl<const HAS_DEFAULT: bool> TransitionalNode for BakedSwitch<HAS_DEFAULT> {
    fn consume(self: Box<Self>, ctx: &mut QuestContextData, next: NodeId) {
        ctx.add_node(self.0.set_next(next).into_switch());
    }

    fn entrypoint_id(&self) -> NodeId {
        NodeId(self.0.id)
    }
}

impl<const HAS_DEFAULT: bool> TransitionalNode for BakedRandom<HAS_DEFAULT> {
    fn consume(self: Box<Self>, ctx: &mut QuestContextData, next: NodeId) {
        ctx.add_node(self.0.set_next(next).into_random());
    }

    fn entrypoint_id(&self) -> NodeId {
        NodeId(self.0.id)
    }
}

impl TransitionalNode for BakedCondition {
    fn consume(self: Box<Self>, ctx: &mut QuestContextData, next: NodeId) {
        ctx.add_node(self.0.set_next(next).into_condition());
    }

    fn entrypoint_id(&self) -> NodeId {
        NodeId(self.0.id)
    }
}
