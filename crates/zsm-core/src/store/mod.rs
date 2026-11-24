use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use rusqlite::{Connection, OptionalExtension, params};

use crate::dto::AccountInfo;
use crate::error::{Error, Result};

pub mod actions;
pub mod categories;
pub mod contacts;
pub mod conversations;
pub mod drafts;
pub mod ingest;
pub mod merge;
pub mod messages;
pub mod notifications;
pub mod outgoing;
pub mod people;
pub mod query;
pub mod routines;
pub mod rules;
pub mod search;
pub mod sorting;
pub mod sync;
pub mod tags;
pub mod timeline;
pub mod today;
pub mod watches;

const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("migrations/0001_init.sql")),
    ("0002_state", include_str!("migrations/0002_state.sql")),
    ("0003_participants", include_str!("migrations/0003_participants.sql")),
    ("0004_sorting", include_str!("migrations/0004_sorting.sql")),
    ("0005_automation", include_str!("migrations/0005_automation.sql")),
    ("0006_writing", include_str!("migrations/0006_writing.sql")),
    ("0007_categories", include_str!("migrations/0007_categories.sql")),
    ("0008_bcc", include_str!("migrations/0008_bcc.sql")),
];

pub struct Store {
    conn: Connection,
    account: Option<i64>,
}

fn register_functions(conn: &Connection) -> Result<()> {
    use rusqlite::functions::FunctionFlags;
    conn.create_scalar_function("zfold", 1, FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC, |ctx| {
        let value: Option<String> = ctx.get(0)?;
        Ok(crate::text::fold(&value.unwrap_or_default()))
    })?;
    Ok(())
}

#[derive(Clone)]
pub struct SharedStore(Arc<Mutex<Store>>);

impl SharedStore {
    pub fn new(store: Store) -> Self {
        Self(Arc::new(Mutex::new(store)))
    }

