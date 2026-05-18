//! One-shot config / data / cache directory migration from the legacy
//! `ursly` brand name to the current `diaspor` name.
//!
//! Anyone who installed v0.1.0-alpha.4 or earlier has their settings,
//! audit log, and cache state under `$XDG_*/ursly`. The rebrand in
//! v0.1.0-alpha.5 moved every `.join("ursly")` callsite to
//! `.join("diaspor")`, which would silently orphan that state.
//!
//! This module runs once at startup, before any subsystem reads its
//! state, and renames the old directory to the new one — but only if
//! the new path doesn't already exist. That way:
//!   - Fresh installs: noop (no old dir present).
//!   - Upgrading from alpha.4: state preserved.
//!   - Re-runs: noop (new dir exists, old left alone if both exist).
//!
//! Failures are logged at WARN and swallowed: a migration error must not
//! prevent the app from launching.

use std::path::PathBuf;

/// Migrate every known `<dir>/ursly` directory to `<dir>/diaspor`.
/// Returns the count of directories actually moved (for logging).
pub fn migrate_ursly_to_diaspor() -> usize {
    let kinds: [(&str, Option<PathBuf>); 3] = [
        ("config", dirs::config_dir()),
        ("data", dirs::data_dir()),
        ("cache", dirs::cache_dir()),
    ];

    let mut moved = 0_usize;
    for (kind, root) in kinds {
        let Some(root) = root else { continue };
        let old = root.join("ursly");
        let new = root.join("diaspor");

        if !old.exists() {
            continue;
        }
        if new.exists() {
            tracing::debug!(
                "migrate_config: {kind}/diaspor already exists; leaving \
                 {kind}/ursly in place at {}",
                old.display()
            );
            continue;
        }

        match std::fs::rename(&old, &new) {
            Ok(()) => {
                tracing::info!(
                    "migrate_config: moved {} to {} ({} state)",
                    old.display(),
                    new.display(),
                    kind
                );
                moved += 1;
            }
            Err(e) => {
                tracing::warn!(
                    "migrate_config: failed to rename {} -> {}: {e}",
                    old.display(),
                    new.display()
                );
            }
        }
    }
    moved
}
