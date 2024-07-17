use std::ops::RangeInclusive;

use eh_mod_cli::dev::database::Database;
use eh_mod_cli::dev::schema::schema::{FleetId, LootId, QuestItemId, Requirement};

pub type Events = Vec<Event>;

pub type WeightedVec<T> = Vec<Weighted<T>>;

#[derive(Debug, Clone)]
pub struct Weighted<T> {
    pub item: T,
    pub weight: f32,
    pub req: Requirement,
}

impl<T> From<T> for Weighted<T> {
    fn from(item: T) -> Self {
        Self {
            item,
            weight: 1.0,
            req: Default::default(),
        }
    }
}

impl<T> From<(T, f32)> for Weighted<T> {
    fn from((item, weight): (T, f32)) -> Self {
        Self {
            item,
            weight,
            req: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum EventKind {
    Combat(WeightedVec<FleetId>, Option<LootId>),
}

#[derive(Debug, Clone)]
pub struct Event {
    id: String,
    pub item: QuestItemId,
    pub kind: EventKind,
    pub weight: f32,
    pub chapters: Option<RangeInclusive<usize>>,
}

impl Event {
    pub fn new(db: &Database, id: impl Into<String>, kind: EventKind) -> Self {
        let id = id.into();
        let item = db.new_quest_item(id.as_str()).edit(|i| {
            i.set_price(0)
                .set_name("The Code")
                .set_description("I am a man of my word, standing on your CPU");
        });

        Self {
            id,
            item: item.id,
            kind,
            weight: 0.0,
            chapters: None,
        }
    }

    pub fn quest_id(&self) -> String {
        format!("event@{}", self.id)
    }

    pub fn name(&self) -> String {
        format!("{}.name", self.id)
    }

    pub fn description(&self) -> String {
        format!("{}.desc", self.id)
    }

    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self
    }

    pub fn with_chapters(mut self, chapter: impl Into<Option<RangeInclusive<usize>>>) -> Self {
        self.chapters = chapter.into();
        self
    }
}
