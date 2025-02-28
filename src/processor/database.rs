use rusqlite::{Connection, Result};
use std::fs;
use std::path::PathBuf;
use dirs::home_dir;

fn get_database_path() -> PathBuf {
    let mut path = home_dir().expect("Unable to find home directory");
    path.push(".k21");
    fs::create_dir_all(&path).expect("Unable to create .k21 directory");
    path.push("ocr_data.db");
    path
}

pub fn create_database() -> Result<()> {
    let db_path = get_database_path();
    let conn = Connection::open(db_path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ocr_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            ocr_text TEXT NOT NULL
        )",
        [],
    )?;
    Ok(())
}

pub fn insert_ocr_entry(timestamp: &str, ocr_text: &str) -> Result<()> {
    let db_path = get_database_path();
    let conn = Connection::open(db_path)?;
    conn.execute(
        "INSERT INTO ocr_entries (timestamp, ocr_text) VALUES (?1, ?2)",
        &[timestamp, ocr_text],
    )?;
    Ok(())
}
