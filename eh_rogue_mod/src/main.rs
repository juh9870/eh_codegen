use crate::test_mod::build_mod;

pub mod test_mod;

fn main() {
    eh_mod_cli::run_main(build_mod)
}
