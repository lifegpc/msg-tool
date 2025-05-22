use crate::types::*;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;

pub struct Counter {
    ok: AtomicUsize,
    ignored: AtomicUsize,
    error: AtomicUsize,
    warning: AtomicUsize,
}

impl Counter {
    pub fn new() -> Self {
        Self {
            ok: AtomicUsize::new(0),
            ignored: AtomicUsize::new(0),
            error: AtomicUsize::new(0),
            warning: AtomicUsize::new(0),
        }
    }

    pub fn inc_error(&self) {
        self.error.fetch_add(1, SeqCst);
    }

    pub fn inc_warning(&self) {
        self.warning.fetch_add(1, SeqCst);
    }

    pub fn inc(&self, result: ScriptResult) {
        match result {
            ScriptResult::Ok => self.ok.fetch_add(1, SeqCst),
            ScriptResult::Ignored => self.ignored.fetch_add(1, SeqCst),
        };
    }
}

impl std::fmt::Display for Counter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "OK: {}, Ignored: {}, Error: {}, Warning: {}",
            self.ok.load(SeqCst),
            self.ignored.load(SeqCst),
            self.error.load(SeqCst),
            self.warning.load(SeqCst),
        )
    }
}
