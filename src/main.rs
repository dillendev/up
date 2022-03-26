use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::{env, fs};

use anyhow::Result;
use clap::Parser;
use glob::Pattern;
use notify::{watcher, RecursiveMode, Watcher};

use crate::config::Config;
use crate::daemon::Daemon;
use crate::service::Service;

mod config;
mod daemon;
mod log;
mod process;
mod service;

#[derive(Debug, Parser)]
struct Args {
    #[clap(default_value = ".dev/up.toml")]
    filename: PathBuf,
}

fn load_config(filename: PathBuf) -> Result<Config> {
    let config = fs::read_to_string(filename)?;

    Ok(toml::from_str(&config)?)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let cwd = env::current_dir()?;
    let config = load_config(args.filename)?;

    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(1))?;
    watcher.watch(&cwd, RecursiveMode::Recursive)?;

    let (mut daemon, stop_handler) = Daemon::new(cwd, rx);

    log::info("daemon", "starting..");

    ctrlc::set_handler(move || {
        log::info("daemon", "stopping..");

        stop_handler.clone().stop();
    })?;

    for (name, cfg) in config.services {
        let mut patterns = vec![];

        for watch_pattern in cfg.watch.unwrap_or_default() {
            patterns.push(Pattern::new(&watch_pattern)?);
        }

        daemon.attach(Service::new(name, cfg.cmd, patterns))?;
    }

    log::info("daemon", "started");

    daemon.monitor();

    Ok(())
}
