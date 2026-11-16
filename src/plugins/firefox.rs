//! Read LeetCode cookies from Firefox's `cookies.sqlite`.
//!
//! Unlike Chrome, Firefox stores cookie values in plain text, so no decryption
//! is needed. Every profile is scanned; the most recently used one wins.
#![allow(clippy::redundant_field_names)] // diesel derive trips this in generated code
use crate::{Error, Result};
use diesel::{prelude::*, sql_query, sql_types, sqlite::SqliteConnection};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(QueryableByName)]
struct CookieRow {
    #[diesel(sql_type = sql_types::Text)]
    name: String,
    #[diesel(sql_type = sql_types::Text)]
    value: String,
}

/// Read cookies matching `domain` from all Firefox profiles.
pub fn cookies(domain: &str) -> Result<HashMap<String, String>> {
    let mut jar = HashMap::new();

    for db in cookie_db_paths() {
        jar.extend(read_profile(&db, domain));
    }

    if jar.is_empty() {
        Err(Error::CookieNotFound)
    } else {
        Ok(jar)
    }
}

/// Read one profile's cookies, via a snapshot copy since Firefox locks the DB.
fn read_profile(db: &Path, domain: &str) -> HashMap<String, String> {
    let tmp = std::env::temp_dir().join(format!(
        "leetcode-cli-ff-{}-{}.sqlite",
        std::process::id(),
        db.parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    if std::fs::copy(db, &tmp).is_err() {
        return HashMap::new();
    }
    let rows = SqliteConnection::establish(&tmp.to_string_lossy())
        .ok()
        .and_then(|mut conn| load_cookies(&mut conn, domain))
        .unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);

    rows.into_iter().map(|r| (r.name, r.value)).collect()
}

/// Modern Firefox/Zen use `host`; old profiles use `host_key`.
fn load_cookies(conn: &mut SqliteConnection, domain: &str) -> Option<Vec<CookieRow>> {
    ["host", "host_key"].into_iter().find_map(|col| {
        sql_query(format!(
            "SELECT name, value FROM moz_cookies WHERE {col} LIKE ?"
        ))
        .bind::<sql_types::Text, _>(format!("%{domain}%"))
        .load::<CookieRow>(conn)
        .ok()
    })
}

/// Every `cookies.sqlite` found in Firefox profile roots, oldest first.
fn cookie_db_paths() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Library/Application Support/Firefox/Profiles"));
        roots.push(home.join("Library/Application Support/zen/Profiles"));
        roots.push(home.join(".mozilla/firefox"));
        roots.push(home.join(".zen"));
    }
    if let Some(data) = dirs::data_dir() {
        roots.push(data.join("Mozilla/Firefox/Profiles"));
    }

    let mut dbs: Vec<PathBuf> = roots
        .into_iter()
        .filter_map(|r| std::fs::read_dir(r).ok())
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path().join("cookies.sqlite"))
        .filter(|p| p.is_file())
        .collect();

    dbs.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    dbs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_moz_cookies() {
        let dir = std::env::temp_dir().join(format!("leetcode-cli-ff-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("cookies.sqlite");
        let mut conn = SqliteConnection::establish(&db.to_string_lossy()).unwrap();
        sql_query("CREATE TABLE moz_cookies (name TEXT, value TEXT, host TEXT)")
            .execute(&mut conn)
            .unwrap();
        for (name, value, host) in [
            ("csrftoken", "csrf123", ".leetcode.com"),
            ("LEETCODE_SESSION", "session123", ".leetcode.com"),
            ("other", "nope", ".example.com"),
        ] {
            sql_query("INSERT INTO moz_cookies VALUES (?, ?, ?)")
                .bind::<sql_types::Text, _>(name)
                .bind::<sql_types::Text, _>(value)
                .bind::<sql_types::Text, _>(host)
                .execute(&mut conn)
                .unwrap();
        }
        drop(conn);

        let jar = read_profile(&db, "leetcode.com");
        assert_eq!(jar.get("csrftoken").map(String::as_str), Some("csrf123"));
        assert_eq!(
            jar.get("LEETCODE_SESSION").map(String::as_str),
            Some("session123")
        );
        assert!(!jar.contains_key("other"));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
