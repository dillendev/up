use std::time::Duration;

use anyhow::{anyhow, Result};
use glob::Pattern;

use crate::log;
use crate::process::Process;

pub struct Service {
    pub name: String,
    pub cmd: String,
    pub proc: Option<Process>,
    pub watched_paths: Vec<Pattern>,
}

impl Service {
    pub fn new(name: String, cmd: String, watched_paths: Vec<Pattern>) -> Self {
        Self {
            name,
            cmd,
            proc: None,
            watched_paths,
        }
    }

    pub fn is_up(&self) -> bool {
        match &self.proc {
            None => false,
            Some(proc) => proc.is_running(),
        }
    }

    pub fn start(&mut self) -> Result<()> {
        if self.proc.is_some() {
            return Err(anyhow!("service '{}' already started", self.name));
        }

        let proc = Process::run(&["/bin/sh", "-c", self.cmd.as_str()])?;

        log::info(
            format!("service/{}", self.name),
            format!("started (pid={})", proc),
        );

        self.proc = Some(proc);

        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        let proc = match self.proc.take() {
            Some(proc) => proc,
            None => return Ok(()),
        };

        proc.stop(Duration::from_secs(10))?;

        log::info(format!("service/{}", self.name), "stopped");

        Ok(())
    }
}
