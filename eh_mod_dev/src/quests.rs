use eh_schema::schema::QuestId;
use std::ops::Deref;

#[derive(Debug)]
pub struct QuestContext {
    data: QuestContextData,
}

impl Deref for QuestContext {
    type Target = QuestContextData;

    fn deref(&self) -> &Self::Target {
        &self.data
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
pub struct QuestContextData {
    pub id: QuestId,
}
