use std::sync::atomic::{AtomicU64, Ordering};

static PLUGIN_NS: AtomicU64 = AtomicU64::new(0);

pub fn record_plugin_ns(ns: u64) {
  PLUGIN_NS.fetch_add(ns, Ordering::Relaxed);
}

pub fn take_plugin_ns() -> u64 {
  PLUGIN_NS.swap(0, Ordering::Relaxed)
}
