use std::{
    ffi::{c_char, CStr, CString},
    path::PathBuf,
    ptr,
};

use cosmic_data_engine::{
    CellValue, ConnectionProfile, DatabaseConnector, DatabaseKind, EngineError, QueryRequest,
    QueryResult, SqlxDatabaseConnector,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
struct ActiveConnection {
    id: &'static str,
    name: &'static str,
    kind: &'static str,
    detail: &'static str,
    status: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecuteQueryInput {
    connection_id: String,
    sql: String,
    max_rows: Option<u32>,
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

#[no_mangle]
pub extern "C" fn cosmic_active_connections_json() -> *mut c_char {
    let connections = active_connections();
    json_to_c_string(&connections)
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

fn active_connections() -> Vec<ActiveConnection> {
    vec![
        ActiveConnection {
            id: "production",
            name: "Production",
            kind: DatabaseKind::Postgres.sql_dialect(),
            detail: "warehouse / paulo",
            status: "Active",
        },
        ActiveConnection {
            id: "analytics",
            name: "Analytics",
            kind: DatabaseKind::MySql.sql_dialect(),
            detail: "events / analyst",
            status: "Active",
        },
        ActiveConnection {
            id: "scratch",
            name: "Scratch",
            kind: DatabaseKind::Sqlite.sql_dialect(),
            detail: "scratch.sqlite",
            status: "Local",
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

fn execute_query(input: ExecuteQueryInput) -> ExecuteQueryEnvelope {
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())
        .and_then(|runtime| {
            runtime
                .block_on(execute_query_async(input))
                .map_err(|error| error.to_string())
        });

    match result {
        Ok(result) => ExecuteQueryEnvelope::Success(result_to_output(result)),
        Err(message) => ExecuteQueryEnvelope::Failure(ExecuteQueryFailure { ok: false, message }),
    }
}

async fn execute_query_async(input: ExecuteQueryInput) -> cosmic_data_engine::Result<QueryResult> {
    let profile = executable_profile(&input.connection_id)?;
    bootstrap_profile(&profile).await?;

    let connector = SqlxDatabaseConnector;
    let session = connector.connect(&profile, None).await?;
    session
        .execute_query(QueryRequest::new(
            input.connection_id,
            input.sql,
            input.max_rows.unwrap_or(100),
        ))
        .await
}

fn executable_profile(connection_id: &str) -> cosmic_data_engine::Result<ConnectionProfile> {
    match connection_id {
        "scratch" => Ok(ConnectionProfile::new_sqlite("Scratch", scratch_database_path())),
        other => Err(EngineError::Validation(format!(
            "Connection '{other}' is not available for query execution yet."
        ))),
    }
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
        let connections = active_connections();

        assert_eq!(connections[0].kind, "PostgreSQL");
        assert_eq!(connections[1].kind, "MySQL");
        assert_eq!(connections[2].kind, "SQLite");
    }
}
