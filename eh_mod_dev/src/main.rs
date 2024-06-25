use crate::test_mod::build_mod;
use clap::Parser;
use std::path::PathBuf;
use tracing_panic::panic_hook;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;

pub mod database;
pub mod layout;

pub mod test_mod;

#[derive(Debug, Parser)]
pub struct Args {
    mod_dir: PathBuf,
}

fn main() {
    let subscriber = tracing_subscriber::Registry::default()
        .with(tracing_subscriber::fmt::Layer::default())
        .with(EnvFilter::from_default_env());

    tracing::subscriber::set_global_default(subscriber).unwrap();

    let args = Args::parse();

    color_backtrace::install();
    let _prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        panic_hook(panic_info);
        // prev_hook(panic_info);
    }));

    build_mod(args.mod_dir)
}