    pub fn lock(&self) -> MutexGuard<'_, Store> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn with<T>(&self, f: impl FnOnce(&mut Store) -> Result<T>) -> Result<T> {
        f(&mut self.lock())
    }
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;
             PRAGMA temp_store = MEMORY;
             PRAGMA cache_size = -65536;",
        )?;
        register_functions(&conn)?;
        migrate(&mut conn)?;
        Ok(Self { conn, account: None })
    }

    pub fn set_account_scope(&mut self, account: Option<i64>) {
        self.account = account;
    }

    pub(crate) fn in_account(&self, alias: &str) -> String {
        match self.account {
            Some(id) => format!(" AND {alias}.account_id = {id}"),
            None => String::new(),
        }
    }

    pub(crate) fn account_scope(&self) -> Option<i64> {
        self.account
    }

    pub fn schema_version(&self) -> Result<u32> {
        Ok(self.conn.query_row("PRAGMA user_version", [], |row| row.get(0))?)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn delete_setting(&self, key: &str) -> Result<()> {
        self.conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
        Ok(())
    }

    pub fn upsert_account(&self, provider: &str, email: &str, display_name: Option<&str>, now: i64) -> Result<i64> {
        if let Some(id) =
            self.conn.query_row("SELECT id FROM accounts WHERE email = ?1", [email], |r| r.get(0)).optional()?
        {
            return Ok(id);
        }
        self.conn.execute(
            "INSERT INTO accounts (provider, email, display_name, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![provider, email, display_name, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn accounts(&self) -> Result<Vec<AccountInfo>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.provider, a.email, a.display_name, s.last_sync_at
             FROM accounts a LEFT JOIN sync_state s ON s.account_id = a.id ORDER BY a.id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(AccountInfo {
                id: r.get(0)?,
                provider: r.get(1)?,
                email: r.get(2)?,
                display_name: r.get(3)?,
                last_sync_at: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn self_addresses(&self, account_id: i64) -> Result<Vec<String>> {
        let mut out: Vec<String> = Vec::new();
        if let Some(email) = self
            .conn
            .query_row("SELECT email FROM accounts WHERE id = ?1", [account_id], |r| r.get::<_, String>(0))
            .optional()?
        {
            out.push(email.to_lowercase());
        }
        if let Some(aliases) = self.setting(&format!("aliases.{account_id}"))? {
            out.extend(aliases.split(',').map(|a| a.trim().to_lowercase()).filter(|a| !a.is_empty()));
        }
        Ok(out)
    }

    pub fn in_transaction<T>(&mut self, f: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T>) -> Result<T> {
        let tx = self.conn.transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let applied: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    for (index, (name, sql)) in MIGRATIONS.iter().enumerate() {
        let version = u32::try_from(index + 1).expect("fewer than 2^32 migrations");
        if version <= applied {
            continue;
        }
        tracing::info!(migration = name, "applying migration");
        let tx = conn.transaction()?;
        tx.execute_batch(sql).map_err(|source| Error::Migration { name, source })?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(store: &Store) {
        let c = store.conn();
        c.execute_batch(
            "INSERT INTO accounts (id, provider, email, created_at) VALUES (1, 'gmail', 'me@example.com', 0);
             INSERT INTO people (id, account_id, display_name, ink, created_at) VALUES (1, 1, 'Maya Okafor', 3, 0);
             INSERT INTO person_addresses (account_id, address, person_id, is_primary) VALUES (1, 'Maya@Example.org', 1, 1);
             INSERT INTO conversations (id, account_id, provider_id, subject, first_message_at, last_message_at, message_count)
               VALUES (1, 1, 'thread-1', 'Q3 launch checklist', 1000, 2000, 2);
             INSERT INTO conversation_people (conversation_id, person_id) VALUES (1, 1);
             INSERT INTO messages (id, account_id, conversation_id, provider_id, from_person_id, from_address, subject, sent_at)
               VALUES (1, 1, 1, 'msg-1', 1, 'maya@example.org', 'Q3 launch checklist', 1000);",
        )
        .unwrap();
    }

    #[test]
    fn migrations_apply_and_record_the_version() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.schema_version().unwrap(), MIGRATIONS.len() as u32);
    }

    #[test]
    fn opening_twice_does_not_reapply_migrations() {
        let dir = std::env::temp_dir().join(format!("zsm-store-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("zsm.db");
        let first = Store::open(&path).unwrap().schema_version().unwrap();
        let second = Store::open(&path).unwrap().schema_version().unwrap();
        assert_eq!(first, second);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn addresses_match_case_insensitively() {
        let store = Store::open_in_memory().unwrap();
        seed(&store);
        let person: i64 = store
            .conn()
            .query_row("SELECT person_id FROM person_addresses WHERE address = 'maya@example.org'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(person, 1);
    }

    #[test]
    fn deleting_a_conversation_cascades_to_messages() {
        let store = Store::open_in_memory().unwrap();
        seed(&store);
        store.conn().execute("DELETE FROM conversations WHERE id = 1", []).unwrap();
        let left: i64 = store.conn().query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0)).unwrap();
        assert_eq!(left, 0);
    }

    #[test]
    fn invalid_values_are_rejected_by_check_constraints() {
        let store = Store::open_in_memory().unwrap();
        seed(&store);
        let bad = store
            .conn()
            .execute("INSERT INTO people (account_id, display_name, ink, created_at) VALUES (1, 'X', 10, 0)", []);
        assert!(bad.is_err(), "ink above 9 must be rejected");
    }

    #[test]
    fn contentless_fts_supports_prefix_search_and_delete() {
        let store = Store::open_in_memory().unwrap();
        let c = store.conn();
        c.execute(
            "INSERT INTO messages_fts (rowid, subject, body, participants) VALUES (7, 'Q3 launch checklist', 'Hi, attaching the checklist', 'Maya Okafor')",
            [],
        )
        .unwrap();
        let hit = |q: &str| -> Vec<i64> {
            let mut stmt = c.prepare("SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?1").unwrap();
            stmt.query_map([q], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
        };
        assert_eq!(hit("chec*"), vec![7], "prefix query");
        assert_eq!(hit("okafor"), vec![7], "participants column");
        assert_eq!(hit("launch checklist"), vec![7], "implicit AND");
        c.execute("DELETE FROM messages_fts WHERE rowid = 7", []).unwrap();
        assert!(hit("checklist").is_empty(), "deleted rows leave the index");
    }

    #[test]
    fn diacritics_are_folded_in_search() {
        let store = Store::open_in_memory().unwrap();
        let c = store.conn();
        c.execute(
            "INSERT INTO people_fts (rowid, name, addresses, organization) VALUES (1, 'Amélie Dubois', '', '')",
            [],
        )
        .unwrap();
        let n: i64 =
            c.query_row("SELECT COUNT(*) FROM people_fts WHERE people_fts MATCH 'amelie'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }
}
