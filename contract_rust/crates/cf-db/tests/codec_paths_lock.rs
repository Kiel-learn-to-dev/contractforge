use std::path::Path;

use cf_db::{
    DbError, codec,
    lock::WriterLock,
    paths::{DataPaths, resolve_data_root, resolve_stored},
};
use chrono::{NaiveDate, NaiveDateTime};
use rusqlite::{
    Connection,
    types::{Value, ValueRef},
};
use rust_decimal::Decimal;

#[test]
fn decimal_codec_preserves_sqlite_numeric_affinity_and_decimal_text() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute("CREATE TABLE amounts (amount NUMERIC(18,2))", [])
        .unwrap();
    for value in ["82.5", "24000000.00"] {
        connection
            .execute(
                "INSERT INTO amounts VALUES (?1)",
                [codec::write_decimal(Some(
                    value.parse::<Decimal>().unwrap(),
                ))],
            )
            .unwrap();
    }
    let mut statement = connection
        .prepare("SELECT amount, typeof(amount) FROM amounts ORDER BY rowid")
        .unwrap();
    let mut rows = statement.query([]).unwrap();
    let first = rows.next().unwrap().unwrap();
    assert_eq!(
        codec::read_decimal(first.get_ref(0).unwrap()).unwrap(),
        Some(Decimal::new(825, 1))
    );
    assert_eq!(first.get::<_, String>(1).unwrap(), "real");
    let second = rows.next().unwrap().unwrap();
    assert_eq!(
        codec::read_decimal(second.get_ref(0).unwrap()).unwrap(),
        Some(Decimal::from(24_000_000))
    );
    assert_eq!(second.get::<_, String>(1).unwrap(), "integer");
    assert_eq!(
        codec::write_decimal(Some(Decimal::new(1, 1))),
        Value::Text("0.1".into())
    );
}

#[test]
fn date_datetime_bool_and_optional_text_codecs_follow_storage_formats() {
    let date = NaiveDate::from_ymd_opt(2026, 3, 24).unwrap();
    let with_micros =
        NaiveDateTime::parse_from_str("2026-03-24 04:40:37.560192", "%Y-%m-%d %H:%M:%S%.6f")
            .unwrap();
    let without_micros =
        NaiveDateTime::parse_from_str("2026-03-24 04:40:37", "%Y-%m-%d %H:%M:%S").unwrap();
    assert_eq!(
        codec::read_date(ValueRef::Text(b"2026-03-24")).unwrap(),
        Some(date)
    );
    assert_eq!(
        codec::read_datetime(ValueRef::Text(b"2026-03-24 04:40:37.560192")).unwrap(),
        Some(with_micros)
    );
    assert_eq!(
        codec::read_datetime(ValueRef::Text(b"2026-03-24 04:40:37")).unwrap(),
        Some(without_micros)
    );
    assert_eq!(codec::read_bool(ValueRef::Integer(1)).unwrap(), Some(true));
    assert_eq!(codec::read_bool(ValueRef::Text(b"0")).unwrap(), Some(false));
    assert_eq!(codec::read_opt_text(ValueRef::Text(b"  ")).unwrap(), None);
    assert_eq!(codec::opt_text(Some("  ")), Value::Null);
    assert_eq!(
        codec::write_date(Some(date)),
        Value::Text("2026-03-24".into())
    );
    assert_eq!(
        codec::write_datetime(Some(with_micros)),
        Value::Text("2026-03-24 04:40:37.560192".into())
    );
}

#[test]
fn root_resolution_and_stored_paths_keep_the_documented_precedence() {
    let temp = tempfile::tempdir().unwrap();
    let local = temp.path().join("local");
    let exe = temp.path().join("exe");
    let local_root = local.join("ContractForge");
    std::fs::create_dir_all(&local_root).unwrap();
    std::fs::write(local_root.join("contract_manager.db"), []).unwrap();
    assert_eq!(
        resolve_data_root(None, Some(&local), Some(&exe)),
        local_root
    );
    let explicit = temp.path().join("explicit");
    assert_eq!(
        resolve_data_root(Some(explicit.to_str().unwrap()), Some(&local), Some(&exe)),
        explicit
    );
    assert_eq!(
        resolve_stored(Path::new("/data/root"), "uploads/x.pdf"),
        Path::new("/data/root/uploads/x.pdf")
    );
    assert_eq!(
        resolve_stored(Path::new("/data/root"), "D:/private/file.pdf"),
        Path::new("D:/private/file.pdf")
    );
    let paths = DataPaths::new(temp.path().join("fresh"));
    paths.ensure_dirs().unwrap();
    assert!(paths.templates.is_dir());
}

#[test]
fn writer_lock_is_exclusive_and_released_on_drop() {
    let temp = tempfile::tempdir().unwrap();
    let lock = WriterLock::acquire(temp.path()).unwrap();
    assert!(matches!(
        WriterLock::acquire(temp.path()),
        Err(DbError::Locked)
    ));
    drop(lock);
    assert!(WriterLock::acquire(temp.path()).is_ok());
}
