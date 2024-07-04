use crate::test_mod::build_mod;
use clap::Parser;
use std::path::PathBuf;
use tracing_panic::panic_hook;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;

pub mod test_mod;

#[derive(Debug, Parser)]
pub struct Args {
    vanilla_dir: PathBuf,
    output_dir: PathBuf,
    output_mod: Option<PathBuf>,
}

fn main() {
    let subscriber = tracing_subscriber::Registry::default()
        .with(tracing_subscriber::fmt::Layer::default().pretty())
        // .with(Filter)
        .with(EnvFilter::from_default_env());

    tracing::subscriber::set_global_default(subscriber).unwrap();

    let args = Args::parse();

    color_backtrace::install();
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        panic_hook(panic_info);
        prev_hook(panic_info);
    }));

    build_mod(args)
}
