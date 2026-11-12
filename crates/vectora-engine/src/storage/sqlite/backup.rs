//! Consistent backup, verification and restore of a ledger database file.
//!
//! Two backup modes share one verification path. `backup_online` copies a
//! live ledger while writers keep committing. `backup_offline` copies the file
//! only after taking an exclusive lock, so the host must first stop its
//! writers; a writer that is still active makes the call fail with `Busy`.
//! Neither mode overwrites an existing file.

use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{Connection, OpenFlags, OptionalExtension, backup::Backup};
use serde::{Deserialize, Serialize};

use super::{LedgerError, SCHEMA_VERSION, SqliteLedger};

const PAGES_PER_STEP: i32 = 128;
const PAUSE_BETWEEN_STEPS: Duration = Duration::from_millis(2);

/// Facts established by opening and checking a ledger file.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BackupReport {
    pub schema_version: i64,
    pub events: u64,
    pub bytes: u64,
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn validate_destination(path: &Path) -> Result<(), LedgerError> {
    let has_parent = path
        .parent()
        .is_none_or(|parent| parent.as_os_str().is_empty() || parent.is_dir());
    if path.as_os_str().is_empty() || path == Path::new(":memory:") || !has_parent {
        return Err(LedgerError::InvalidInput);
    }
    if path.exists() {
        return Err(LedgerError::InvalidInput);
    }
    Ok(())
}

fn sync_file_and_directory(path: &Path) -> Result<(), LedgerError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|_| LedgerError::Storage)?;
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(|_| LedgerError::Storage)?;
    }
    Ok(())
}

/// Opens `path` read-only and checks integrity, schema version and contents.
pub fn verify_backup(path: impl AsRef<Path>) -> Result<BackupReport, LedgerError> {
    let path = path.as_ref();
    if !path.is_file() {
        return Err(LedgerError::InvalidInput);
    }
    let corrupt = |_| LedgerError::BackupInvalid;
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(corrupt)?;
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(corrupt)?;
    if integrity != "ok" {
        return Err(LedgerError::BackupInvalid);
    }
    let version: Option<i64> = connection
        .query_row("SELECT version FROM ve_ledger_meta WHERE id=1", [], |row| {
            row.get(0)
        })
        .optional()
        .map_err(corrupt)?;
    let schema_version = version.ok_or(LedgerError::BackupInvalid)?;
    if !(1..=SCHEMA_VERSION).contains(&schema_version) {
        return Err(LedgerError::SchemaVersion);
    }
    let events: i64 = connection
        .query_row("SELECT COUNT(*) FROM ve_ledger_events", [], |row| {
            row.get(0)
        })
        .map_err(corrupt)?;
    let bytes = fs::metadata(path).map_err(|_| LedgerError::Storage)?.len();
    Ok(BackupReport {
        schema_version,
        events: events as u64,
        bytes,
    })
}

/// Verifies the partial file, then publishes it atomically.
fn publish(partial: &Path, destination: &Path) -> Result<BackupReport, LedgerError> {
    let report = match verify_backup(partial) {
        Ok(report) => report,
        Err(error) => {
            let _ = fs::remove_file(partial);
            return Err(error);
        }
    };
    sync_file_and_directory(partial)?;
    fs::rename(partial, destination).map_err(|_| {
        let _ = fs::remove_file(partial);
        LedgerError::Storage
    })?;
    sync_file_and_directory(destination)?;
    Ok(report)
}

