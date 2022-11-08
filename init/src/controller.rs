use anyhow::Result;
use nix::sys::wait::waitpid;
use tokio::sync::mpsc::Receiver;

#[derive(PartialEq)]
enum Flow {
    Stop,
    Continue,
}

#[derive(Debug)]
pub enum Event {
    ChildExited,
    Shutdown,
}

pub struct Controller {}

impl Controller {
    pub fn new() -> Self {
        Self {}
    }

    fn handle_event(&self, event: Event) -> Result<Flow> {
        match event {
            Event::ChildExited => {
                // Make sure zombie processes are cleaned up
                waitpid(None, None).ok();
            }
            Event::Shutdown => {
                return Ok(Flow::Stop);
            }
        }

        Ok(Flow::Continue)
    }

    pub async fn run(self, mut rx: Receiver<Event>) -> Result<()> {
        while let Some(event) = rx.recv().await {
            log::debug!(target: "controller", "received event: {:#?}", event);

            if self.handle_event(event)? == Flow::Stop {
                break;
            }
        }

        log::info!(target: "controller", "shutdown");

        Ok(())
    }
}
