use std::ffi::CString;

use anyhow::{anyhow, Result};
use nix::sys::signal::{kill, Signal};
use nix::unistd::{execv, fork, ForkResult, Pid};

use crate::log;

//use nix::sys::wait::waitpid;
//waitpid(child, None)?;

macro_rules! cstr {
    ($s:expr) => {
        CString::new($s).unwrap().as_c_str()
    };
}

pub struct Service {
    pub name: String,
    pub cmd: String,
    pub pid: Option<Pid>,
}

impl Service {
    pub fn new(name: String, cmd: String) -> Self {
        Self {
            name,
            cmd,
            pid: None,
        }
    }

    pub fn is_running(&self) -> bool {
        match self.pid {
            None => false,
            Some(pid) => kill(pid, None).is_ok(),
        }
    }

    pub fn start(&mut self) -> Result<()> {
        if self.pid.is_some() {
            return Err(anyhow!("service '{}' already started", self.name));
        }

        match unsafe { fork()? } {
            ForkResult::Parent { child, .. } => {
                log::info(
                    format!("service/{}", self.name),
                    format!("started (pid={})", child),
                );

                self.pid = Some(child);

                Ok(())
            }
            ForkResult::Child => {
                execv(
                    cstr!("/bin/sh"),
                    &[cstr!("/bin/sh"), cstr!("-c"), cstr!(self.cmd.as_str())],
                )?;

                unsafe { libc::_exit(0) }
            }
        }
    }

    pub fn detach(&mut self) {
        self.pid.take();
    }

    pub fn stop(&mut self) -> Result<()> {
        let pid = match self.pid.take() {
            Some(pid) => pid,
            None => return Ok(()),
        };

        kill(pid, Signal::SIGTERM)?;
        log::info(format!("service/{}", self.name), "stopped");

        Ok(())
    }
}
