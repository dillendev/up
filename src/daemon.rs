use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{anyhow, Result};
use nix::sys::wait::WaitStatus::StillAlive;
use nix::sys::wait::{waitpid, WaitPidFlag};

use crate::service::Service;
use crate::Event;

fn check_service(service: &mut Service) -> Result<()> {
    if service.is_up() {
        return Ok(());
    }

    log::info!(
        target: "daemon",
        "service '{}' stopped, restarting", service.name,
    );

    service.stop()?;
    service.start()
}

fn restart_service(service: &mut Service) -> Result<()> {
    log::info!(target: "daemon", "restarting service '{}'", service.name);

    service.stop()?;
    service.start()
}

pub struct Daemon {
    root: PathBuf,
    events: Receiver<Event>,
    stopped: Arc<AtomicBool>,
    services: Vec<Service>,
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

    pub fn attach(&mut self, service: Service) -> Result<()> {
        self.services.push(service);

        let service = self
            .services
            .last_mut()
            .ok_or_else(|| anyhow!("no such service"))?;

        log::info!(target: "daemon", "starting service '{}'", service.name);

        service.start()?;

        Ok(())
    }

    fn process_events(&mut self) -> HashMap<String, bool> {
        let mut marked_restart = HashMap::new();

        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::FileChanged(path) => {
                    let path = path.strip_prefix(&self.root).unwrap_or(&path);

                    for service in self
                        .services
                        .iter()
                        .filter(|s| s.watched_paths.iter().any(|p| p.matches_path(path)))
                    {
                        if !marked_restart.contains_key(&service.name) {
                            marked_restart.insert(service.name.clone(), true);
                        }
                    }
                }
                Event::ChildExited => {
                    // Make sure zombie processes are cleaned up
                    waitpid(None, None).ok();
                }
            }
        }

        marked_restart
    }

    pub fn monitor(mut self) {
        while !self.stopped.load(Ordering::Relaxed) {
            let marked_restart = self.process_events();

            // Make sure all services are healthy
            for service in self.services.iter_mut() {
                if let Err(err) = check_service(service) {
                    log::error!(
                        target: "daemon",
                        "failed to check service '{}': {}", service.name, err,
                    );

                    continue;
                }

                if marked_restart.contains_key(&service.name) {
                    if let Err(err) = restart_service(service) {
                        log::error!(
                            target: "daemon",
                            "failed to restart service '{}': {}", service.name, err,
                        );
                    }
                }
            }

            sleep(Duration::from_secs(5));
        }

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
            let service = &mut self.services[i];

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
