use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;
use std::{env, fs};

use anyhow::Result;
use clap::Parser;
use glob::Pattern;
use libc::{SIGCHLD, SIGINT, SIGTERM};
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use signal_hook::flag;
use signal_hook::iterator::{Handle, Signals};

use crate::config::Config;
use crate::daemon::Daemon;
use crate::event::Event;
use crate::service::Service;

mod config;
mod daemon;
mod event;
mod process;
mod service;

#[derive(Debug, Parser)]
struct Args {
    #[clap(default_value = "up.toml")]
    filename: PathBuf,
}

fn load_config(filename: PathBuf) -> Result<Config> {
    let config = fs::read_to_string(filename)?;

    Ok(toml::from_str(&config)?)
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init_custom_env("UP_LOG");

    let args = Args::parse();
    let cwd = env::current_dir()?;
    let config = load_config(args.filename)?;

    // Set up the file watcher
    let (watcher_tx, watcher_rx) = channel();
    let mut watcher = watcher(watcher_tx, Duration::from_secs(1))?;
    watcher.watch(&cwd, RecursiveMode::Recursive)?;

    // Set up the event channel
    let (tx, rx) = channel();

    proxy_watcher_events(watcher_rx, tx.clone());

    // Set up the daemon
    let (mut daemon, stopped) = Daemon::new(cwd, rx);

    log::info!(target: "daemon", "starting..");

    // Forward signals
    flag::register(SIGTERM, Arc::clone(&stopped))?;
    flag::register(SIGINT, stopped)?;

    let proxy_handle = proxy_signals(tx.clone())?;

    // Start services
    for (name, cfg) in config.services {
        let mut patterns = vec![];

        for watch_pattern in cfg.watch.unwrap_or_default() {
            patterns.push(Pattern::new(&watch_pattern)?);
        }

        daemon.add(Service::new(name, cfg.cmd, patterns))?;
    }

    log::info!(target: "daemon", "started");

    daemon.run_event_loop();

    proxy_handle.close();

    log::info!(target: "daemon", "stopped");

    Ok(())
}

fn proxy_signals(tx: Sender<Event>) -> Result<Handle> {
    let mut signals = Signals::new(&[SIGCHLD, SIGTERM, SIGINT])?;
    let handle = signals.handle();

    tokio::spawn(async move {
        for signal in signals.forever() {
            let event = match signal {
                SIGCHLD => Event::ChildExited,
                SIGTERM | SIGINT => Event::WakeUp,
                _ => continue,
            };

            let _ = tx.send(event).ok();
        }
    });

    Ok(handle)
}

fn proxy_watcher_events(rx: Receiver<DebouncedEvent>, tx: Sender<Event>) {
    tokio::spawn(async move {
        'event_loop: for event in rx {
            match event {
                DebouncedEvent::Create(path)
                | DebouncedEvent::Write(path)
                | DebouncedEvent::Remove(path) => {
                    if tx.send(Event::FileChanged(path)).is_err() {
                        break 'event_loop;
                    }
                }
                _ => continue,
            }
        }
    });
}
