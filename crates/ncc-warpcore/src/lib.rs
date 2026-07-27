//! Durable, local-first storage and replication outbox for an endpoint ship.
//!
//! A local save and its outbound command are committed in one SQLite
//! transaction. Network delivery is deliberately outside that transaction.

use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug)]
pub enum WarpCoreError {
    Database(rusqlite::Error),
    Serialization(serde_json::Error),
    Poisoned,
    InvalidIntent(&'static str),
    VersionConflict { expected: u64, actual: u64 },
    UnknownCommand(String),
}

impl std::fmt::Display for WarpCoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::Serialization(error) => write!(formatter, "serialization error: {error}"),
            Self::Poisoned => write!(formatter, "Warp Core database lock is poisoned"),
            Self::InvalidIntent(reason) => write!(formatter, "invalid save intent: {reason}"),
            Self::VersionConflict { expected, actual } => {
                write!(
                    formatter,
                    "local version conflict: expected {expected}, found {actual}"
                )
            }
            Self::UnknownCommand(id) => write!(formatter, "unknown outbound command: {id}"),
        }
    }
}

impl std::error::Error for WarpCoreError {}

impl From<rusqlite::Error> for WarpCoreError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value)
    }
}

impl From<serde_json::Error> for WarpCoreError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveOperation {
    Upsert,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaveIntent {
    pub collection: String,
    pub entity_id: String,
    pub operation: SaveOperation,
    pub document: Option<Value>,
    /// Optimistic local concurrency check. Omit for last-local-writer-wins.
    pub expected_local_version: Option<u64>,
}

impl SaveIntent {
    pub fn upsert(
        collection: impl Into<String>,
        entity_id: impl Into<String>,
        document: Value,
    ) -> Self {
        Self {
            collection: collection.into(),
            entity_id: entity_id.into(),
            operation: SaveOperation::Upsert,
            document: Some(document),
            expected_local_version: None,
        }
    }
}

/// The stable command envelope transmitted to base.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerSaveCommand {
    pub command_id: Uuid,
    pub ship_id: String,
    pub event_sequence: u64,
    pub collection: String,
    pub entity_id: String,
    pub operation: SaveOperation,
    pub document: Option<Value>,
    pub local_version: u64,
    pub occurred_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalDocument {
    pub collection: String,
    pub entity_id: String,
    pub document: Option<Value>,
    pub local_version: u64,
    pub dirty: bool,
    pub deleted: bool,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboxStatus {
    Queued,
    Transmitting,
    Retry,
    Acknowledged,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutboxEntry {
    pub command: ServerSaveCommand,
    pub status: OutboxStatus,
    pub attempts: u32,
    pub last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarpCoreStatus {
    pub queued: u64,
    pub transmitting: u64,
    pub retry: u64,
    pub acknowledged: u64,
    pub rejected: u64,
    pub dirty_documents: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseResponse {
    Acknowledged { base_revision: Option<String> },
    Rejected { reason: String, permanent: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportError {
    pub message: String,
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TransportError {}

/// Base adapters implement only transport and authentication. Queue ordering,
/// retries, local durability, and acknowledgement semantics remain Warp Core
/// responsibilities.
#[allow(async_fn_in_trait)]
pub trait BaseTransport: Send + Sync {
    async fn transmit(&self, command: &ServerSaveCommand) -> Result<BaseResponse, TransportError>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayReport {
    pub attempted: u32,
    pub acknowledged: u32,
    pub rejected: u32,
    pub interrupted: bool,
}

/// Thread-safe owner of the endpoint's durable operating state.
pub struct WarpCore {
    ship_id: String,
    connection: Mutex<Connection>,
    relay_gate: tokio::sync::Mutex<()>,
}

impl WarpCore {
    /// Opens an existing ship database or commissions a new endpoint identity.
    /// The generated ship ID is stored inside the same durable database.
    pub fn open_or_create(path: impl AsRef<Path>) -> Result<Self, WarpCoreError> {
        let connection = Connection::open(path)?;
        configure(&connection)?;
        migrate(&connection)?;
        let ship_id = connection
            .query_row(
                "SELECT value FROM metadata WHERE key = 'ship_id'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        connection.execute(
            "INSERT INTO metadata(key, value) VALUES ('ship_id', ?1)
             ON CONFLICT(key) DO NOTHING",
            [&ship_id],
        )?;

        let core = Self {
            ship_id,
            connection: Mutex::new(connection),
            relay_gate: tokio::sync::Mutex::new(()),
        };
        core.recover_interrupted_transmissions()?;
        Ok(core)
    }

    pub fn open(path: impl AsRef<Path>, ship_id: impl Into<String>) -> Result<Self, WarpCoreError> {
        let ship_id = ship_id.into();
        if ship_id.trim().is_empty() {
            return Err(WarpCoreError::InvalidIntent("ship id must not be empty"));
        }

        let connection = Connection::open(path)?;
        configure(&connection)?;
        migrate(&connection)?;

        let core = Self {
            ship_id,
            connection: Mutex::new(connection),
            relay_gate: tokio::sync::Mutex::new(()),
        };
        core.recover_interrupted_transmissions()?;
        Ok(core)
    }

    pub fn open_in_memory(ship_id: impl Into<String>) -> Result<Self, WarpCoreError> {
        let ship_id = ship_id.into();
        if ship_id.trim().is_empty() {
            return Err(WarpCoreError::InvalidIntent("ship id must not be empty"));
        }

        let connection = Connection::open_in_memory()?;
        configure(&connection)?;
        migrate(&connection)?;
        Ok(Self {
            ship_id,
            connection: Mutex::new(connection),
            relay_gate: tokio::sync::Mutex::new(()),
        })
    }

    pub fn ship_id(&self) -> &str {
        &self.ship_id
    }

    /// Atomically stores local truth, appends the ship log, and creates the
    /// idempotent command that will eventually be sent to base.
    pub fn save(&self, intent: SaveIntent) -> Result<ServerSaveCommand, WarpCoreError> {
        validate_intent(&intent)?;
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let command = save_transaction(&transaction, &self.ship_id, intent)?;
        transaction.commit()?;
        Ok(command)
    }

    pub fn document(
        &self,
        collection: &str,
        entity_id: &str,
    ) -> Result<Option<LocalDocument>, WarpCoreError> {
        let connection = self.lock()?;
        let row = connection
            .query_row(
                "SELECT document_json, local_version, dirty, deleted, updated_at_ms
                 FROM documents WHERE collection = ?1 AND entity_id = ?2",
                params![collection, entity_id],
                |row| {
                    let json: Option<String> = row.get(0)?;
                    Ok((
                        json,
                        row.get::<_, i64>(1)?,
                        row.get::<_, bool>(2)?,
                        row.get::<_, bool>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()?;

        row.map(|(json, version, dirty, deleted, updated_at_ms)| {
            Ok(LocalDocument {
                collection: collection.into(),
                entity_id: entity_id.into(),
                document: json.map(|value| serde_json::from_str(&value)).transpose()?,
                local_version: version as u64,
                dirty,
                deleted,
                updated_at_ms,
            })
        })
        .transpose()
    }

    /// Returns commands in ship-log order. A delivery worker should mark each
    /// command transmitting immediately before making the network request.
    pub fn pending_commands(&self, limit: u32) -> Result<Vec<OutboxEntry>, WarpCoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT command_json, status, attempts, last_error
             FROM outbox
             WHERE status IN ('queued', 'retry')
             ORDER BY event_sequence ASC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], decode_outbox_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn mark_transmitting(&self, command_id: Uuid) -> Result<(), WarpCoreError> {
        self.update_command_status(
            command_id,
            "transmitting",
            "attempts = attempts + 1, last_error = NULL",
            None,
        )
    }

    /// Records base acknowledgement. The local document becomes clean only if
    /// no newer local version has been written since this command was created.
    pub fn acknowledge(
        &self,
        command_id: Uuid,
        base_revision: Option<&str>,
    ) -> Result<(), WarpCoreError> {
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let command = command_by_id(&transaction, command_id)?;
        let now = now_ms();

        let changed = transaction.execute(
            "UPDATE outbox
             SET status = 'acknowledged', acknowledged_at_ms = ?2,
                 base_revision = ?3, last_error = NULL, updated_at_ms = ?2
             WHERE command_id = ?1",
            params![command_id.to_string(), now, base_revision],
        )?;
        if changed == 0 {
            return Err(WarpCoreError::UnknownCommand(command_id.to_string()));
        }

        transaction.execute(
            "UPDATE documents SET dirty = 0
             WHERE collection = ?1 AND entity_id = ?2 AND local_version = ?3",
            params![
                command.collection,
                command.entity_id,
                command.local_version as i64
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// A transient failure returns to the queue. A permanent rejection remains
    /// visible for human remediation but never blocks later commands.
    pub fn reject(
        &self,
        command_id: Uuid,
        error: impl Into<String>,
        permanent: bool,
    ) -> Result<(), WarpCoreError> {
        let status = if permanent { "rejected" } else { "retry" };
        self.update_command_status(command_id, status, "last_error = ?3", Some(error.into()))
    }

    pub fn status(&self) -> Result<WarpCoreStatus, WarpCoreError> {
        let connection = self.lock()?;
        let mut status = WarpCoreStatus::default();
        let mut statement =
            connection.prepare("SELECT status, COUNT(*) FROM outbox GROUP BY status")?;
        let counts = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
        })?;
        for count in counts {
            let (name, value) = count?;
            match name.as_str() {
                "queued" => status.queued = value,
                "transmitting" => status.transmitting = value,
                "retry" => status.retry = value,
                "acknowledged" => status.acknowledged = value,
                "rejected" => status.rejected = value,
                _ => {}
            }
        }
        status.dirty_documents = connection.query_row(
            "SELECT COUNT(*) FROM documents WHERE dirty = 1",
            [],
            |row| row.get::<_, i64>(0),
        )? as u64;
        Ok(status)
    }

    /// Relays a bounded batch in ship-log order. Only one relay may run per
    /// ship. A transport failure requeues the current command and stops the
    /// batch, leaving every later command untouched for a future contact.
    pub async fn relay_once<T: BaseTransport>(
        &self,
        transport: &T,
        batch_size: u32,
    ) -> Result<RelayReport, WarpCoreError> {
        let _relay_guard = self.relay_gate.lock().await;
        let pending = self.pending_commands(batch_size)?;
        let mut report = RelayReport::default();

        for entry in pending {
            let command_id = entry.command.command_id;
            self.mark_transmitting(command_id)?;
            report.attempted += 1;

            match transport.transmit(&entry.command).await {
                Ok(BaseResponse::Acknowledged { base_revision }) => {
                    self.acknowledge(command_id, base_revision.as_deref())?;
                    report.acknowledged += 1;
                }
                Ok(BaseResponse::Rejected { reason, permanent }) => {
                    self.reject(command_id, reason, permanent)?;
                    report.rejected += 1;
                    if !permanent {
                        report.interrupted = true;
                        break;
                    }
                }
                Err(error) => {
                    self.reject(command_id, error.message, false)?;
                    report.interrupted = true;
                    break;
                }
            }
        }

        Ok(report)
    }

    fn recover_interrupted_transmissions(&self) -> Result<(), WarpCoreError> {
        let connection = self.lock()?;
        connection.execute(
            "UPDATE outbox SET status = 'queued', updated_at_ms = ?1
             WHERE status = 'transmitting'",
            [now_ms()],
        )?;
        Ok(())
    }

    fn update_command_status(
        &self,
        command_id: Uuid,
        status: &str,
        assignments: &str,
        error: Option<String>,
    ) -> Result<(), WarpCoreError> {
        let connection = self.lock()?;
        let sql = format!(
            "UPDATE outbox SET status = ?2, {assignments}, updated_at_ms = ?4
             WHERE command_id = ?1"
        );
        let changed = connection.execute(
            &sql,
            params![command_id.to_string(), status, error, now_ms()],
        )?;
        if changed == 0 {
            return Err(WarpCoreError::UnknownCommand(command_id.to_string()));
        }
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, WarpCoreError> {
        self.connection.lock().map_err(|_| WarpCoreError::Poisoned)
    }
}

fn configure(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "FULL")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

fn migrate(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE IF NOT EXISTS metadata (
             key TEXT PRIMARY KEY NOT NULL,
             value TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS documents (
             collection TEXT NOT NULL,
             entity_id TEXT NOT NULL,
             document_json TEXT,
             local_version INTEGER NOT NULL,
             dirty INTEGER NOT NULL DEFAULT 1,
             deleted INTEGER NOT NULL DEFAULT 0,
             updated_at_ms INTEGER NOT NULL,
             PRIMARY KEY (collection, entity_id)
         );
         CREATE TABLE IF NOT EXISTS ship_events (
             sequence INTEGER PRIMARY KEY AUTOINCREMENT,
             event_id TEXT NOT NULL UNIQUE,
             collection TEXT NOT NULL,
             entity_id TEXT NOT NULL,
             operation TEXT NOT NULL,
             document_json TEXT,
             local_version INTEGER NOT NULL,
             occurred_at_ms INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS outbox (
             command_id TEXT PRIMARY KEY NOT NULL,
             event_sequence INTEGER NOT NULL UNIQUE,
             command_json TEXT NOT NULL,
             status TEXT NOT NULL,
             attempts INTEGER NOT NULL DEFAULT 0,
             last_error TEXT,
             base_revision TEXT,
             created_at_ms INTEGER NOT NULL,
             updated_at_ms INTEGER NOT NULL,
             acknowledged_at_ms INTEGER,
             FOREIGN KEY (event_sequence) REFERENCES ship_events(sequence)
         );
         CREATE INDEX IF NOT EXISTS outbox_delivery
             ON outbox(status, event_sequence);
         INSERT INTO metadata(key, value) VALUES ('schema_version', '1')
             ON CONFLICT(key) DO UPDATE SET value = excluded.value;
         COMMIT;",
    )?;
    debug_assert_eq!(SCHEMA_VERSION, 1);
    Ok(())
}

fn validate_intent(intent: &SaveIntent) -> Result<(), WarpCoreError> {
    if intent.collection.trim().is_empty() {
        return Err(WarpCoreError::InvalidIntent("collection must not be empty"));
    }
    if intent.entity_id.trim().is_empty() {
        return Err(WarpCoreError::InvalidIntent("entity id must not be empty"));
    }
    match (&intent.operation, &intent.document) {
        (SaveOperation::Upsert, None) => {
            Err(WarpCoreError::InvalidIntent("upsert requires a document"))
        }
        (SaveOperation::Delete, Some(_)) => Err(WarpCoreError::InvalidIntent(
            "delete must not contain a document",
        )),
        _ => Ok(()),
    }
}

fn save_transaction(
    transaction: &Transaction<'_>,
    ship_id: &str,
    intent: SaveIntent,
) -> Result<ServerSaveCommand, WarpCoreError> {
    let current_version = transaction
        .query_row(
            "SELECT local_version FROM documents
             WHERE collection = ?1 AND entity_id = ?2",
            params![intent.collection, intent.entity_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0) as u64;

    if let Some(expected) = intent.expected_local_version
        && expected != current_version
    {
        return Err(WarpCoreError::VersionConflict {
            expected,
            actual: current_version,
        });
    }

    let local_version = current_version + 1;
    let occurred_at_ms = now_ms();
    let document_json = intent
        .document
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    let operation = match intent.operation {
        SaveOperation::Upsert => "upsert",
        SaveOperation::Delete => "delete",
    };

    transaction.execute(
        "INSERT INTO documents(
             collection, entity_id, document_json, local_version, dirty, deleted, updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)
         ON CONFLICT(collection, entity_id) DO UPDATE SET
             document_json = excluded.document_json,
             local_version = excluded.local_version,
             dirty = 1,
             deleted = excluded.deleted,
             updated_at_ms = excluded.updated_at_ms",
        params![
            intent.collection,
            intent.entity_id,
            document_json,
            local_version as i64,
            matches!(intent.operation, SaveOperation::Delete),
            occurred_at_ms
        ],
    )?;

    let event_id = Uuid::new_v4();
    transaction.execute(
        "INSERT INTO ship_events(
             event_id, collection, entity_id, operation, document_json,
             local_version, occurred_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            event_id.to_string(),
            intent.collection,
            intent.entity_id,
            operation,
            document_json,
            local_version as i64,
            occurred_at_ms
        ],
    )?;
    let event_sequence = transaction.last_insert_rowid() as u64;
    let command = ServerSaveCommand {
        command_id: Uuid::new_v4(),
        ship_id: ship_id.into(),
        event_sequence,
        collection: intent.collection,
        entity_id: intent.entity_id,
        operation: intent.operation,
        document: intent.document,
        local_version,
        occurred_at_ms,
    };
    let command_json = serde_json::to_string(&command)?;

    transaction.execute(
        "INSERT INTO outbox(
             command_id, event_sequence, command_json, status,
             attempts, created_at_ms, updated_at_ms
         ) VALUES (?1, ?2, ?3, 'queued', 0, ?4, ?4)",
        params![
            command.command_id.to_string(),
            event_sequence as i64,
            command_json,
            occurred_at_ms
        ],
    )?;
    Ok(command)
}

fn command_by_id(
    transaction: &Transaction<'_>,
    command_id: Uuid,
) -> Result<ServerSaveCommand, WarpCoreError> {
    let json = transaction
        .query_row(
            "SELECT command_json FROM outbox WHERE command_id = ?1",
            [command_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| WarpCoreError::UnknownCommand(command_id.to_string()))?;
    Ok(serde_json::from_str(&json)?)
}

fn decode_outbox_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<OutboxEntry> {
    let command_json: String = row.get(0)?;
    let status: String = row.get(1)?;
    let command = serde_json::from_str(&command_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            command_json.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    let status = match status.as_str() {
        "queued" => OutboxStatus::Queued,
        "transmitting" => OutboxStatus::Transmitting,
        "retry" => OutboxStatus::Retry,
        "acknowledged" => OutboxStatus::Acknowledged,
        "rejected" => OutboxStatus::Rejected,
        _ => {
            return Err(rusqlite::Error::InvalidColumnType(
                1,
                "status".into(),
                rusqlite::types::Type::Text,
            ));
        }
    };
    Ok(OutboxEntry {
        command,
        status,
        attempts: row.get(2)?,
        last_error: row.get(3)?,
    })
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn local_save_and_server_command_commit_together() {
        let core = WarpCore::open_in_memory("ship-alpha").unwrap();
        let command = core
            .save(SaveIntent::upsert(
                "missions",
                "mission-1",
                json!({"title": "Audit the system"}),
            ))
            .unwrap();

        let document = core.document("missions", "mission-1").unwrap().unwrap();
        let pending = core.pending_commands(10).unwrap();

        assert_eq!(document.local_version, 1);
        assert!(document.dirty);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].command, command);
    }

    #[test]
    fn interrupted_transmission_recovers_after_restart() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("warp-core.db");
        let command_id;

        {
            let core = WarpCore::open(&path, "ship-alpha").unwrap();
            let command = core
                .save(SaveIntent::upsert(
                    "logs",
                    "entry-1",
                    json!({"text": "hello"}),
                ))
                .unwrap();
            command_id = command.command_id;
            core.mark_transmitting(command_id).unwrap();
            assert_eq!(core.status().unwrap().transmitting, 1);
        }

        let recovered = WarpCore::open(&path, "ship-alpha").unwrap();
        assert_eq!(recovered.status().unwrap().queued, 1);
        assert_eq!(
            recovered.pending_commands(1).unwrap()[0].command.command_id,
            command_id
        );
    }

    #[test]
    fn acknowledging_an_old_command_does_not_clean_a_newer_save() {
        let core = WarpCore::open_in_memory("ship-alpha").unwrap();
        let old = core
            .save(SaveIntent::upsert("crew", "data", json!({"model": "a"})))
            .unwrap();
        core.save(SaveIntent::upsert("crew", "data", json!({"model": "b"})))
            .unwrap();

        core.acknowledge(old.command_id, Some("base-revision-1"))
            .unwrap();

        let document = core.document("crew", "data").unwrap().unwrap();
        assert_eq!(document.local_version, 2);
        assert!(document.dirty);
    }

    #[test]
    fn permanent_rejection_does_not_block_later_commands() {
        let core = WarpCore::open_in_memory("ship-alpha").unwrap();
        let rejected = core
            .save(SaveIntent::upsert("crew", "worf", json!({"model": "a"})))
            .unwrap();
        let later = core
            .save(SaveIntent::upsert("crew", "data", json!({"model": "b"})))
            .unwrap();

        core.reject(rejected.command_id, "base refused command", true)
            .unwrap();

        let pending = core.pending_commands(10).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].command.command_id, later.command_id);
        assert_eq!(core.status().unwrap().rejected, 1);
    }

    struct FailingTransport {
        calls: AtomicUsize,
    }

    impl BaseTransport for FailingTransport {
        async fn transmit(
            &self,
            _command: &ServerSaveCommand,
        ) -> Result<BaseResponse, TransportError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(TransportError {
                message: "subspace unavailable".into(),
            })
        }
    }

    #[tokio::test]
    async fn transport_failure_stops_drain_without_touching_later_commands() {
        let core = WarpCore::open_in_memory("ship-alpha").unwrap();
        core.save(SaveIntent::upsert("logs", "one", json!({"value": 1})))
            .unwrap();
        core.save(SaveIntent::upsert("logs", "two", json!({"value": 2})))
            .unwrap();
        let transport = FailingTransport {
            calls: AtomicUsize::new(0),
        };

        let report = core.relay_once(&transport, 10).await.unwrap();

        assert_eq!(report.attempted, 1);
        assert!(report.interrupted);
        assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
        assert_eq!(core.status().unwrap().retry, 1);
        assert_eq!(core.status().unwrap().queued, 1);
    }
}
