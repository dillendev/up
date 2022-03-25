use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::log;
use crate::service::Service;

fn check_service(service: &mut Service) -> Result<()> {
    if service.is_running() {
        return Ok(());
    }

    log::info(
        "daemon",
        format!("service '{}' stopped, restarting", service.name),
    );

    service.detach();
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
    stopped: Rc<AtomicBool>,
    services: Vec<Service>,
}

impl Daemon {
    pub fn new() -> (Self, StopHandler) {
        let stopped = Rc::new(AtomicBool::new(false));

        (
            Self {
                services: vec![],
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

    pub fn monitor(mut self) {
        while !self.stopped.load(Ordering::Relaxed) {
            for service in self.services.iter_mut() {
                if let Err(err) = check_service(service) {
                    log::error(
                        "daemon",
                        format!("failed to check service '{}': {}", service.name, err),
                    );
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
