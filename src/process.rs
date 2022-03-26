use std::ffi::CString;
use std::fmt::{Display, Formatter};
use std::time::Duration;

use anyhow::Result;
use nix::sys::signal::{kill, killpg, Signal};
use nix::sys::wait::waitpid;
use nix::unistd::{execv, fork, getpgid, setsid, write, ForkResult, Pid};

macro_rules! cstr {
    ($s:expr) => {
        CString::new($s).unwrap()
    };
}

pub struct Process {
    pid: Pid,
}

impl Process {
    pub fn new(pid: Pid) -> Process {
        Process { pid }
    }

    pub fn is_running(&self) -> bool {
        kill(self.pid, None).is_ok()
    }

    /// Stops the process by sending SIGTERM to the process group
    pub fn stop(self, wait: Duration) -> Result<()> {
        if !self.is_running() {
            return Ok(());
        }

        let pgid = getpgid(Some(self.pid))?;
        killpg(pgid, Signal::SIGTERM)?;
        waitpid(self.pid, None)?;

        // @TODO: SIGKILL after `wait` time
        //log::error(
        //    format!("process/{}", self.pid),
        //    "failed to terminate cleanly, forcing to stop",
        //);
        //kill(self.pid, Signal::SIGKILL)?;

        Ok(())
    }

    /// Runs the process using `fork` and `exec` and returns the `Process`
    pub fn run(argv: &[&str]) -> Result<Process> {
        match unsafe { fork()? } {
            ForkResult::Parent { child, .. } => Ok(Process::new(child)),
            ForkResult::Child => {
                // Create a new session so that we can track all child processes
                setsid()?;

                if let Err(err) = execv(
                    &cstr!(argv[0]),
                    argv.iter()
                        .map(|arg| cstr!(*arg))
                        .collect::<Vec<_>>()
                        .as_slice(),
                ) {
                    write(
                        libc::STDERR_FILENO,
                        format!("execv failed: {}\n", err).as_bytes(),
                    )
                    .ok();
                    unsafe {
                        libc::_exit(1);
                    }
                }

                unreachable!()
            }
        }
    }
}

impl Display for Process {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.pid)
    }
}
