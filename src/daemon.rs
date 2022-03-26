use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{anyhow, Result};
use notify::DebouncedEvent;

use crate::log;
use crate::service::Service;

fn check_service(service: &mut Service) -> Result<()> {
    if service.is_up() {
        return Ok(());
    }

    log::info(
        "daemon",
        format!("service '{}' stopped, restarting", service.name),
    );

    service.stop()?;
    service.start()
}

fn restart_service(service: &mut Service) -> Result<()> {
    log::info("daemon", format!("restarting service '{}'", service.name));

    service.stop()?;
    service.start()
}

#[derive(Clone)]
pub struct StopHandler {
    stopped: Rc<AtomicBool>,
}

unsafe impl Send for StopHandler {}

impl StopHandler {
    pub fn stop(self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

pub struct Daemon {
    root: PathBuf,
    stopped: Rc<AtomicBool>,
    services: Vec<Service>,
    file_changes: Receiver<DebouncedEvent>,
}

impl Daemon {
    pub fn new(root: PathBuf, file_changes: Receiver<DebouncedEvent>) -> (Self, StopHandler) {
        let stopped = Rc::new(AtomicBool::new(false));

        (
            Self {
                root,
                services: vec![],
                file_changes,
                stopped: Rc::clone(&stopped),
            },
            StopHandler { stopped },
        )
    }

    pub fn attach(&mut self, service: Service) -> Result<()> {
        self.services.push(service);

        let service = self
            .services
            .last_mut()
            .ok_or_else(|| anyhow!("no such service"))?;

        log::info("daemon", format!("starting service '{}'", service.name));

        service.start()?;

        Ok(())
    }

    fn process_file_changes(&mut self) -> HashMap<String, bool> {
        let mut marked_restart = HashMap::new();

        while let Ok(event) = self.file_changes.try_recv() {
            match event {
                DebouncedEvent::NoticeWrite(_) => {}
                DebouncedEvent::NoticeRemove(_) => {}
                DebouncedEvent::Chmod(_) => {}
                DebouncedEvent::Create(path)
                | DebouncedEvent::Write(path)
                | DebouncedEvent::Remove(path) => {
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
                DebouncedEvent::Rename(_, _) => {}
                DebouncedEvent::Rescan => {}
                DebouncedEvent::Error(_, _) => {}
            }
        }

        marked_restart
    }

    pub fn monitor(mut self) {
        while !self.stopped.load(Ordering::Relaxed) {
            let marked_restart = self.process_file_changes();

            // Make sure all services are healthy
            for service in self.services.iter_mut() {
                if let Err(err) = check_service(service) {
                    log::error(
                        "daemon",
                        format!("failed to check service '{}': {}", service.name, err),
                    );

                    continue;
                }

                if marked_restart.contains_key(&service.name) {
                    if let Err(err) = restart_service(service) {
                        log::error(
                            "daemon",
                            format!("failed to restart service '{}': {}", service.name, err),
                        );
                    }
                }
            }

            sleep(Duration::from_secs(5));
        }

        self.shutdown();
    }

    pub fn shutdown(&mut self) {
        let mut i = 0;

        while !self.services.is_empty() {
            let service = &mut self.services[i];

            log::info("daemon", format!("stopping service '{}'", service.name));

            if let Err(err) = service.stop() {
                log::error(
                    "daemon",
                    format!("failed to stop service '{}': {}", service.name, err),
                );
                continue;
            }

            self.services.remove(i);

            i = 0;
        }
    }
}
