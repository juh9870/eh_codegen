use eh_mod_cli::run_main;

use crate::roguelite::build_mod;

mod roguelite;

fn main() {
    run_main(build_mod)
}
