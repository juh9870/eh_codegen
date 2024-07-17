use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

use itertools::Itertools;

use eh_mod_dev::database::{Database, DbItem};
use eh_mod_dev::layout::Layout;
use eh_mod_dev::schema::schema::{Component, ComponentStats, MinMax};

use crate::modifier::StatsModifier;

#[derive(Debug)]
pub struct SizedComponent {
    id: String,
    ids: Vec<String>,
    stats: Vec<DbItem<ComponentStats>>,
    components: Vec<DbItem<Component>>,
    sizes: (usize, usize),
}

fn size_name(size: usize) -> Cow<'static, str> {
    match size {
        0 => "s".into(),
        1 => "m".into(),
        2 => "l".into(),
        3 => "xl".into(),
        _ => size.to_string().into(),
    }
}

impl SizedComponent {
    pub fn new(db: &Database, id: impl Into<String>, sizes: impl MinMax<usize>) -> Self {
        let sizes = sizes.into_min_max();
        let id = id.into();
        let ids = (sizes.0..sizes.1)
            .map(|size| format!("{}_{}", id, size_name(size)))
            .collect_vec();
        Self {
            stats: ids
                .iter()
                .map(|id| db.new_component_stats(db.new_id(id.as_str())))
                .collect_vec(),
            components: ids
                .iter()
                .map(|id| {
                    db.new_component(db.new_id(id.as_str()), db.id(id.as_str()))
                        .with(|c| {
                            c.with_name(format!("${}.name", id))
                                .with_description(format!("${}.desc", id))
                                .with_icon(id.clone())
                        })
                })
                .collect_vec(),
            ids,
            id,
            sizes,
        }
    }

    /// Assigns square layouts of increasing sizes to components
    pub fn layouts_square(&mut self) -> &mut Self {
        let min = self.sizes.0 + 1;
        for (size, comp) in self.components.iter_mut().enumerate() {
            comp.layout = Layout::new_square(min + size, '1').to_string();
        }
        self
    }

    /// Applies stat progression given by the modifier
    ///
    /// Modifier is applied incrementally, starting at the lowest sized
    /// component stats
    pub fn stats(&mut self, mult: StatsModifier) {
        for i in 1..self.stats.len() {
            let mut cur = self.stats[i - 1].deref().clone();
            mult.apply(&mut cur);

            let s = self.stats[i].deref_mut();
            cur.id = s.id;
            *s = cur;
        }
    }
}
