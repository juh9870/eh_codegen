use eh_mod_cli::run_main;

use crate::test_mod::build_mod;

mod test_mod;

fn main() {
    run_main(build_mod)
}
