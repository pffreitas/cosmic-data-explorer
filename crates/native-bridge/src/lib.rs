use std::{
    ffi::{c_char, CStr, CString},
    future::Future,
    path::PathBuf,
    ptr,
};

use cosmic_data_engine::{
    AppStorage, CellValue, ConnectionProfile, CredentialStore, DatabaseConnector, DatabaseKind,
    EngineError, KeyringCredentialStore, QueryRequest, QueryResult, SqlxDatabaseConnector,
};
use serde::{Deserialize, Serialize};

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

    async fn storage(&self) -> cosmic_data_engine::Result<AppStorage> {
        let storage = AppStorage::connect(&self.storage_path).await?;
        storage.initialize().await?;
        Ok(storage)
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

fn execute_query(input: ExecuteQueryInput) -> ExecuteQueryEnvelope {
    let result = run_async(execute_query_async(input));

    match result {
        Ok(result) => ExecuteQueryEnvelope::Success(result_to_output(result)),
        Err(message) => ExecuteQueryEnvelope::Failure(ExecuteQueryFailure { ok: false, message }),
    }
}

async fn execute_query_async(input: ExecuteQueryInput) -> cosmic_data_engine::Result<QueryResult> {
    let service = default_connection_service()?;
    let profile = service.resolve_profile(&input.connection_id).await?;
    let password = service.password_for_profile(&profile)?;
    bootstrap_profile(&profile).await?;

    let connector = SqlxDatabaseConnector;
    let session = connector.connect(&profile, password.as_deref()).await?;
    session
        .execute_query(QueryRequest::new(
            input.connection_id,
            input.sql,
            input.max_rows.unwrap_or(100),
        ))
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
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?
        .block_on(future)
        .map_err(|error| error.to_string())
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

    #[test]
    fn active_connections_use_engine_database_labels() {
        let connections = builtin_active_connections();

        assert_eq!(connections[0].kind, "PostgreSQL");
        assert_eq!(connections[1].kind, "MySQL");
        assert_eq!(connections[2].kind, "SQLite");
    }
}
