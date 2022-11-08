use std::ffi::CString;
use std::fmt::{Display, Formatter};

use anyhow::Result;
use nix::libc;
use nix::sys::signal::{kill, killpg, Signal};
use nix::sys::wait::waitpid;
use nix::unistd::{execv, fork, getpgid, setsid, ForkResult, Pid};

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

    /// Sends a signal to the process group
    pub fn kill<T: Into<Option<Signal>>>(self, signal: T) -> Result<()> {
        let pgid = getpgid(Some(self.pid))?;
        killpg(pgid, signal)?;
        waitpid(pgid, None).ok();

        Ok(())
    }

    /// Runs the process using `fork` and `exec` and returns the `Process`
    pub fn run(argv: &[&str]) -> Result<Process> {
        match unsafe { fork()? } {
            ForkResult::Parent { child, .. } => Ok(Process::new(child)),
            ForkResult::Child => {
                // Create a new session so that we can track all child processes
                setsid()?;

                let mut c_argv = vec![];

                for arg in argv {
                    c_argv.push(CString::new(*arg)?);
                }

                if execv(c_argv[0].as_c_str(), &c_argv[1..]).is_err() {
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
