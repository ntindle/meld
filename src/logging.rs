use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE: AtomicBool = AtomicBool::new(false);
static JSON_MODE: AtomicBool = AtomicBool::new(false);

pub fn set_verbose(v: bool) {
    VERBOSE.store(v, Ordering::Relaxed);
}

pub fn set_json(v: bool) {
    JSON_MODE.store(v, Ordering::Relaxed);
}

pub fn json_mode() -> bool {
    JSON_MODE.load(Ordering::Relaxed)
}

pub fn info(msg: &str) {
    if !json_mode() {
        println!("{msg}");
    }
}

pub fn verbose(msg: &str) {
    if VERBOSE.load(Ordering::Relaxed) && !json_mode() {
        println!("  {msg}");
    }
}

pub fn warn(msg: &str) {
    if !json_mode() {
        eprintln!("warning: {msg}");
    }
}
