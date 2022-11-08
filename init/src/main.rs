use anyhow::{Context, Result};
use controller::{Controller, Event};
use signal_hook::{
    consts::{SIGCHLD, SIGINT, SIGTERM},
    iterator::{Handle, Signals},
};
use tokio::{
    sync::mpsc::{channel, Sender},
    task,
};

mod controller;
mod process;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let (tx, rx) = channel(10);
    let handle = handle_signals(tx)?;
    let controller = Controller::new();

    controller.run(rx).await?;
    handle.close();

    Ok(())
}

fn handle_signals(tx: Sender<Event>) -> Result<Handle> {
    let mut signals =
        Signals::new(&[SIGCHLD, SIGTERM, SIGINT]).with_context(|| "failed to register signals")?;
    let handle = signals.handle();

    task::spawn(async move {
        for signal in signals.forever() {
            let result = match signal {
                SIGCHLD => tx.send(Event::ChildExited).await,
                SIGTERM | SIGINT => tx.send(Event::Shutdown).await,
                _ => unreachable!(),
            };

            if let Err(e) = result {
                log::error!(target: "signals", "failed to send signal: {}", e);
                break;
            }
        }
    });

    Ok(handle)
}
