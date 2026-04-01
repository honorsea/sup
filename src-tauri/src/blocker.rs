// src-tauri/src/blocker.rs
//
// Adblock engine wrapper using Brave's adblock-rust (v0.12).
//
// The engine is stored in a OnceCell — a single atomic read after init, no
// locking required for concurrent access. Initialization is dispatched to a
// blocking thread pool thread (spawn_blocking in lib.rs) so startup latency
// is hidden behind window creation.
//
// NOTE: should_block() is ready for future use when Tauri exposes a WebView2
// network interception API. The engine is kept warmed up so the first call
// has no cold-start cost.

use adblock::{
    engine::Engine,
    lists::{FilterSet, ParseOptions},
};
use once_cell::sync::OnceCell;

static BLOCKER: OnceCell<Engine> = OnceCell::new();

/// Embedded filter rules — compiled into the binary, zero I/O at runtime.
const BUNDLED_RULES: &str = include_str!("rules.txt");

/// Initialize the adblock engine. Idempotent; safe to call from any thread.
/// Dispatched via `spawn_blocking` in lib.rs so it doesn't stall startup.
pub fn init() {
    BLOCKER.get_or_init(|| {
        let mut filter_set = FilterSet::new(true);
        let rules: Vec<String> = BUNDLED_RULES
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !t.is_empty() && !t.starts_with('!')
            })
            .map(str::to_owned)
            .collect();
        filter_set.add_filters(rules, ParseOptions::default());
        // `true` = enable optimizer: O(1) domain lookup after one-time setup.
        Engine::from_filter_set(filter_set, true)
    });
}
