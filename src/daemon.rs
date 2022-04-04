use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use nix::sys::wait::WaitStatus::StillAlive;
use nix::sys::wait::{waitpid, WaitPidFlag};

use crate::service::Service;
use crate::Event;

const WAIT_DURATION: Duration = Duration::from_secs(3);
const RESTART_WAIT_DURATION: Duration = Duration::from_secs(1);

fn restart_service(service: &mut Service, flags: &Flags) -> Result<()> {
    log::info!(target: "daemon", "restarting service '{}' (attempt {})", service.name, flags.restart_attempts);

    service.stop()?;
    service.start()
}

#[derive(Default)]
struct Flags {
    restart: bool,
    restart_attempts: u32,
    restarted_at: Option<Instant>,
}

impl Flags {
    pub fn can_be_restarted(&self) -> bool {
        self.restarted_at
            .map(|t| t.elapsed() >= RESTART_WAIT_DURATION * (self.restart_attempts + 1).pow(2))
            .unwrap_or(true)
    }
}

pub struct Daemon {
    root: PathBuf,
    events: Receiver<Event>,
    stopped: Arc<AtomicBool>,
    services: Vec<(Service, Flags)>,
}

impl Daemon {
    pub fn new(root: PathBuf, events: Receiver<Event>) -> (Self, Arc<AtomicBool>) {
        let stopped = Arc::new(AtomicBool::new(false));

        (
            Self {
                root,
                events,
                services: vec![],
                stopped: Arc::clone(&stopped),
            },
            stopped,
        )
    }

    pub fn add(&mut self, service: Service) -> Result<()> {
        log::info!(target: "daemon", "adding service '{}'", service.name);

        self.services.push((service, Flags::default()));

        Ok(())
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::FileChanged(path) => {
                let path = path.strip_prefix(&self.root).unwrap_or(&path);

                for (_, flags) in self
                    .services
                    .iter_mut()
                    .filter(|(s, _)| s.watched_paths.iter().any(|p| p.matches_path(path)))
                {
                    flags.restart = true;
                }
            }
            Event::ChildExited => {
                // Make sure zombie processes are cleaned up
                waitpid(None, None).ok();
            }
            Event::WakeUp => {}
        }
    }

    fn handle_events_wait(&mut self, timeout: Duration) {
        if let Ok(event) = self.events.recv_timeout(timeout) {
            self.handle_event(event);

            while let Ok(event) = self.events.try_recv() {
                self.handle_event(event);
            }
        }
    }

    pub fn run_event_loop(mut self) {
        log::info!(target: "daemon", "starting event loop");

        for (service, _) in self.services.iter_mut() {
            if let Err(err) = service.start() {
                log::error!(
                    target: "daemon",
                    "failed to start service '{}': {}", service.name, err,
                );

                continue;
            }
        }

        while !self.stopped.load(Ordering::Relaxed) {
            // Handle pending events synchronously
            self.handle_events_wait(WAIT_DURATION);

            if self.stopped.load(Ordering::Relaxed) {
                break;
            }

            // Make sure all services are healthy
            for (service, flags) in self.services.iter_mut() {
                if !service.is_up() {
                    flags.restart = true;
                }

                if flags.restart {
                    if !flags.can_be_restarted() {
                        continue;
                    }

                    flags.restart_attempts += 1;
                    flags.restarted_at = Some(Instant::now());

                    if let Err(err) = restart_service(service, flags) {
                        log::error!(
                            target: "daemon",
                            "failed to restart service '{}': {}", service.name, err,
                        );

                        continue;
                    }

                    flags.restart = false;

                    continue;
                }

                if flags.can_be_restarted() {
                    flags.restarted_at = None;
                }
            }
        }

        log::info!(target: "daemon", "stopping event loop");

        self.shutdown();
    }

    pub fn shutdown(&mut self) {
        log::info!(target: "daemon", "shutting down");

        // Cleanup zombie processes before shutting down
        loop {
            match waitpid(None, Some(WaitPidFlag::WNOHANG)) {
                Ok(StillAlive) | Err(_) => break,
                Ok(_) => continue,
            }
        }

        let mut i = 0;

        while !self.services.is_empty() {
            let (service, _) = &mut self.services[i];

            log::info!(target: "daemon", "stopping service '{}'", service.name);

            if let Err(err) = service.stop() {
                log::error!(
                    target: "daemon",
                    "failed to stop service '{}': {}", service.name, err,
                );
                continue;
            }

            self.services.remove(i);

            i = 0;
        }
    }
}