impl SqliteLedger {
    /// Copies this ledger while other connections may keep writing. The copy is
    /// a consistent snapshot of one committed state.
    pub fn backup_online(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<BackupReport, LedgerError> {
        let destination = destination.as_ref();
        validate_destination(destination)?;
        let partial = sibling(destination, ".partial");
        let _ = fs::remove_file(&partial);
        let copied = (|| -> Result<(), LedgerError> {
            let mut target = Connection::open(&partial)?;
            let backup = Backup::new(&self.connection, &mut target)?;
            backup.run_to_completion(PAGES_PER_STEP, PAUSE_BETWEEN_STEPS, None)?;
            Ok(())
        })();
        if let Err(error) = copied {
            let _ = fs::remove_file(&partial);
            return Err(error);
        }
        publish(&partial, destination)
    }

    /// Copies the ledger file at `source` after taking an exclusive lock. The
    /// host must have stopped its writers: an active writer yields `Busy`.
    pub fn backup_offline(
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<BackupReport, LedgerError> {
        let (source, destination) = (source.as_ref(), destination.as_ref());
        if !source.is_file() {
            return Err(LedgerError::InvalidInput);
        }
        validate_destination(destination)?;
        let locker = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        locker.busy_timeout(Duration::ZERO)?;
        locker
            .execute_batch("BEGIN EXCLUSIVE")
            .map_err(|_| LedgerError::Busy)?;
        let partial = sibling(destination, ".partial");
        let copied = fs::copy(source, &partial).map_err(|_| LedgerError::Storage);
        locker.execute_batch("ROLLBACK")?;
        if let Err(error) = copied {
            let _ = fs::remove_file(&partial);
            return Err(error);
        }
        publish(&partial, destination)
    }

    /// Restores a verified backup to `destination`. The host must have stopped
    /// every writer of `destination`. Without `replace`, an existing file is an
    /// error; with it, the previous file is kept as `<destination>.replaced`.
    pub fn restore(
        backup: impl AsRef<Path>,
        destination: impl AsRef<Path>,
        replace: bool,
    ) -> Result<BackupReport, LedgerError> {
        let (backup, destination) = (backup.as_ref(), destination.as_ref());
        verify_backup(backup)?;
        if destination.as_os_str().is_empty() || destination == Path::new(":memory:") {
            return Err(LedgerError::InvalidInput);
        }
        let replaced = sibling(destination, ".replaced");
        if destination.exists() {
            if !replace || replaced.exists() {
                return Err(LedgerError::InvalidInput);
            }
            let probe =
                Connection::open_with_flags(destination, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
            probe.busy_timeout(Duration::ZERO)?;
            probe
                .execute_batch("BEGIN EXCLUSIVE")
                .map_err(|_| LedgerError::Busy)?;
            probe.execute_batch("ROLLBACK")?;
        }
        let partial = sibling(destination, ".restoring");
        let _ = fs::remove_file(&partial);
        fs::copy(backup, &partial).map_err(|_| LedgerError::Storage)?;
        let report = match verify_backup(&partial) {
            Ok(report) => report,
            Err(error) => {
                let _ = fs::remove_file(&partial);
                return Err(error);
            }
        };
        sync_file_and_directory(&partial)?;
        if destination.exists() {
            fs::rename(destination, &replaced).map_err(|_| {
                let _ = fs::remove_file(&partial);
                LedgerError::Storage
            })?;
        }
        fs::rename(&partial, destination).map_err(|_| LedgerError::Storage)?;
        sync_file_and_directory(destination)?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::*;
    use crate::storage::sqlite::EventDraft;

    fn dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "vectora-backup-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn append(ledger: &mut SqliteLedger, n: u64) {
        ledger
            .append(EventDraft {
                scope: "scope".into(),
                aggregate: "run".into(),
                event_id: format!("e{n}"),
                idempotency_key: format!("k{n}"),
                event_type: "step".into(),
                payload: json!({"n": n}),
                expected_version: n - 1,
            })
            .unwrap();
    }

    fn sequences(path: &Path) -> Vec<u64> {
        SqliteLedger::open_persistent(path)
            .unwrap()
            .read("scope", "run", 0, 1000)
            .unwrap()
            .iter()
            .map(|event| event.sequence)
            .collect()
    }

    #[test]
    fn online_backup_is_a_consistent_snapshot_while_a_writer_commits() {
        let dir = dir();
        let source = dir.join("ledger.db");
        let mut ledger = SqliteLedger::open_persistent(&source).unwrap();
        for n in 1..=5 {
            append(&mut ledger, n);
        }
        let stop = Arc::new(AtomicBool::new(false));
        let writer = {
            let (source, stop) = (source.clone(), Arc::clone(&stop));
            std::thread::spawn(move || {
                let mut writer = SqliteLedger::open_persistent(source).unwrap();
                let mut n = 6;
                while !stop.load(Ordering::Relaxed) {
                    if writer
                        .append(EventDraft {
                            scope: "scope".into(),
                            aggregate: "run".into(),
                            event_id: format!("e{n}"),
                            idempotency_key: format!("k{n}"),
                            event_type: "step".into(),
                            payload: json!({"n": n}),
                            expected_version: n - 1,
                        })
                        .is_ok()
                    {
                        n += 1;
                    }
                }
            })
        };
        let backup = dir.join("backup.db");
        let report = ledger.backup_online(&backup).unwrap();
        stop.store(true, Ordering::Relaxed);
        writer.join().unwrap();

        assert!(report.events >= 5);
        let copied = sequences(&backup);
        assert_eq!(copied, (1..=copied.len() as u64).collect::<Vec<_>>());
        assert_eq!(copied.len() as u64, report.events);
        assert!(sequences(&source).len() >= copied.len());
        assert!(!sibling(&backup, ".partial").exists());

        assert!(matches!(
            ledger.backup_online(&backup),
            Err(LedgerError::InvalidInput)
        ));
        assert!(matches!(
            ledger.backup_online(dir.join("missing").join("b.db")),
            Err(LedgerError::InvalidInput)
        ));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn offline_backup_fails_busy_while_a_writer_is_active() {
        let dir = dir();
        let source = dir.join("ledger.db");
        let mut ledger = SqliteLedger::open_persistent(&source).unwrap();
        append(&mut ledger, 1);
        append(&mut ledger, 2);
        let backup = dir.join("backup.db");

        let writer = Connection::open(&source).unwrap();
        writer.execute_batch("BEGIN IMMEDIATE").unwrap();
        assert!(matches!(
            SqliteLedger::backup_offline(&source, &backup),
            Err(LedgerError::Busy)
        ));
        assert!(!backup.exists() && !sibling(&backup, ".partial").exists());
        writer.execute_batch("ROLLBACK").unwrap();
        drop(writer);

        let report = SqliteLedger::backup_offline(&source, &backup).unwrap();
        assert_eq!(report.events, 2);
        assert_eq!(sequences(&backup), [1, 2]);
        assert!(matches!(
            SqliteLedger::backup_offline(&source, &backup),
            Err(LedgerError::InvalidInput)
        ));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn restore_returns_to_the_backup_state_and_keeps_the_replaced_file() {
        let dir = dir();
        let live = dir.join("ledger.db");
        let backup = dir.join("backup.db");
        let mut ledger = SqliteLedger::open_persistent(&live).unwrap();
        append(&mut ledger, 1);
        append(&mut ledger, 2);
        ledger.backup_online(&backup).unwrap();
        append(&mut ledger, 3);
        append(&mut ledger, 4);
        drop(ledger);

        assert!(matches!(
            SqliteLedger::restore(&backup, &live, false),
            Err(LedgerError::InvalidInput)
        ));
        assert_eq!(
            sequences(&live),
            [1, 2, 3, 4],
            "a refused restore changes nothing"
        );

        let report = SqliteLedger::restore(&backup, &live, true).unwrap();
        assert_eq!(report.events, 2);
        assert_eq!(sequences(&live), [1, 2]);
        assert_eq!(
            verify_backup(sibling(&live, ".replaced")).unwrap().events,
            4,
            "the previous state must remain recoverable"
        );
        assert!(matches!(
            SqliteLedger::restore(&backup, &live, true),
            Err(LedgerError::InvalidInput)
        ));

        let fresh = dir.join("fresh.db");
        SqliteLedger::restore(&backup, &fresh, false).unwrap();
        assert_eq!(sequences(&fresh), [1, 2]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn restore_refuses_a_destination_with_an_active_writer() {
        let dir = dir();
        let live = dir.join("ledger.db");
        let backup = dir.join("backup.db");
        let mut ledger = SqliteLedger::open_persistent(&live).unwrap();
        append(&mut ledger, 1);
        ledger.backup_online(&backup).unwrap();
        append(&mut ledger, 2);
        let writer = Connection::open(&live).unwrap();
        writer.execute_batch("BEGIN IMMEDIATE").unwrap();
        assert!(matches!(
            SqliteLedger::restore(&backup, &live, true),
            Err(LedgerError::Busy)
        ));
        writer.execute_batch("ROLLBACK").unwrap();
        drop((writer, ledger));
        assert_eq!(sequences(&live), [1, 2]);
        assert!(!sibling(&live, ".replaced").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn corrupt_foreign_and_newer_schema_files_are_rejected_without_touching_the_destination() {
        let dir = dir();
        let live = dir.join("ledger.db");
        let mut ledger = SqliteLedger::open_persistent(&live).unwrap();
        append(&mut ledger, 1);
        drop(ledger);

        let garbage = dir.join("garbage.db");
        fs::write(&garbage, b"not a sqlite database, just bytes").unwrap();
        let foreign = dir.join("foreign.db");
        Connection::open(&foreign)
            .unwrap()
            .execute_batch("CREATE TABLE other(x)")
            .unwrap();
        let newer = dir.join("newer.db");
        fs::copy(&live, &newer).unwrap();
        Connection::open(&newer)
            .unwrap()
            .execute_batch("UPDATE ve_ledger_meta SET version=99 WHERE id=1")
            .unwrap();

        assert!(matches!(
            verify_backup(&garbage),
            Err(LedgerError::BackupInvalid)
        ));
        assert!(matches!(
            verify_backup(&foreign),
            Err(LedgerError::BackupInvalid)
        ));
        assert!(matches!(
            verify_backup(&newer),
            Err(LedgerError::SchemaVersion)
        ));
        assert!(matches!(
            verify_backup(dir.join("missing.db")),
            Err(LedgerError::InvalidInput)
        ));
        for bad in [&garbage, &foreign, &newer] {
            assert!(SqliteLedger::restore(bad, &live, true).is_err());
        }
        assert_eq!(sequences(&live), [1]);
        assert!(!sibling(&live, ".replaced").exists());
        assert!(!sibling(&live, ".restoring").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
