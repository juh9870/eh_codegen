use std::borrow::Cow;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use parking_lot::RwLock;

use eh_mod_dev::database::Database;
use eh_mod_dev::mapping::{IdMapping, KindProvider};
use eh_mod_dev::schema::schema::{
    Node, NodeCancelQuest, NodeCompleteQuest, NodeFailQuest, Quest, QuestId,
};

use crate::quests::branch::{BranchBuilder, BranchBuilderData};

pub mod branch;

pub const COMPLETE_ID_NAME: &str = "complete";
pub const FAIL_ID_NAME: &str = "fail";
pub const CANCEL_ID_NAME: &str = "cancel";
pub const START_ID: NodeId = NodeId(1);
pub const COMPLETE_ID: NodeId = NodeId(2);
pub const FAIL_ID: NodeId = NodeId(3);
pub const CANCEL_ID: NodeId = NodeId(4);

#[derive(Debug)]
pub struct QuestContext {
    data: QuestContextData,
}

impl QuestContext {
    pub fn new(
        db: &Database,
        id: impl Into<String>,
        starting_node_id: impl Into<String>,
    ) -> QuestContext {
        let mappings = db.get_mappings::<NodeId>();
        let string_id = id.into();
        let id = db.new_id(string_id.as_str());
        let mut data = QuestContextData {
            id,
            db: db.clone(),
            string_id,
            mappings,
            nodes: vec![],
            has_cancel: false,
            has_complete: false,
            has_fail: false,
            has_start: false,
        };
        data.init_defaults();
        data.set_start_id(starting_node_id);
        Self { data }
    }

    pub fn into_quest(self) -> Quest {
        Quest {
            id: self.id,
            name: "".to_string(),
            quest_type: Default::default(),
            start_condition: Default::default(),
            weight: 1.0,
            origin: Default::default(),
            requirement: Default::default(),
            level: 0,
            use_random_seed: false,
            nodes: self.data.nodes,
        }
    }
}

impl Deref for QuestContext {
    type Target = QuestContextData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for QuestContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[derive(Debug)]
pub struct QuestContextRef<'a> {
    data: &'a mut QuestContextData,
}

impl<'a> Deref for QuestContextRef<'a> {
    type Target = QuestContextData;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

#[derive(Debug)]
pub struct Contextual<'a, T> {
    context: &'a mut QuestContextData,
    data: T,
}

impl<'a, T> Contextual<'a, T> {
    pub fn new(context: &'a mut QuestContextData, data: T) -> Self {
        Self { context, data }
    }
}

impl<'a, T> Contextual<'a, T> {
    pub fn ctx(&mut self) -> &mut QuestContextData {
        self.context
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn into_inner(contextual: Self) -> T {
        contextual.data
    }

    pub fn map<U>(item: Self, f: impl FnOnce(T) -> U) -> Contextual<'a, U> {
        Contextual {
            context: item.context,
            data: f(item.data),
        }
    }
}

impl<T> Deref for Contextual<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for Contextual<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[derive(Debug)]
pub struct QuestContextData {
    pub id: QuestId,
    pub db: Database,
    pub string_id: String,
    mappings: Arc<RwLock<IdMapping>>,
    pub nodes: Vec<Node>,
    has_cancel: bool,
    has_complete: bool,
    has_fail: bool,
    has_start: bool,
}

impl QuestContextData {
    pub fn id(&self, id: impl IntoNodeId) -> NodeId {
        let m = self.mappings.read();
        NodeId(id.into_id(&self.string_id, &m))
    }
    pub fn new_id(&mut self, id: impl IntoNodeId) -> NodeId {
        let mut m = self.mappings.write();
        NodeId(id.into_new_id(self.string_id.clone(), &mut m))
    }

    pub fn set_id(&mut self, string_id: impl Into<String>, numeric_id: i32) {
        self.mappings
            .write()
            .set_id(self.string_id.clone(), string_id, numeric_id);
    }

