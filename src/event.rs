use std::path::PathBuf;

#[derive(Debug)]
pub enum Event {
    WakeUp,
    FileChanged(PathBuf),
    ChildExited,
}

unsafe impl Send for Event {}
