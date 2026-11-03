//! Backup discovery, verification, restore, and lossless library export/import.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;

use maktabati_storage::{conn, migrate, Db, Result, StorageError};

const EXPORT_FORMAT_VERSION: u32 = 1;

/// One application table in a portable JSON export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableExport {
    /// Ordered column names from the source database.
    pub columns: Vec<String>,
    /// Rows keyed by column name, making the file readable and resilient to column ordering.
    pub rows: Vec<BTreeMap<String, serde_json::Value>>,
}

/// Complete portable export of the library database.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryExport {
    /// Export format version, separate from the SQLite schema version.
    pub format_version: u32,
    /// Highest applied SQLite migration.
    pub schema_version: i64,
    /// Export creation time in epoch milliseconds.
    pub exported_at_ms: i64,
    /// Every regular application table, including search projections and settings.
    pub tables: BTreeMap<String, TableExport>,
}

/// A backup file found in the app's backup directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct BackupInfo {
    /// Path passed back to verify/restore.
    pub path: String,
    /// File name for display.
    pub name: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Modification time in epoch milliseconds.
    pub modified_at_ms: i64,
}

/// Result of checking a backup file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct BackupVerification {
    /// Original path of the candidate backup.
    pub path: String,
    /// `PRAGMA integrity_check` result.
    pub integrity: String,
    /// Whether SQLite reports exactly `ok`.
    pub integrity_ok: bool,
    /// Schema migration version stored in the backup, or `None` when absent.
    pub schema_version: Option<i64>,
    /// Whether the schema version matches the current application.
    pub schema_compatible: bool,
    /// Overall restore readiness.
    pub valid: bool,
}

/// Result of a successful restore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RestoreResult {
    /// Path of the automatic pre-restore safety snapshot.
    pub safety_snapshot: String,
    /// Schema version now active in the database.
    pub schema_version: i64,
}

/// List regular `.db` files in `backup_dir`, newest first.
///
/// # Errors
///
/// Returns an I/O error when the directory or its entries cannot be read.
pub fn list_backups(backup_dir: &Path) -> Result<Vec<BackupInfo>> {
    if !backup_dir.exists() {
        return Ok(Vec::new());
    }

    let mut backups = Vec::new();
    for entry in fs::read_dir(backup_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || !entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("db"))
        {
            continue;
        }
        let metadata = entry.metadata()?;
        let modified_at_ms = i64::try_from(
            metadata
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| StorageError::Migration(error.to_string()))?
                .as_millis(),
        )
        .map_err(|error| StorageError::Migration(error.to_string()))?;
        backups.push(BackupInfo {
            path: entry.path().display().to_string(),
            name: entry.file_name().to_string_lossy().into_owned(),
            size_bytes: metadata.len(),
            modified_at_ms,
        });
    }
    backups.sort_by(|left, right| {
        right
            .modified_at_ms
            .cmp(&left.modified_at_ms)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(backups)
}

/// Verify SQLite integrity and exact schema compatibility for a backup under `backup_dir`.
///
/// # Errors
///
/// Returns an error when the path is outside the backup directory or cannot be opened.
pub fn verify_backup(
    backup_dir: &Path,
    path: &Path,
    expected_schema_version: i64,
) -> Result<BackupVerification> {
    let canonical_dir = fs::canonicalize(backup_dir)?;
    let canonical_path = fs::canonicalize(path)?;
    if !canonical_path.starts_with(&canonical_dir)
        || !canonical_path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("db"))
    {
        return Err(StorageError::Migration(
            "backup must be a .db file in the application backup directory".to_owned(),
        ));
    }

    let connection = Connection::open_with_flags(
        &canonical_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )?;
    let integrity_results = {
        let mut statement = connection.prepare("PRAGMA integrity_check")?;
        let results = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        results
    };
    let integrity = if integrity_results.is_empty() {
        "integrity_check returned no results".to_owned()
    } else {
        integrity_results.join("; ")
    };
    let has_migrations: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='_migrations')",
        [],
        |row| row.get(0),
    )?;
    let schema_version = if has_migrations {
        connection.query_row("SELECT MAX(version) FROM _migrations", [], |row| {
            row.get::<_, Option<i64>>(0)
        })?
    } else {
        None
    };
    let stored_migrations = if has_migrations {
        let mut statement = connection
            .prepare("SELECT version, name, checksum FROM _migrations ORDER BY version")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let expected_migrations: Vec<_> = migrate::embedded()
        .into_iter()
        .map(|migration| (migration.version, migration.name, migration.checksum))
        .collect();
    let integrity_ok = integrity_results.as_slice() == ["ok"];
    let schema_compatible =
        schema_version == Some(expected_schema_version) && stored_migrations == expected_migrations;

    Ok(BackupVerification {
        path: canonical_path.display().to_string(),
        integrity,
        integrity_ok,
        schema_version,
        schema_compatible,
        valid: integrity_ok && schema_compatible,
    })
}

