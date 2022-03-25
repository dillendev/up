use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use crate::config::Config;
use crate::daemon::Daemon;
use crate::service::Service;

mod config;
mod daemon;
mod log;
mod service;

#[derive(Debug, Parser)]
struct Args {
    #[clap(default_value = ".dev/up.toml")]
    filename: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = fs::read_to_string(args.filename)?;
    let config: Config = toml::from_str(&config)?;
    let (mut daemon, stop_handler) = Daemon::new();

    log::info("daemon", "starting..");

    ctrlc::set_handler(move || {
        log::info("daemon", "stopping..");

        stop_handler.clone().stop();
    })?;

    for (name, service_config) in config.services {
        daemon.attach(Service::new(name, service_config.cmd))?;
    }

    log::info("daemon", "started");

    daemon.monitor();

    Ok(())
}