    pub fn branch(&mut self) -> BranchBuilder {
        Contextual::new(self, BranchBuilderData::default())
    }

    pub fn cached(
        &mut self,
        id: impl Into<String>,
        func: impl FnOnce(&mut Self) -> NodeId,
    ) -> NodeId {
        let id = id.into();
        if self.mappings.read().is_used(self.string_id.clone(), &id) {
            return self.id(id);
        }

        func(self)
    }

    fn set_start_id(&mut self, string_id: impl Into<String>) {
        if self.has_start {
            panic!(
                "Quest already has start node: {}",
                self.mappings
                    .read()
                    .get_inverse_id(&self.string_id, START_ID.0)
                    .unwrap()
            )
        }

        self.has_start = true;
        let string_id = string_id.into();
        self.set_id(&string_id, START_ID.0);
        self.mappings
            .write()
            .forget_used_id(self.string_id.clone(), &string_id);
    }

    fn add_complete(&mut self) -> NodeId {
        if self.has_complete {
            return COMPLETE_ID;
        }
        self.nodes
            .push(NodeCompleteQuest { id: COMPLETE_ID.0 }.into());
        self.has_complete = true;
        COMPLETE_ID
    }

    fn add_fail(&mut self) -> NodeId {
        if self.has_fail {
            return FAIL_ID;
        }
        self.nodes.push(NodeFailQuest { id: FAIL_ID.0 }.into());
        self.has_fail = true;
        FAIL_ID
    }

    fn add_cancel(&mut self) -> NodeId {
        if self.has_cancel {
            return CANCEL_ID;
        }
        self.nodes.push(NodeCancelQuest { id: CANCEL_ID.0 }.into());
        self.has_cancel = true;
        CANCEL_ID
    }

    fn add_node(&mut self, node: impl Into<Node>) {
        self.nodes.push(node.into());
    }

    fn init_defaults(&mut self) {
        self.mappings
            .write()
            .add_id_range_for(self.string_id.clone(), 10..1000000);

        self.set_id(COMPLETE_ID_NAME, COMPLETE_ID.0);
        self.set_id(FAIL_ID_NAME, FAIL_ID.0);
        self.set_id(CANCEL_ID_NAME, CANCEL_ID.0);
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct NodeId(i32);

impl KindProvider for NodeId {
    fn kind() -> Cow<'static, str> {
        Cow::Borrowed("QuestBuilderNode")
    }
}

pub trait IntoNodeId {
    fn into_id<'a>(self, quest_id: impl Into<Cow<'a, str>>, ids: &'a IdMapping) -> i32;
    fn into_new_id(self, quest_id: impl Into<Cow<'static, str>>, ids: &mut IdMapping) -> i32;
}

impl IntoNodeId for NodeId {
    fn into_id<'a>(self, _: impl Into<Cow<'a, str>>, _: &'a IdMapping) -> i32 {
        self.0
    }
    fn into_new_id(self, _: impl Into<Cow<'static, str>>, _: &mut IdMapping) -> i32 {
        self.0
    }
}

impl IntoNodeId for &str {
    fn into_id<'a>(self, quest_id: impl Into<Cow<'a, str>>, ids: &'a IdMapping) -> i32 {
        ids.existing_id(quest_id, self)
    }
    fn into_new_id(self, quest_id: impl Into<Cow<'static, str>>, ids: &mut IdMapping) -> i32 {
        ids.new_id(quest_id, self)
    }
}

impl IntoNodeId for String {
    fn into_id<'a>(self, quest_id: impl Into<Cow<'a, str>>, ids: &'a IdMapping) -> i32 {
        ids.existing_id(quest_id, &self)
    }
    fn into_new_id(self, quest_id: impl Into<Cow<'static, str>>, ids: &mut IdMapping) -> i32 {
        ids.new_id(quest_id, self)
    }
}
