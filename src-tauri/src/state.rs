use std::sync::Arc;
use std::sync::Mutex;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::ChildStdout;

pub type SharedChild = Arc<Mutex<Option<Child>>>;
pub type SharedScanReader = Arc<Mutex<Option<ScanState>>>;

#[derive(Default)]
pub struct RobocopyState {
    pub child: SharedChild,
}

#[derive(Default)]
pub struct ScanState {
    pub child: Option<Child>,
    pub reader: Option<BufReader<ChildStdout>>,
    pub file_count: u64,
    pub total_bytes: u64,
    pub done: bool,
}

pub fn shared_child() -> SharedChild {
    Arc::new(Mutex::new(None))
}
