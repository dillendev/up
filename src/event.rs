use std::path::PathBuf;

#[derive(Debug)]
pub enum Event {
    FileChanged(PathBuf),
    ChildExited,
}

unsafe impl Send for Event {}
