use std::sync::atomic::{AtomicU64, Ordering};

static POSTCSS_NS: AtomicU64 = AtomicU64::new(0);
static POSTCSS_PLUGIN_NS: AtomicU64 = AtomicU64::new(0);

/// Record elapsed nanoseconds spent inside the PostCSS pipeline.
pub fn record_postcss_ns(ns: u64) {
  POSTCSS_NS.fetch_add(ns, Ordering::Relaxed);
}

/// Return accumulated PostCSS nanoseconds (resetting the counter).
pub fn take_postcss_ns() -> u64 {
  POSTCSS_NS.swap(0, Ordering::Relaxed)
}

/// Record elapsed nanoseconds spent inside PostCSS plugins.
pub fn record_postcss_plugin_ns(ns: u64) {
  POSTCSS_PLUGIN_NS.fetch_add(ns, Ordering::Relaxed);
}

/// Return accumulated PostCSS plugin nanoseconds (resetting the counter).
pub fn take_postcss_plugin_ns() -> u64 {
  POSTCSS_PLUGIN_NS.swap(0, Ordering::Relaxed)
}