/// Verify and restore a backup, keeping a safety snapshot of the current database.
///
/// # Errors
///
/// Returns an error if verification fails or the database cannot be safely replaced.
pub fn restore_backup(db: &Db, backup_dir: &Path, path: &Path) -> Result<RestoreResult> {
    let schema_version = migrate::embedded()
        .last()
        .map_or(0, |migration| migration.version);
    let verification = verify_backup(backup_dir, path, schema_version)?;
    if !verification.valid {
        return Err(StorageError::Migration(format!(
            "backup is not restorable: integrity={}, schema={:?}, expected={schema_version}",
            verification.integrity, verification.schema_version
        )));
    }

    let safety_snapshot = backup_dir.join(format!("pre-restore-{}.db", conn::now_ms()));
    db.replace_from(Path::new(&verification.path), &safety_snapshot)?;
    Ok(RestoreResult {
        safety_snapshot: safety_snapshot.display().to_string(),
        schema_version,
    })
}

/// Read a deterministic snapshot of all regular application tables.
///
/// FTS virtual tables and their internal shadow tables are intentionally omitted; their
/// external-content indexes are rebuilt after import from the exported search projections.
pub fn export_snapshot(db: &Db) -> Result<LibraryExport> {
    let tables = db.read(read_tables)?;
    Ok(LibraryExport {
        format_version: EXPORT_FORMAT_VERSION,
        schema_version: migrate::embedded()
            .last()
            .map_or(0, |migration| migration.version),
        exported_at_ms: conn::now_ms(),
        tables,
    })
}

/// Serialize a complete library export as pretty JSON.
pub fn export_json(db: &Db) -> Result<String> {
    serde_json::to_string_pretty(&export_snapshot(db)?).map_err(StorageError::from)
}

/// Write one CSV file per database table into a new timestamped directory under `parent`.
///
/// # Errors
///
/// Returns an I/O or database error when the destination cannot be created or written.
pub fn export_csv(db: &Db, parent: &Path) -> Result<PathBuf> {
    fs::create_dir_all(parent)?;
    let destination = parent.join(format!("maktabati-csv-{}", conn::now_ms()));
    fs::create_dir(&destination)?;

    let snapshot = export_snapshot(db)?;
    let write_result = (|| -> Result<()> {
        for (table_name, table) in &snapshot.tables {
            if !table_name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
            {
                return Err(StorageError::Migration(format!(
                    "unsafe database table name for CSV export: {table_name}"
                )));
            }
            let path = destination.join(format!("{table_name}.csv"));
            let file = OpenOptions::new().write(true).create_new(true).open(path)?;
            write_table_csv(file, table)?;
        }
        let manifest = destination.join("manifest.csv");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(manifest)?;
        writeln!(file, "format_version,schema_version,exported_at_ms")?;
        writeln!(
            file,
            "{},{},{}",
            snapshot.format_version, snapshot.schema_version, snapshot.exported_at_ms
        )?;
        file.flush()?;
        Ok(())
    })();

    if let Err(error) = write_result {
        if let Err(cleanup_error) = fs::remove_dir_all(&destination) {
            tracing::error!(
                path = %destination.display(),
                %cleanup_error,
                "failed to clean incomplete CSV export"
            );
        }
        return Err(error);
    }
    Ok(destination)
}

