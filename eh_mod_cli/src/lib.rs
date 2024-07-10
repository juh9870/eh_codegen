use clap::Parser;
use std::path::PathBuf;
use tracing_panic::panic_hook;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;

pub use eh_mod_dev as dev;

#[derive(Debug, Parser)]
pub struct Args {
    pub vanilla_dir: PathBuf,
    pub output_dir: PathBuf,
    pub output_mod: Option<PathBuf>,
}

pub fn run_main(build: impl FnOnce(Args)) {
    let subscriber = tracing_subscriber::Registry::default()
        .with(tracing_subscriber::fmt::Layer::default().pretty())
        .with(EnvFilter::from_default_env());

    tracing::subscriber::set_global_default(subscriber).unwrap();

    rayon::ThreadPoolBuilder::new()
        .num_threads(num_cpus::get())
        .build_global()
        .unwrap();

    let args = Args::parse();

    color_backtrace::install();
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        panic_hook(panic_info);
        prev_hook(panic_info);
    }));

    build(args)
}
