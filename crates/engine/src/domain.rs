use std::{
    fmt,
    path::{Path, PathBuf},
};

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::{EngineError, Result};

pub const CREDENTIAL_SERVICE: &str = "cosmic-data-explorer";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DatabaseKind {
    Postgres,
    MySql,
    Sqlite,
}

impl DatabaseKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::MySql => "mysql",
            Self::Sqlite => "sqlite",
        }
    }

    pub fn sql_dialect(self) -> &'static str {
        match self {
            Self::Postgres => "PostgreSQL",
            Self::MySql => "MySQL",
            Self::Sqlite => "SQLite",
        }
    }
}

impl std::str::FromStr for DatabaseKind {
    type Err = EngineError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "postgres" | "postgresql" => Ok(Self::Postgres),
            "mysql" | "mariadb" => Ok(Self::MySql),
            "sqlite" => Ok(Self::Sqlite),
            other => Err(EngineError::Validation(format!(
                "unsupported database kind '{other}'"
            ))),
        }
    }
}

impl fmt::Display for DatabaseKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SslMode {
    Disabled,
    Preferred,
    Required,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CredentialRef {
    pub service: String,
    pub account: String,
}

impl CredentialRef {
    pub fn new(account: impl Into<String>) -> Self {
        Self {
            service: CREDENTIAL_SERVICE.to_string(),
            account: account.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ConnectionConfig {
    Network {
        host: String,
        port: u16,
        database: String,
        user: String,
        ssl_mode: SslMode,
    },
    PostgresUrl {
        url: String,
    },
    Sqlite {
        file_path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedConnectionProfile {
    pub profile: ConnectionProfile,
    pub password: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub id: String,
    pub display_name: String,
    pub kind: DatabaseKind,
    pub config: ConnectionConfig,
    pub credential_ref: CredentialRef,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ConnectionProfile {
    pub fn new_network(
        display_name: impl Into<String>,
        kind: DatabaseKind,
        host: impl Into<String>,
        port: u16,
        database: impl Into<String>,
        user: impl Into<String>,
        ssl_mode: SslMode,
    ) -> Self {
        let display_name = display_name.into();
        let user = user.into();
        let id = new_profile_id();
        let now = Utc::now();
        let credential_ref =
            CredentialRef::new(format!("{}-{}-{id}", slug(&display_name), slug(&user)));

        Self {
            id,
            display_name,
            kind,
            config: ConnectionConfig::Network {
                host: host.into(),
                port,
                database: database.into(),
                user,
                ssl_mode,
            },
            credential_ref,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn new_sqlite(display_name: impl Into<String>, file_path: impl AsRef<Path>) -> Self {
        let display_name = display_name.into();
        let id = new_profile_id();
        let now = Utc::now();
        let credential_ref = CredentialRef::new(format!("{}-sqlite-{id}", slug(&display_name)));

        Self {
            id,
            display_name,
            kind: DatabaseKind::Sqlite,
            config: ConnectionConfig::Sqlite {
                file_path: file_path.as_ref().to_path_buf(),
            },
            credential_ref,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn new_postgres_connection_string(
        display_name: impl Into<String>,
        connection_string: &str,
    ) -> Result<ParsedConnectionProfile> {
        let display_name = display_name.into();
        if display_name.trim().is_empty() {
            return Err(EngineError::Validation(
                "connection display name is required".to_string(),
            ));
        }

        let parsed = parse_postgres_connection_string(connection_string, true)?;
        let id = new_profile_id();
        let now = Utc::now();
        let credential_ref = CredentialRef::new(format!(
            "{}-{}-{id}",
            slug(&display_name),
            slug(&parsed.user)
        ));

        Ok(ParsedConnectionProfile {
            profile: Self {
                id,
                display_name,
                kind: DatabaseKind::Postgres,
                config: ConnectionConfig::PostgresUrl {
                    url: parsed.sanitized_url,
                },
                credential_ref,
                created_at: now,
                updated_at: now,
            },
            password: parsed.password,
        })
    }

    pub fn credential_ref(&self) -> CredentialRef {
        self.credential_ref.clone()
    }

    pub fn detail(&self) -> String {
        match &self.config {
            ConnectionConfig::Network { database, user, .. } => format!("{database} / {user}"),
            ConnectionConfig::PostgresUrl { url } => parse_postgres_connection_string(url, false)
                .map(|parsed| format!("{} / {}", parsed.database, parsed.user))
                .unwrap_or_else(|_| "PostgreSQL URL".to_string()),
            ConnectionConfig::Sqlite { file_path } => file_path.display().to_string(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.display_name.trim().is_empty() {
            return Err(EngineError::Validation(
                "connection display name is required".to_string(),
            ));
        }

        match (&self.kind, &self.config) {
            (
                DatabaseKind::Postgres | DatabaseKind::MySql,
                ConnectionConfig::Network {
                    host,
                    port,
                    database,
                    user,
                    ..
                },
            ) => {
                require_non_empty("host", host)?;
                if *port == 0 {
                    return Err(EngineError::Validation(
                        "port must be greater than 0".to_string(),
                    ));
                }
                require_non_empty("database", database)?;
                require_non_empty("user", user)?;
            }
            (DatabaseKind::Postgres, ConnectionConfig::PostgresUrl { url }) => {
                parse_postgres_connection_string(url, false)?;
            }
            (DatabaseKind::Sqlite, ConnectionConfig::Sqlite { file_path }) => {
                if file_path.as_os_str().is_empty() {
                    return Err(EngineError::Validation(
                        "SQLite file path is required".to_string(),
                    ));
                }
            }
            (kind, _) => {
                return Err(EngineError::Validation(format!(
                    "connection config does not match database kind {kind}"
                )));
            }
        }

        require_non_empty("credential service", &self.credential_ref.service)?;
        require_non_empty("credential account", &self.credential_ref.account)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ParsedPostgresConnectionString {
    sanitized_url: String,
    user: String,
    database: String,
    password: Option<String>,
}

fn parse_postgres_connection_string(
    connection_string: &str,
    allow_password: bool,
) -> Result<ParsedPostgresConnectionString> {
    let mut url = Url::parse(connection_string).map_err(|error| {
        EngineError::Validation(format!(
            "PostgreSQL connection string must be a valid URL: {error}"
        ))
    })?;

    if !matches!(url.scheme(), "postgres" | "postgresql") {
        return Err(EngineError::Validation(
            "PostgreSQL connection string must use postgres:// or postgresql://".to_string(),
        ));
    }

    let host = url.host_str().unwrap_or_default().trim();
    require_non_empty("host", host)?;

    let user = decode_url_part(url.username(), "user")?;
    require_non_empty("user", &user)?;

    let database = decode_url_part(url.path().trim_start_matches('/'), "database")?;
    require_non_empty("database", &database)?;

    let authority_password = url
        .password()
        .map(|password| decode_url_part(password, "password"))
        .transpose()?;
    let query_pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
    let query_password = query_pairs
        .iter()
        .find(|(key, _)| key == "password")
        .map(|(_, value)| value.clone());
    let password = authority_password.or(query_password);

    if !allow_password && password.is_some() {
        return Err(EngineError::Validation(
            "PostgreSQL connection profile metadata must not contain a password".to_string(),
        ));
    }

    url.set_password(None)
        .map_err(|_| EngineError::Validation("could not sanitize connection string".to_string()))?;

    if query_pairs.iter().any(|(key, _)| key == "password") {
        url.query_pairs_mut().clear().extend_pairs(
            query_pairs
                .iter()
                .filter(|(key, _)| key != "password")
                .map(|(key, value)| (&**key, &**value)),
        );
    }

    Ok(ParsedPostgresConnectionString {
        sanitized_url: url.to_string(),
        user,
        database,
        password,
    })
}

fn decode_url_part(value: &str, field: &str) -> Result<String> {
    percent_decode_str(value)
        .decode_utf8()
        .map(|value| value.to_string())
        .map_err(|error| EngineError::Validation(format!("{field} must be valid UTF-8: {error}")))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryRequest {
    pub connection_id: String,
    pub sql: String,
    pub max_rows: u32,
    pub timeout_ms: u64,
    pub selected_range: Option<TextRange>,
}

impl QueryRequest {
    pub fn new(connection_id: impl Into<String>, sql: impl Into<String>, max_rows: u32) -> Self {
        Self {
            connection_id: connection_id.into(),
            sql: sql.into(),
            max_rows,
            timeout_ms: 30_000,
            selected_range: None,
        }
    }

    pub fn selected_sql(&self) -> &str {
        if let Some(range) = &self.selected_range {
            self.sql.get(range.start..range.end).unwrap_or(&self.sql)
        } else {
            &self.sql
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub type_name: String,
    pub nullable: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub enum CellValue {
    Null,
    Text(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Bytes(Vec<u8>),
    Date(NaiveDate),
    Time(NaiveTime),
    DateTime(NaiveDateTime),
    Timestamp(DateTime<Utc>),
    Json(serde_json::Value),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryRow {
    pub cells: Vec<CellValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<Column>,
    pub rows: Vec<QueryRow>,
    pub rows_affected: u64,
    pub elapsed_ms: u128,
    pub truncated: bool,
}

impl QueryResult {
    pub fn empty(rows_affected: u64, elapsed_ms: u128) -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected,
            elapsed_ms,
            truncated: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    pub id: String,
    pub connection_id: String,
    pub sql: String,
    pub executed_at: DateTime<Utc>,
    pub elapsed_ms: u128,
    pub rows_returned: u64,
}

impl QueryHistoryEntry {
    pub fn new(
        connection_id: impl Into<String>,
        sql: impl Into<String>,
        elapsed_ms: u128,
        rows_returned: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            connection_id: connection_id.into(),
            sql: sql.into(),
            executed_at: Utc::now(),
            elapsed_ms,
            rows_returned,
        }
    }
}

fn require_non_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(EngineError::Validation(format!("{field} is required")))
    } else {
        Ok(())
    }
}

fn new_profile_id() -> String {
    format!("profile_{}", Uuid::new_v4())
}

fn slug(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for character in value.trim().chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "profile".to_string()
    } else {
        slug
    }
}