/// Import a portable JSON export into the current database, replacing its contents atomically.
///
/// This deliberately does not migrate arbitrary old exports: both format and schema must match
/// the running application exactly.
///
/// # Errors
///
/// Returns an error for malformed exports, incompatible schemas, constraint violations, or
/// database failures. The transaction guarantees the current library remains unchanged.
pub fn import_json(db: &Db, contents: &str) -> Result<()> {
    let export: LibraryExport = serde_json::from_str(contents)?;
    let expected_schema_version = migrate::embedded()
        .last()
        .map_or(0, |migration| migration.version);
    if export.format_version != EXPORT_FORMAT_VERSION
        || export.schema_version != expected_schema_version
    {
        return Err(StorageError::Migration(format!(
            "unsupported export format/schema: format={}, schema={}, expected format={}, schema={expected_schema_version}",
            export.format_version, export.schema_version, EXPORT_FORMAT_VERSION
        )));
    }

    let expected_schema = db.read(application_schema)?;
    let expected_tables: Vec<_> = expected_schema.keys().cloned().collect();
    let supplied_tables: Vec<_> = export.tables.keys().cloned().collect();
    if expected_tables != supplied_tables {
        return Err(StorageError::Migration(
            "JSON export tables do not match this database schema".to_owned(),
        ));
    }
    for (name, table) in &export.tables {
        if expected_schema.get(name) != Some(&table.columns)
            || table.rows.iter().any(|row| {
                row.len() != table.columns.len()
                    || table.columns.iter().any(|column| !row.contains_key(column))
            })
        {
            return Err(StorageError::Migration(format!(
                "invalid column shape in JSON export table `{name}`"
            )));
        }
    }

    db.transact(|connection| {
        connection.pragma_update(None, "defer_foreign_keys", "ON")?;
        for table_name in &expected_tables {
            connection.execute(&format!("DELETE FROM {}", quote_identifier(table_name)), [])?;
        }
        for (table_name, table) in &export.tables {
            let quoted_table = quote_identifier(table_name);
            let columns = table
                .columns
                .iter()
                .map(|column| quote_identifier(column))
                .collect::<Vec<_>>()
                .join(", ");
            let placeholders = std::iter::repeat("?")
                .take(table.columns.len())
                .collect::<Vec<_>>()
                .join(", ");
            let statement =
                format!("INSERT INTO {quoted_table} ({columns}) VALUES ({placeholders})");
            let mut statement = connection.prepare(&statement)?;
            for row in &table.rows {
                let values = table
                    .columns
                    .iter()
                    .map(|column| {
                        row.get(column)
                            .ok_or_else(|| {
                                StorageError::Migration(format!(
                                    "missing column `{column}` in JSON export table `{table_name}`"
                                ))
                            })
                            .and_then(json_to_sql_value)
                    })
                    .collect::<Result<Vec<_>>>()?;
                statement.execute(rusqlite::params_from_iter(values))?;
            }
        }
        for fts_table in [
            "fts_book",
            "fts_book_norm",
            "fts_note",
            "fts_quote",
            "fts_author",
        ] {
            connection.execute(
                &format!(
                    "INSERT INTO {} ({}) VALUES ('rebuild')",
                    quote_identifier(fts_table),
                    quote_identifier(fts_table)
                ),
                [],
            )?;
        }
        connection.execute(
            "DELETE FROM sqlite_sequence WHERE name IN ('search_document', 'search_note_doc')",
            [],
        )?;

        let violation: Option<(String, Option<i64>, String, i64)> = connection
            .query_row("PRAGMA foreign_key_check", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .optional()?;
        if let Some((table, _, parent, _)) = violation {
            return Err(StorageError::Migration(format!(
                "JSON export violates a foreign key in `{table}` referencing `{parent}`"
            )));
        }
        Ok(())
    })
}

fn read_tables(connection: &Connection) -> Result<BTreeMap<String, TableExport>> {
    let mut tables = BTreeMap::new();
    for (table_name, columns) in application_schema(connection)? {
        let quoted_table = quote_identifier(&table_name);
        let mut statement = connection.prepare(&format!("SELECT * FROM {quoted_table}"))?;
        let mut rows = Vec::new();
        let mut query = statement.query([])?;
        while let Some(row) = query.next()? {
            let mut values = BTreeMap::new();
            for (index, column) in columns.iter().enumerate() {
                values.insert(column.clone(), sql_value_to_json(row.get_ref(index)?)?);
            }
            rows.push(values);
        }
        rows.sort_by(|left, right| {
            columns
                .iter()
                .fold(std::cmp::Ordering::Equal, |ordering, column| {
                    if ordering != std::cmp::Ordering::Equal {
                        ordering
                    } else {
                        left.get(column)
                            .map_or_else(String::new, serde_json::Value::to_string)
                            .cmp(
                                &right
                                    .get(column)
                                    .map_or_else(String::new, serde_json::Value::to_string),
                            )
                    }
                })
        });
        tables.insert(table_name, TableExport { columns, rows });
    }
    Ok(tables)
}

fn application_schema(connection: &Connection) -> Result<BTreeMap<String, Vec<String>>> {
    let table_names = {
        let mut statement = connection.prepare(
            "SELECT name FROM sqlite_master
         WHERE type = 'table'
           AND name NOT LIKE 'sqlite_%'
           AND name NOT LIKE 'fts_%'
         ORDER BY name",
        )?;
        let names = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        names
    };
    let mut schema = BTreeMap::new();
    for table_name in table_names {
        let quoted_table = quote_identifier(&table_name);
        let mut statement = connection.prepare(&format!("PRAGMA table_info({quoted_table})"))?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        schema.insert(table_name, columns);
    }
    Ok(schema)
}

fn sql_value_to_json(value: ValueRef<'_>) -> Result<serde_json::Value> {
    match value {
        ValueRef::Null => Ok(serde_json::Value::Null),
        ValueRef::Integer(number) => Ok(number.into()),
        ValueRef::Real(number) => serde_json::Number::from_f64(number)
            .map(serde_json::Value::Number)
            .ok_or_else(|| StorageError::Migration("database contains a non-finite number".into())),
        ValueRef::Text(bytes) => String::from_utf8(bytes.to_vec())
            .map(serde_json::Value::String)
            .map_err(|error| {
                StorageError::Migration(format!("database contains invalid UTF-8 text: {error}"))
            }),
        ValueRef::Blob(bytes) => Ok(serde_json::json!({ "$blob_hex": hex::encode(bytes) })),
    }
}

fn json_to_sql_value(value: &serde_json::Value) -> Result<Value> {
    match value {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(value) => Ok(Value::Integer(i64::from(*value))),
        serde_json::Value::Number(number) => number
            .as_i64()
            .map(Value::Integer)
            .or_else(|| number.as_f64().map(Value::Real))
            .ok_or_else(|| StorageError::Migration("invalid number in JSON export".to_owned())),
        serde_json::Value::String(value) => Ok(Value::Text(value.clone())),
        serde_json::Value::Array(_) => Err(StorageError::Migration(
            "unexpected array in JSON export".to_owned(),
        )),
        serde_json::Value::Object(object) => {
            let encoded = object.get("$blob_hex").and_then(serde_json::Value::as_str);
            encoded
                .ok_or_else(|| {
                    StorageError::Migration("unexpected object in JSON export".to_owned())
                })
                .and_then(|encoded| {
                    hex::decode(encoded)
                        .map(Value::Blob)
                        .map_err(|error| StorageError::Migration(error.to_string()))
                })
        }
    }
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn write_table_csv(file: File, table: &TableExport) -> Result<()> {
    let mut writer = BufWriter::new(file);
    write_csv_record(&mut writer, table.columns.iter().map(String::as_str))?;
    for row in &table.rows {
        let cells = table.columns.iter().map(|column| {
            row.get(column)
                .map(csv_cell)
                .unwrap_or_else(|| "\\N".to_owned())
        });
        write_csv_record(&mut writer, cells)?;
    }
    writer.flush()?;
    Ok(())
}

fn csv_cell(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "\\N".to_owned(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Object(object) => object
            .get("$blob_hex")
            .and_then(serde_json::Value::as_str)
            .map_or_else(|| value.to_string(), |hex| format!("hex:{hex}")),
        other => other.to_string(),
    }
}

fn write_csv_record(
    writer: &mut impl Write,
    cells: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<()> {
    let mut first = true;
    for cell in cells {
        if !first {
            writer.write_all(b",")?;
        }
        first = false;
        let cell = cell.as_ref();
        writer.write_all(b"\"")?;
        writer.write_all(cell.replace('"', "\"\"").as_bytes())?;
        writer.write_all(b"\"")?;
    }
    writer.write_all(b"\r\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use maktabati_storage::{migrate, seed};

    fn migrated_db(path: &Path) -> Db {
        let db = Db::open(path).unwrap();
        migrate::apply(&db, || Ok(())).unwrap();
        db.write(seed::apply).unwrap();
        db
    }

    #[test]
    fn listing_only_includes_database_files() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("manual.db"), b"data").unwrap();
        fs::write(directory.path().join("notes.json"), b"data").unwrap();

        let backups = list_backups(directory.path()).unwrap();

        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].name, "manual.db");
    }

    #[test]
    fn verifies_integrity_and_schema_version() {
        let directory = tempfile::tempdir().unwrap();
        let db_path = directory.path().join("source.db");
        let db = migrated_db(&db_path);
        let backup_path = directory.path().join("backups");
        fs::create_dir(&backup_path).unwrap();
        let backup_file = backup_path.join("manual.db");
        db.backup_to(&backup_file).unwrap();
        let expected = migrate::embedded().last().unwrap().version;

        let report = verify_backup(&backup_path, &backup_file, expected).unwrap();

        assert!(report.valid);
        assert_eq!(report.integrity, "ok");
        assert_eq!(report.schema_version, Some(expected));
    }

    #[test]
    fn csv_export_writes_a_file_for_each_application_table() {
        let directory = tempfile::tempdir().unwrap();
        let db = migrated_db(&directory.path().join("library.db"));

        let destination = export_csv(&db, directory.path()).unwrap();

        assert!(destination.join("books.csv").exists());
        assert!(destination.join("authors.csv").exists());
        assert!(destination.join("quotes.csv").exists());
        assert!(destination.join("notes_page.csv").exists());
        assert!(destination.join("reading_sessions.csv").exists());
        assert!(destination.join("manifest.csv").exists());
    }

    #[test]
    fn restore_keeps_a_safety_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let db = migrated_db(&directory.path().join("library.db"));
        db.write(|connection| {
            connection.execute(
                "INSERT INTO app_settings (key, value, updated_at) VALUES ('restore-test', '\"before\"', 1)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let backup_dir = directory.path().join("backups");
        fs::create_dir(&backup_dir).unwrap();
        let backup_path = backup_dir.join("manual.db");
        db.backup_to(&backup_path).unwrap();
        db.write(|connection| {
            connection.execute(
                "UPDATE app_settings SET value = '\"after\"' WHERE key = 'restore-test'",
                [],
            )?;
            Ok(())
        })
        .unwrap();

        let result = restore_backup(&db, &backup_dir, &backup_path).unwrap();

        assert!(Path::new(&result.safety_snapshot).exists());
        db.read(|connection| {
            let value: String = connection.query_row(
                "SELECT value FROM app_settings WHERE key = 'restore-test'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(serde_json::from_str::<String>(&value).unwrap(), "before");
            Ok(())
        })
        .unwrap();
        let safety = Connection::open(&result.safety_snapshot).unwrap();
        let safety_value: String = safety
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'restore-test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<String>(&safety_value).unwrap(),
            "after"
        );
        assert!(
            verify_backup(
                &backup_dir,
                &PathBuf::from(&result.safety_snapshot),
                result.schema_version
            )
            .unwrap()
            .valid
        );
    }

    #[test]
    fn corrupted_backup_fails_verification_and_restore() {
        let directory = tempfile::tempdir().unwrap();
        let db = migrated_db(&directory.path().join("library.db"));
        let backup_dir = directory.path().join("backups");
        fs::create_dir(&backup_dir).unwrap();
        let backup_path = backup_dir.join("manual.db");
        db.backup_to(&backup_path).unwrap();
        let mut bytes = fs::read(&backup_path).unwrap();
        bytes[0] ^= 0xff;
        fs::write(&backup_path, bytes).unwrap();
        let expected = migrate::embedded().last().unwrap().version;

        assert!(!matches!(
            verify_backup(&backup_dir, &backup_path, expected),
            Ok(report) if report.valid
        ));
        assert!(restore_backup(&db, &backup_dir, &backup_path).is_err());
        assert_eq!(
            migrate::current_version(&db).unwrap(),
            expected,
            "verification failure must leave the active database unchanged"
        );
    }

    #[test]
    fn json_export_import_round_trips_all_application_tables() {
        let directory = tempfile::tempdir().unwrap();
        let source = migrated_db(&directory.path().join("source.db"));
        source
            .write(|connection| {
                connection.execute(
                    "INSERT INTO authors (id, name, name_norm, created_at, updated_at)
                     VALUES ('author-1', 'Author', 'author', 1, 1)",
                    [],
                )?;
                connection.execute(
                    "INSERT INTO books (id, title, title_norm, language_code, created_at, updated_at)
                     VALUES ('book-1', 'Book', 'book', 'en', 1, 1)",
                    [],
                )?;
                connection.execute(
                    "INSERT INTO book_authors (book_id, author_id) VALUES ('book-1', 'author-1')",
                    [],
                )?;
                connection.execute(
                    "INSERT INTO quotes (id, book_id, text, text_norm, created_at, updated_at)
                     VALUES ('quote-1', 'book-1', 'A quote', 'a quote', 1, 1)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let original = export_snapshot(&source).unwrap();
        let serialized = export_json(&source).unwrap();

        let destination = migrated_db(&directory.path().join("destination.db"));
        import_json(&destination, &serialized).unwrap();

        let restored = export_snapshot(&destination).unwrap();
        assert_eq!(restored.schema_version, original.schema_version);
        assert_eq!(restored.tables, original.tables);
    }
}
