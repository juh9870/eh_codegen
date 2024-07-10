use eh_mod_cli::dev::schema::schema::Quest;
use std::collections::HashSet;

pub fn next_id(quest: &Quest) -> impl FnMut() -> i32 {
    let nodes: HashSet<i32> = quest.nodes.iter().map(|n| n.id()).copied().collect();
    let mut last_id = 0;

    move || {
        while last_id < 999999 {
            last_id += 1;
            if !nodes.contains(&last_id) {
                return last_id;
            }
        }
        panic!("Out of IDs")
    }
}
