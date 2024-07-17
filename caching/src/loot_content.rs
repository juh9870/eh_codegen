use ahash::AHashMap;

use eh_mod_dev::database::Database;
use eh_mod_dev::mapping::KindProvider;
use eh_mod_dev::schema::schema::{Loot, LootContent, LootId};

pub trait LootContentExt {
    fn loot(self, db: &Database) -> LootId;
}

impl LootContentExt for LootContent {
    fn loot(self, db: &Database) -> LootId {
        let cache = db.extra_or_init::<Cache>();
        let mut cache = cache.write();

        if let Some(id) = cache.get(&self) {
            return *id;
        }

        let id = db.use_id_mappings(|m| LootId::new(m.get_unstable_id(Loot::kind())));

        db.new_loot(id).set_loot(self.clone());

        cache.insert(self, id);

        id
    }
}

type Cache = AHashMap<LootContent, LootId>;
