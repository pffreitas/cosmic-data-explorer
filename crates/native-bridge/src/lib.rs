use std::{
    collections::HashMap,
    ffi::{c_char, CStr, CString},
    future::Future,
    path::PathBuf,
    ptr,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use cosmic_data_engine::{
    AppStorage, CellValue, ConnectionProfile, CredentialStore, DatabaseConnector, DatabaseKind,
    DatabaseSchema, EngineError, KeyringCredentialStore, QueryRequest, QueryResult,
    SqlxDatabaseConnector,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{futures::OwnedNotified, Mutex, Notify};

const SESSION_IDLE_TTL: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActiveConnection {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub detail: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecuteQueryInput {
    connection_id: String,
    sql: String,
    max_rows: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenConnectionInput {
    connection_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoadSchemaInput {
    connection_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewTableInput {
    connection_id: String,
    schema: Option<String>,
    table: String,
    max_rows: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateConnectionInput {
    name: String,
    connection_string: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ExecuteQueryEnvelope {
    Success(ExecuteQuerySuccess),
    Failure(ExecuteQueryFailure),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecuteQuerySuccess {
    ok: bool,
    columns: Vec<QueryColumnOutput>,
    rows: Vec<Vec<String>>,
    rows_affected: u64,
    elapsed_ms: u128,
    truncated: bool,
}

#[derive(Debug, Serialize)]
struct QueryColumnOutput {
    name: String,
    #[serde(rename = "typeName")]
    type_name: String,
    nullable: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ExecuteQueryFailure {
    ok: bool,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum OpenConnectionEnvelope {
    Success(OpenConnectionSuccess),
    Failure(OpenConnectionFailure),
}

#[derive(Debug, Serialize)]
struct OpenConnectionSuccess {
    ok: bool,
}

#[derive(Debug, Serialize)]
struct OpenConnectionFailure {
    ok: bool,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum LoadSchemaEnvelope {
    Success(LoadSchemaSuccess),
    Failure(LoadSchemaFailure),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadSchemaSuccess {
    ok: bool,
    tables: Vec<SchemaTableOutput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SchemaTableOutput {
    schema: Option<String>,
    name: String,
    kind: String,
    column_count: usize,
}

#[derive(Debug, Serialize)]
struct LoadSchemaFailure {
    ok: bool,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum CreateConnectionEnvelope {
    Success(CreateConnectionSuccess),
    Failure(CreateConnectionFailure),
}

#[derive(Debug, Serialize)]
struct CreateConnectionSuccess {
    ok: bool,
    connection: ActiveConnection,
}

#[derive(Debug, Serialize)]
struct CreateConnectionFailure {
    ok: bool,
    message: String,
}

pub struct ConnectionService<C, S> {
    storage_path: PathBuf,
    credentials: S,
    connector: C,
}

impl<C, S> ConnectionService<C, S>
where
    C: DatabaseConnector,
    S: CredentialStore,
{
    pub fn new(storage_path: PathBuf, credentials: S, connector: C) -> Self {
        Self {
            storage_path,
            credentials,
            connector,
        }
    }

    pub async fn active_connections(&self) -> cosmic_data_engine::Result<Vec<ActiveConnection>> {
        let mut connections = builtin_active_connections();
        let storage = self.storage().await?;
        connections.extend(
            storage
                .list_profiles()
                .await?
                .into_iter()
                .map(|profile| active_connection_from_profile(&profile)),
        );
        Ok(connections)
    }

    pub async fn create_connection(
        &self,
        name: &str,
        connection_string: &str,
    ) -> cosmic_data_engine::Result<ActiveConnection> {
        let parsed = ConnectionProfile::new_postgres_connection_string(name, connection_string)?;
        self.connector
            .test_connection(&parsed.profile, parsed.password.as_deref())
            .await?;

        if let Some(password) = &parsed.password {
            self.credentials
                .set_password(&parsed.profile.credential_ref(), password)?;
        }

        let storage = self.storage().await?;
        if let Err(error) = storage.save_profile(&parsed.profile).await {
            let _ = self
                .credentials
                .delete_password(&parsed.profile.credential_ref());
            return Err(error);
        }

        Ok(active_connection_from_profile(&parsed.profile))
    }

    pub async fn resolve_profile(
        &self,
        connection_id: &str,
    ) -> cosmic_data_engine::Result<ConnectionProfile> {
        if connection_id == "scratch" {
            return Ok(ConnectionProfile::new_sqlite(
                "Scratch",
                scratch_database_path(),
            ));
        }

        let storage = self.storage().await?;
        storage
            .list_profiles()
            .await?
            .into_iter()
            .find(|profile| profile.id == connection_id)
            .ok_or_else(|| unresolved_connection(connection_id))
    }

    pub fn password_for_profile(
        &self,
        profile: &ConnectionProfile,
    ) -> cosmic_data_engine::Result<Option<String>> {
        self.credentials.get_password(&profile.credential_ref())
    }

    pub async fn connect_session(
        &self,
        connection_id: &str,
    ) -> cosmic_data_engine::Result<cosmic_data_engine::DatabaseSession> {
        let profile = self.resolve_profile(connection_id).await?;
        let password = self.password_for_profile(&profile)?;
        bootstrap_profile(&profile).await?;
        self.connector.connect(&profile, password.as_deref()).await
    }

    async fn storage(&self) -> cosmic_data_engine::Result<AppStorage> {
        let storage = AppStorage::connect(&self.storage_path).await?;
        storage.initialize().await?;
        Ok(storage)
    }
}

#[derive(Debug)]
struct CachedSession {
    session: cosmic_data_engine::DatabaseSession,
    last_activity: Instant,
}

#[derive(Debug)]
struct ConnectionSessionCache {
    ttl: Duration,
    sessions: HashMap<String, CachedSession>,
    in_flight: HashMap<String, Arc<Notify>>,
}

#[derive(Debug)]
enum SessionCacheAction {
    Ready(cosmic_data_engine::DatabaseSession),
    Connect,
    Wait(OwnedNotified),
}

impl ConnectionSessionCache {
    fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            sessions: HashMap::new(),
            in_flight: HashMap::new(),
        }
    }

    fn begin(&mut self, connection_id: &str, now: Instant) -> SessionCacheAction {
        self.prune(now);
        if let Some(cached) = self.sessions.get_mut(connection_id) {
            cached.last_activity = now;
            return SessionCacheAction::Ready(cached.session.clone());
        }

        if let Some(notify) = self.in_flight.get(connection_id) {
            return SessionCacheAction::Wait(Arc::clone(notify).notified_owned());
        }

        self.in_flight
            .insert(connection_id.to_string(), Arc::new(Notify::new()));
        SessionCacheAction::Connect
    }

    fn insert(
        &mut self,
        connection_id: String,
        session: cosmic_data_engine::DatabaseSession,
        now: Instant,
    ) -> cosmic_data_engine::DatabaseSession {
        self.prune(now);
        self.sessions.insert(
            connection_id,
            CachedSession {
                session: session.clone(),
                last_activity: now,
            },
        );
        session
    }

    fn finish_connecting(&mut self, connection_id: &str) {
        if let Some(notify) = self.in_flight.remove(connection_id) {
            notify.notify_waiters();
        }
    }

    fn prune(&mut self, now: Instant) {
        let ttl = self.ttl;
        self.sessions
            .retain(|_, cached| now.duration_since(cached.last_activity) <= ttl);
    }
}

static SESSION_CACHE: OnceLock<Mutex<ConnectionSessionCache>> = OnceLock::new();
static BRIDGE_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn default_session_cache() -> &'static Mutex<ConnectionSessionCache> {
    SESSION_CACHE.get_or_init(|| Mutex::new(ConnectionSessionCache::new(SESSION_IDLE_TTL)))
}

async fn session_for_connection<C, S>(
    connection_id: &str,
    service: &ConnectionService<C, S>,
    cache: &Mutex<ConnectionSessionCache>,
    now: Instant,
) -> cosmic_data_engine::Result<cosmic_data_engine::DatabaseSession>
where
    C: DatabaseConnector,
    S: CredentialStore,
{
    loop {
        let action = {
            let mut cache = cache.lock().await;
            cache.begin(connection_id, now)
        };

        match action {
            SessionCacheAction::Ready(session) => return Ok(session),
            SessionCacheAction::Wait(notified) => notified.await,
            SessionCacheAction::Connect => {
                let result = service.connect_session(connection_id).await;
                let mut cache = cache.lock().await;
                cache.finish_connecting(connection_id);
                return result.map(|session| cache.insert(connection_id.to_string(), session, now));
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn cosmic_active_connections_json() -> *mut c_char {
    let connections = match default_connection_service() {
        Ok(service) => {
            run_async(service.active_connections()).unwrap_or_else(|_| builtin_active_connections())
        }
        Err(_) => builtin_active_connections(),
    };
    json_to_c_string(&connections)
}

#[no_mangle]
pub extern "C" fn cosmic_create_connection_json(input_json: *const c_char) -> *mut c_char {
    let envelope = match parse_create_connection_input(input_json) {
        Ok(input) => create_connection(input),
        Err(message) => {
            CreateConnectionEnvelope::Failure(CreateConnectionFailure { ok: false, message })
        }
    };

    json_to_c_string(&envelope)
}

#[no_mangle]
pub extern "C" fn cosmic_open_connection_json(input_json: *const c_char) -> *mut c_char {
    let envelope = match parse_open_connection_input(input_json) {
        Ok(input) => open_connection(input),
        Err(message) => {
            OpenConnectionEnvelope::Failure(OpenConnectionFailure { ok: false, message })
        }
    };

    json_to_c_string(&envelope)
}

#[no_mangle]
pub extern "C" fn cosmic_load_schema_json(input_json: *const c_char) -> *mut c_char {
    let envelope = match parse_load_schema_input(input_json) {
        Ok(input) => load_schema(input),
        Err(message) => LoadSchemaEnvelope::Failure(LoadSchemaFailure { ok: false, message }),
    };

    json_to_c_string(&envelope)
}

#[no_mangle]
pub extern "C" fn cosmic_preview_table_json(input_json: *const c_char) -> *mut c_char {
    let envelope = match parse_preview_table_input(input_json) {
        Ok(input) => preview_table(input),
        Err(message) => ExecuteQueryEnvelope::Failure(ExecuteQueryFailure { ok: false, message }),
    };

    json_to_c_string(&envelope)
}

#[no_mangle]
pub extern "C" fn cosmic_execute_query_json(input_json: *const c_char) -> *mut c_char {
    let envelope = match parse_execute_query_input(input_json) {
        Ok(input) => execute_query(input),
        Err(message) => ExecuteQueryEnvelope::Failure(ExecuteQueryFailure { ok: false, message }),
    };

    json_to_c_string(&envelope)
}

/// Releases a string allocated by this bridge.
///
/// # Safety
///
/// `ptr` must be either null or a pointer previously returned by a function in
/// this crate that transfers ownership of a C string to the caller. Passing any
/// other pointer, or passing the same pointer more than once, is undefined
/// behavior.
#[no_mangle]
pub unsafe extern "C" fn cosmic_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = unsafe { CString::from_raw(ptr) };
    }
}

fn builtin_active_connections() -> Vec<ActiveConnection> {
    vec![
        ActiveConnection {
            id: "production".to_string(),
            name: "Production".to_string(),
            kind: DatabaseKind::Postgres.sql_dialect().to_string(),
            detail: "warehouse / paulo".to_string(),
            status: "Active".to_string(),
        },
        ActiveConnection {
            id: "analytics".to_string(),
            name: "Analytics".to_string(),
            kind: DatabaseKind::MySql.sql_dialect().to_string(),
            detail: "events / analyst".to_string(),
            status: "Active".to_string(),
        },
        ActiveConnection {
            id: "scratch".to_string(),
            name: "Scratch".to_string(),
            kind: DatabaseKind::Sqlite.sql_dialect().to_string(),
            detail: "scratch.sqlite".to_string(),
            status: "Local".to_string(),
        },
    ]
}

fn parse_execute_query_input(
    input_json: *const c_char,
) -> std::result::Result<ExecuteQueryInput, String> {
    if input_json.is_null() {
        return Err("Query request JSON is required".to_string());
    }

    let json = unsafe { CStr::from_ptr(input_json) }
        .to_str()
        .map_err(|_| "Query request JSON must be valid UTF-8".to_string())?;

    serde_json::from_str(json).map_err(|error| format!("Invalid query request JSON: {error}"))
}

fn parse_open_connection_input(
    input_json: *const c_char,
) -> std::result::Result<OpenConnectionInput, String> {
    if input_json.is_null() {
        return Err("Open connection request JSON is required".to_string());
    }

    let json = unsafe { CStr::from_ptr(input_json) }
        .to_str()
        .map_err(|_| "Open connection request JSON must be valid UTF-8".to_string())?;

    serde_json::from_str(json)
        .map_err(|error| format!("Invalid open connection request JSON: {error}"))
}

fn parse_load_schema_input(
    input_json: *const c_char,
) -> std::result::Result<LoadSchemaInput, String> {
    if input_json.is_null() {
        return Err("Schema request JSON is required".to_string());
    }

    let json = unsafe { CStr::from_ptr(input_json) }
        .to_str()
        .map_err(|_| "Schema request JSON must be valid UTF-8".to_string())?;

    serde_json::from_str(json).map_err(|error| format!("Invalid schema request JSON: {error}"))
}

fn parse_preview_table_input(
    input_json: *const c_char,
) -> std::result::Result<PreviewTableInput, String> {
    if input_json.is_null() {
        return Err("Preview request JSON is required".to_string());
    }

    let json = unsafe { CStr::from_ptr(input_json) }
        .to_str()
        .map_err(|_| "Preview request JSON must be valid UTF-8".to_string())?;

    serde_json::from_str(json).map_err(|error| format!("Invalid preview request JSON: {error}"))
}

fn parse_create_connection_input(
    input_json: *const c_char,
) -> std::result::Result<CreateConnectionInput, String> {
    if input_json.is_null() {
        return Err("Connection request JSON is required".to_string());
    }

    let json = unsafe { CStr::from_ptr(input_json) }
        .to_str()
        .map_err(|_| "Connection request JSON must be valid UTF-8".to_string())?;

    serde_json::from_str(json).map_err(|error| format!("Invalid connection request JSON: {error}"))
}

fn create_connection(input: CreateConnectionInput) -> CreateConnectionEnvelope {
    let result = default_connection_service()
        .map_err(|error| error.to_string())
        .and_then(|service| {
            run_async(service.create_connection(&input.name, &input.connection_string))
        });

    match result {
        Ok(connection) => CreateConnectionEnvelope::Success(CreateConnectionSuccess {
            ok: true,
            connection,
        }),
        Err(message) => {
            CreateConnectionEnvelope::Failure(CreateConnectionFailure { ok: false, message })
        }
    }
}

fn open_connection(input: OpenConnectionInput) -> OpenConnectionEnvelope {
    let result = run_async(open_connection_async(input));

    match result {
        Ok(()) => OpenConnectionEnvelope::Success(OpenConnectionSuccess { ok: true }),
        Err(message) => {
            OpenConnectionEnvelope::Failure(OpenConnectionFailure { ok: false, message })
        }
    }
}

fn execute_query(input: ExecuteQueryInput) -> ExecuteQueryEnvelope {
    let result = run_async(execute_query_async(input));

    match result {
        Ok(result) => ExecuteQueryEnvelope::Success(result_to_output(result)),
        Err(message) => ExecuteQueryEnvelope::Failure(ExecuteQueryFailure { ok: false, message }),
    }
}

fn load_schema(input: LoadSchemaInput) -> LoadSchemaEnvelope {
    let result = run_async(load_schema_async(input));

    match result {
        Ok(schema) => LoadSchemaEnvelope::Success(schema_to_output(schema)),
        Err(message) => LoadSchemaEnvelope::Failure(LoadSchemaFailure { ok: false, message }),
    }
}

fn preview_table(input: PreviewTableInput) -> ExecuteQueryEnvelope {
    let result = run_async(preview_table_async(input));

    match result {
        Ok(result) => ExecuteQueryEnvelope::Success(result_to_output(result)),
        Err(message) => ExecuteQueryEnvelope::Failure(ExecuteQueryFailure { ok: false, message }),
    }
}

async fn open_connection_async(input: OpenConnectionInput) -> cosmic_data_engine::Result<()> {
    let service = default_connection_service()?;
    let _ = session_for_connection(
        &input.connection_id,
        &service,
        default_session_cache(),
        Instant::now(),
    )
    .await?;
    Ok(())
}

async fn execute_query_async(input: ExecuteQueryInput) -> cosmic_data_engine::Result<QueryResult> {
    let service = default_connection_service()?;
    let session = session_for_connection(
        &input.connection_id,
        &service,
        default_session_cache(),
        Instant::now(),
    )
    .await?;
    session
        .execute_query(QueryRequest::new(
            input.connection_id,
            input.sql,
            input.max_rows.unwrap_or(100),
        ))
        .await
}

async fn load_schema_async(input: LoadSchemaInput) -> cosmic_data_engine::Result<DatabaseSchema> {
    let service = default_connection_service()?;
    let session = session_for_connection(
        &input.connection_id,
        &service,
        default_session_cache(),
        Instant::now(),
    )
    .await?;
    session.load_schema().await
}

async fn preview_table_async(input: PreviewTableInput) -> cosmic_data_engine::Result<QueryResult> {
    let service = default_connection_service()?;
    let session = session_for_connection(
        &input.connection_id,
        &service,
        default_session_cache(),
        Instant::now(),
    )
    .await?;
    session
        .preview_table(
            input.schema.as_deref(),
            &input.table,
            input.max_rows.unwrap_or(50),
        )
        .await
}

async fn bootstrap_profile(profile: &ConnectionProfile) -> cosmic_data_engine::Result<()> {
    if profile.display_name != "Scratch" {
        return Ok(());
    }

    if let Some(parent) = scratch_database_path().parent() {
        std::fs::create_dir_all(parent).map_err(EngineError::Io)?;
    }

    let connector = SqlxDatabaseConnector;
    let session = connector.connect(profile, None).await?;
    session
        .execute_query(QueryRequest::new(
            profile.id.clone(),
            "create table if not exists users (id integer primary key, name text not null)",
            100,
        ))
        .await?;
    session
        .execute_query(QueryRequest::new(
            profile.id.clone(),
            "insert into users (id, name) values (1, 'Ada') on conflict(id) do nothing",
            100,
        ))
        .await?;
    session
        .execute_query(QueryRequest::new(
            profile.id.clone(),
            "insert into users (id, name) values (2, 'Grace') on conflict(id) do nothing",
            100,
        ))
        .await?;
    Ok(())
}

fn default_connection_service(
) -> cosmic_data_engine::Result<ConnectionService<SqlxDatabaseConnector, KeyringCredentialStore>> {
    let storage_path = AppStorage::default_database_path()
        .ok_or_else(|| EngineError::Validation("App storage path is unavailable".to_string()))?;
    Ok(ConnectionService::new(
        storage_path,
        KeyringCredentialStore,
        SqlxDatabaseConnector,
    ))
}

fn run_async<T>(
    future: impl Future<Output = cosmic_data_engine::Result<T>>,
) -> std::result::Result<T, String> {
    bridge_runtime()?
        .block_on(future)
        .map_err(|error| error.to_string())
}

fn bridge_runtime() -> std::result::Result<&'static tokio::runtime::Runtime, String> {
    if let Some(runtime) = BRIDGE_RUNTIME.get() {
        return Ok(runtime);
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    let _ = BRIDGE_RUNTIME.set(runtime);

    Ok(BRIDGE_RUNTIME
        .get()
        .expect("bridge runtime should be initialized"))
}

fn active_connection_from_profile(profile: &ConnectionProfile) -> ActiveConnection {
    ActiveConnection {
        id: profile.id.clone(),
        name: profile.display_name.clone(),
        kind: profile.kind.sql_dialect().to_string(),
        detail: profile.detail(),
        status: "Saved".to_string(),
    }
}

fn unresolved_connection(connection_id: &str) -> EngineError {
    EngineError::Validation(format!(
        "Connection '{connection_id}' is not available for query execution yet."
    ))
}

fn scratch_database_path() -> PathBuf {
    std::env::temp_dir()
        .join("cosmic-data-explorer")
        .join("scratch.sqlite")
}

fn result_to_output(result: QueryResult) -> ExecuteQuerySuccess {
    ExecuteQuerySuccess {
        ok: true,
        columns: result
            .columns
            .into_iter()
            .map(|column| QueryColumnOutput {
                name: column.name,
                type_name: column.type_name,
                nullable: column.nullable,
            })
            .collect(),
        rows: result
            .rows
            .into_iter()
            .map(|row| row.cells.into_iter().map(display_cell).collect())
            .collect(),
        rows_affected: result.rows_affected,
        elapsed_ms: result.elapsed_ms,
        truncated: result.truncated,
    }
}

fn schema_to_output(schema: DatabaseSchema) -> LoadSchemaSuccess {
    LoadSchemaSuccess {
        ok: true,
        tables: schema
            .tables
            .into_iter()
            .map(|table| SchemaTableOutput {
                schema: table.schema,
                name: table.name,
                kind: table.kind,
                column_count: table.columns.len(),
            })
            .collect(),
    }
}

fn display_cell(cell: CellValue) -> String {
    match cell {
        CellValue::Null => "NULL".to_string(),
        CellValue::Text(value) => value,
        CellValue::Integer(value) => value.to_string(),
        CellValue::Float(value) => value.to_string(),
        CellValue::Boolean(value) => value.to_string(),
        CellValue::Bytes(value) => format!("<{} bytes>", value.len()),
        CellValue::Date(value) => value.to_string(),
        CellValue::Time(value) => value.to_string(),
        CellValue::DateTime(value) => value.to_string(),
        CellValue::Timestamp(value) => value.to_rfc3339(),
        CellValue::Json(value) => value.to_string(),
    }
}

fn json_to_c_string<T: Serialize>(value: &T) -> *mut c_char {
    let Ok(json) = serde_json::to_string(value) else {
        return ptr::null_mut();
    };
    let Ok(c_string) = CString::new(json) else {
        return ptr::null_mut();
    };
    c_string.into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex as StdMutex,
        },
        time::{Duration, Instant},
    };

    use async_trait::async_trait;
    use cosmic_data_engine::{CredentialRef, DatabaseSession, InMemoryCredentialStore};

    #[test]
    fn active_connections_use_engine_database_labels() {
        let connections = builtin_active_connections();

        assert_eq!(connections[0].kind, "PostgreSQL");
        assert_eq!(connections[1].kind, "MySQL");
        assert_eq!(connections[2].kind, "SQLite");
    }

    #[test]
    fn run_async_keeps_spawned_runtime_work_alive_after_call_returns() {
        let completed = Arc::new(AtomicUsize::new(0));
        let completed_in_task = Arc::clone(&completed);

        run_async(async move {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(10)).await;
                completed_in_task.store(1, Ordering::SeqCst);
            });
            Ok(())
        })
        .unwrap();

        std::thread::sleep(Duration::from_millis(50));

        assert_eq!(
            completed.load(Ordering::SeqCst),
            1,
            "bridge runtime must outlive individual FFI calls"
        );
    }

    #[tokio::test]
    async fn session_cache_reuses_open_session_within_ttl() {
        let fixture = SessionCacheFixture::new().await;
        let cache = Mutex::new(ConnectionSessionCache::new(Duration::from_secs(3600)));
        let now = Instant::now();

        let first = session_for_connection(&fixture.profile.id, &fixture.service, &cache, now)
            .await
            .unwrap();
        let second = session_for_connection(
            &fixture.profile.id,
            &fixture.service,
            &cache,
            now + Duration::from_secs(30),
        )
        .await
        .unwrap();

        assert_eq!(first.profile_id, second.profile_id);
        assert_eq!(fixture.credentials.get_count(), 1);
    }

    #[tokio::test]
    async fn session_cache_reopens_session_after_ttl() {
        let fixture = SessionCacheFixture::new().await;
        let cache = Mutex::new(ConnectionSessionCache::new(Duration::from_secs(3600)));
        let now = Instant::now();

        let _ = session_for_connection(&fixture.profile.id, &fixture.service, &cache, now)
            .await
            .unwrap();
        let _ = session_for_connection(
            &fixture.profile.id,
            &fixture.service,
            &cache,
            now + Duration::from_secs(3601),
        )
        .await
        .unwrap();

        assert_eq!(fixture.credentials.get_count(), 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn session_cache_shares_concurrent_first_open() {
        let fixture = SessionCacheFixture::new().await;
        let cache = Mutex::new(ConnectionSessionCache::new(Duration::from_secs(3600)));
        let now = Instant::now();

        let (first, second) = tokio::join!(
            session_for_connection(&fixture.profile.id, &fixture.service, &cache, now),
            session_for_connection(&fixture.profile.id, &fixture.service, &cache, now),
        );

        assert_eq!(first.unwrap().profile_id, second.unwrap().profile_id);
        assert_eq!(fixture.credentials.get_count(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn session_cache_does_not_block_other_connections_while_first_open_connects() {
        let fixture = MultiConnectionFixture::new().await;
        let cache = Arc::new(Mutex::new(ConnectionSessionCache::new(
            Duration::from_secs(3600),
        )));
        let service = Arc::new(fixture.service);
        let now = Instant::now();

        let slow_open = {
            let cache = Arc::clone(&cache);
            let service = Arc::clone(&service);
            let connection_id = fixture.slow_profile.id.clone();

            tokio::spawn(async move {
                session_for_connection(&connection_id, service.as_ref(), cache.as_ref(), now).await
            })
        };

        fixture.connector.slow_connect_started.notified().await;

        let fast_open = tokio::time::timeout(
            Duration::from_millis(100),
            session_for_connection(
                &fixture.fast_profile.id,
                service.as_ref(),
                cache.as_ref(),
                now,
            ),
        )
        .await;

        fixture.connector.release_slow_connect.notify_waiters();
        let slow_result = slow_open.await.expect("slow open task should not panic");
        slow_result.expect("slow connection should eventually open");

        assert!(
            fast_open.is_ok(),
            "a stalled first open must not block other connections from opening"
        );
        fast_open
            .expect("fast connection should not time out")
            .expect("fast connection should open");
    }

    struct SessionCacheFixture {
        _tempdir: tempfile::TempDir,
        profile: ConnectionProfile,
        credentials: CountingCredentialStore,
        service: ConnectionService<SqlxDatabaseConnector, CountingCredentialStore>,
    }

    impl SessionCacheFixture {
        async fn new() -> Self {
            let tempdir = tempfile::tempdir().unwrap();
            let storage_path = tempdir.path().join("app.sqlite");
            let profile =
                ConnectionProfile::new_sqlite("Local", tempdir.path().join("sample.sqlite"));
            let storage = AppStorage::connect(&storage_path).await.unwrap();
            storage.initialize().await.unwrap();
            storage.save_profile(&profile).await.unwrap();

            let credentials = CountingCredentialStore::default();
            credentials
                .set_password(&profile.credential_ref(), "unused-sqlite-password")
                .unwrap();
            let service =
                ConnectionService::new(storage_path, credentials.clone(), SqlxDatabaseConnector);

            Self {
                _tempdir: tempdir,
                profile,
                credentials,
                service,
            }
        }
    }

    struct MultiConnectionFixture {
        _tempdir: tempfile::TempDir,
        slow_profile: ConnectionProfile,
        fast_profile: ConnectionProfile,
        connector: DelayedSqliteConnector,
        service: ConnectionService<DelayedSqliteConnector, CountingCredentialStore>,
    }

    impl MultiConnectionFixture {
        async fn new() -> Self {
            let tempdir = tempfile::tempdir().unwrap();
            let storage_path = tempdir.path().join("app.sqlite");
            let slow_profile =
                ConnectionProfile::new_sqlite("Slow Local", tempdir.path().join("slow.sqlite"));
            let fast_profile =
                ConnectionProfile::new_sqlite("Fast Local", tempdir.path().join("fast.sqlite"));
            let storage = AppStorage::connect(&storage_path).await.unwrap();
            storage.initialize().await.unwrap();
            storage.save_profile(&slow_profile).await.unwrap();
            storage.save_profile(&fast_profile).await.unwrap();

            let credentials = CountingCredentialStore::default();
            let connector = DelayedSqliteConnector::new(slow_profile.id.clone());
            let service = ConnectionService::new(storage_path, credentials, connector.clone());

            Self {
                _tempdir: tempdir,
                slow_profile,
                fast_profile,
                connector,
                service,
            }
        }
    }

    #[derive(Clone)]
    struct DelayedSqliteConnector {
        slow_profile_id: String,
        slow_connect_started: Arc<tokio::sync::Notify>,
        release_slow_connect: Arc<tokio::sync::Notify>,
        delegate: SqlxDatabaseConnector,
    }

    impl DelayedSqliteConnector {
        fn new(slow_profile_id: String) -> Self {
            Self {
                slow_profile_id,
                slow_connect_started: Arc::new(tokio::sync::Notify::new()),
                release_slow_connect: Arc::new(tokio::sync::Notify::new()),
                delegate: SqlxDatabaseConnector,
            }
        }
    }

    #[async_trait]
    impl DatabaseConnector for DelayedSqliteConnector {
        async fn test_connection(
            &self,
            profile: &ConnectionProfile,
            password: Option<&str>,
        ) -> cosmic_data_engine::Result<()> {
            self.delegate.test_connection(profile, password).await
        }

        async fn connect(
            &self,
            profile: &ConnectionProfile,
            password: Option<&str>,
        ) -> cosmic_data_engine::Result<DatabaseSession> {
            if profile.id == self.slow_profile_id {
                self.slow_connect_started.notify_waiters();
                self.release_slow_connect.notified().await;
            }

            self.delegate.connect(profile, password).await
        }
    }

    #[derive(Clone, Default)]
    struct CountingCredentialStore {
        passwords: InMemoryCredentialStore,
        get_count: Arc<AtomicUsize>,
        deleted: Arc<StdMutex<HashMap<CredentialRef, usize>>>,
    }

    impl CountingCredentialStore {
        fn get_count(&self) -> usize {
            self.get_count.load(Ordering::SeqCst)
        }
    }

    impl CredentialStore for CountingCredentialStore {
        fn set_password(
            &self,
            credential: &CredentialRef,
            password: &str,
        ) -> cosmic_data_engine::Result<()> {
            self.passwords.set_password(credential, password)
        }

        fn get_password(
            &self,
            credential: &CredentialRef,
        ) -> cosmic_data_engine::Result<Option<String>> {
            self.get_count.fetch_add(1, Ordering::SeqCst);
            self.passwords.get_password(credential)
        }

        fn delete_password(&self, credential: &CredentialRef) -> cosmic_data_engine::Result<()> {
            let mut deleted = self
                .deleted
                .lock()
                .map_err(|error| EngineError::Credential(error.to_string()))?;
            *deleted.entry(credential.clone()).or_default() += 1;
            self.passwords.delete_password(credential)
        }
    }
}
