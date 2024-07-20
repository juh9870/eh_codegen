use crate::helpers::MinMax;
use crate::schema::{CharacterId, Requirement, RequirementCharacterRelations};

impl CharacterId {
    pub fn relation_at_least(self, min: i32) -> Requirement {
        RequirementCharacterRelations {
            min_value: min,
            max_value: i32::MAX,
            character: Some(self),
        }
        .wrap()
    }

    pub fn relation_at_most(self, max: i32) -> Requirement {
        RequirementCharacterRelations {
            min_value: 0,
            max_value: max,
            character: Some(self),
        }
        .wrap()
    }

    pub fn relation_amount(self, amount: impl MinMax<i32>) -> Requirement {
        let (min_amount, max_amount) = amount.into_min_max();
        RequirementCharacterRelations {
            min_value: min_amount,
            max_value: max_amount,
            character: Some(self),
        }
        .wrap()
    }
}
