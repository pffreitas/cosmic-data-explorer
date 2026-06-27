use std::{collections::BTreeMap, str::FromStr, time::Instant};

use async_trait::async_trait;
use futures_util::TryStreamExt;
use sqlx::{
    mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlRow, MySqlSslMode},
    postgres::{PgConnectOptions, PgPoolOptions, PgRow, PgSslMode},
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow},
    AssertSqlSafe, Column as SqlxColumn, Executor, MySqlPool, PgPool, Row, SqlSafeStr, SqlitePool,
    Statement, TypeInfo,
};

use crate::{
    CellValue, Column, ConnectionConfig, ConnectionProfile, DatabaseKind, EngineError,
    QueryRequest, QueryResult, QueryRow, Result, SslMode,
};

#[async_trait]
pub trait DatabaseConnector: Send + Sync {
    async fn test_connection(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<()>;

    async fn connect(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<DatabaseSession>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SqlxDatabaseConnector;

#[async_trait]
impl DatabaseConnector for SqlxDatabaseConnector {
    async fn test_connection(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<()> {
        let session = self.connect(profile, password).await?;
        session.ping().await
    }

    async fn connect(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> Result<DatabaseSession> {
        profile.validate()?;

        let pool = match (&profile.kind, &profile.config) {
            (DatabaseKind::Sqlite, ConnectionConfig::Sqlite { file_path }) => {
                let options = SqliteConnectOptions::new()
                    .filename(file_path)
                    .create_if_missing(true);
                DatabasePool::Sqlite(
                    SqlitePoolOptions::new()
                        .max_connections(5)
                        .connect_with(options)
                        .await?,
                )
            }
            (
                DatabaseKind::Postgres,
                ConnectionConfig::Network {
                    host,
                    port,
                    database,
                    user,
                    ssl_mode,
                },
            ) => {
                let mut options = PgConnectOptions::new()
                    .host(host)
                    .port(*port)
                    .database(database)
                    .username(user)
                    .ssl_mode(pg_ssl_mode(*ssl_mode));

                if let Some(password) = password {
                    options = options.password(password);
                }

                DatabasePool::Postgres(
                    PgPoolOptions::new()
                        .max_connections(5)
                        .connect_with(options)
                        .await?,
                )
            }
            (DatabaseKind::Postgres, ConnectionConfig::PostgresUrl { url }) => {
                let mut options = PgConnectOptions::from_str(url)?;

                if let Some(password) = password {
                    options = options.password(password);
                }
                if let Some(capacity) = postgres_statement_cache_capacity(url) {
                    options = options.statement_cache_capacity(capacity);
                }

                DatabasePool::Postgres(postgres_pool_options(url).connect_with(options).await?)
            }
            (
                DatabaseKind::MySql,
                ConnectionConfig::Network {
                    host,
                    port,
                    database,
                    user,
                    ssl_mode,
                },
            ) => {
                let mut options = MySqlConnectOptions::new()
                    .host(host)
                    .port(*port)
                    .database(database)
                    .username(user)
                    .ssl_mode(mysql_ssl_mode(*ssl_mode));

                if let Some(password) = password {
                    options = options.password(password);
                }

                DatabasePool::MySql(
                    MySqlPoolOptions::new()
                        .max_connections(5)
                        .connect_with(options)
                        .await?,
                )
            }
            _ => {
                return Err(EngineError::Validation(
                    "connection profile kind/config mismatch".to_string(),
                ));
            }
        };

        Ok(DatabaseSession {
            profile_id: profile.id.clone(),
            kind: profile.kind,
            pool,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseSession {
    pub profile_id: String,
    pub kind: DatabaseKind,
    pool: DatabasePool,
}

impl DatabaseSession {
    pub async fn ping(&self) -> Result<()> {
        match &self.pool {
            DatabasePool::Sqlite(pool) => {
                sqlx::query("select 1").execute(pool).await?;
            }
            DatabasePool::Postgres(pool) => {
                sqlx::query("select 1").execute(pool).await?;
            }
            DatabasePool::MySql(pool) => {
                sqlx::query("select 1").execute(pool).await?;
            }
        }
        Ok(())
    }

    pub async fn load_schema(&self) -> Result<DatabaseSchema> {
        match &self.pool {
            DatabasePool::Sqlite(pool) => load_sqlite_schema(pool).await,
            DatabasePool::Postgres(pool) => load_postgres_schema(pool).await,
            DatabasePool::MySql(pool) => load_mysql_schema(pool).await,
        }
    }

    pub async fn preview_table(
        &self,
        schema: Option<&str>,
        table: &str,
        max_rows: u32,
    ) -> Result<QueryResult> {
        let sql = match self.kind {
            DatabaseKind::Sqlite | DatabaseKind::Postgres => format!(
                "select * from {} limit {}",
                quote_path_double(schema, table)?,
                max_rows.max(1)
            ),
            DatabaseKind::MySql => format!(
                "select * from {} limit {}",
                quote_path_backtick(schema, table)?,
                max_rows.max(1)
            ),
        };

        self.execute_query(QueryRequest::new(
            self.profile_id.clone(),
            sql,
            max_rows.max(1),
        ))
        .await
    }

    pub async fn execute_query(&self, request: QueryRequest) -> Result<QueryResult> {
        let timeout_ms = request.timeout_ms;
        tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            self.execute_query_inner(request),
        )
        .await
        .map_err(|_| EngineError::Timeout(timeout_ms))?
    }

    async fn execute_query_inner(&self, request: QueryRequest) -> Result<QueryResult> {
        let sql = request.selected_sql().trim().to_string();
        if sql.is_empty() {
            return Err(EngineError::Validation("SQL text is required".to_string()));
        }

        let max_rows = request.max_rows.max(1) as usize;
        match &self.pool {
            DatabasePool::Sqlite(pool) => execute_sqlite(pool, &sql, max_rows).await,
            DatabasePool::Postgres(pool) => execute_postgres(pool, &sql, max_rows).await,
            DatabasePool::MySql(pool) => execute_mysql(pool, &sql, max_rows).await,
        }
    }
}

#[derive(Debug, Clone)]
enum DatabasePool {
    Postgres(PgPool),
    MySql(MySqlPool),
    Sqlite(SqlitePool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseSchema {
    pub tables: Vec<SchemaTable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaTable {
    pub schema: Option<String>,
    pub name: String,
    pub kind: String,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnInfo {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
    pub ordinal: u32,
}

async fn execute_sqlite(pool: &SqlitePool, sql: &str, max_rows: usize) -> Result<QueryResult> {
    if query_returns_rows(sql) {
        collect_sqlite_rows(pool, sql, max_rows).await
    } else {
        let start = Instant::now();
        let result = sqlx::query(AssertSqlSafe(sql)).execute(pool).await?;
        Ok(QueryResult::empty(
            result.rows_affected(),
            start.elapsed().as_millis(),
        ))
    }
}

async fn execute_postgres(pool: &PgPool, sql: &str, max_rows: usize) -> Result<QueryResult> {
    if query_returns_rows(sql) {
        collect_postgres_rows(pool, sql, max_rows).await
    } else {
        let start = Instant::now();
        let result = sqlx::query(AssertSqlSafe(sql)).execute(pool).await?;
        Ok(QueryResult::empty(
            result.rows_affected(),
            start.elapsed().as_millis(),
        ))
    }
}

async fn execute_mysql(pool: &MySqlPool, sql: &str, max_rows: usize) -> Result<QueryResult> {
    if query_returns_rows(sql) {
        collect_mysql_rows(pool, sql, max_rows).await
    } else {
        let start = Instant::now();
        let result = sqlx::query(AssertSqlSafe(sql)).execute(pool).await?;
        Ok(QueryResult::empty(
            result.rows_affected(),
            start.elapsed().as_millis(),
        ))
    }
}

async fn collect_sqlite_rows(pool: &SqlitePool, sql: &str, max_rows: usize) -> Result<QueryResult> {
    let start = Instant::now();
    let statement = pool.prepare(AssertSqlSafe(sql).into_sql_str()).await?;
    let columns = columns_from_sqlx(statement.columns());
    let mut stream = statement.query().fetch(pool);
    let mut rows = Vec::new();
    let mut truncated = false;

    while let Some(row) = stream.try_next().await? {
        if rows.len() >= max_rows {
            truncated = true;
            break;
        }
        rows.push(QueryRow {
            cells: sqlite_cells(&row),
        });
    }

    Ok(QueryResult {
        columns,
        rows,
        rows_affected: 0,
        elapsed_ms: start.elapsed().as_millis(),
        truncated,
    })
}

async fn collect_postgres_rows(pool: &PgPool, sql: &str, max_rows: usize) -> Result<QueryResult> {
    let start = Instant::now();
    let statement = pool.prepare(AssertSqlSafe(sql).into_sql_str()).await?;
    let columns = columns_from_sqlx(statement.columns());
    let mut stream = statement.query().fetch(pool);
    let mut rows = Vec::new();
    let mut truncated = false;

    while let Some(row) = stream.try_next().await? {
        if rows.len() >= max_rows {
            truncated = true;
            break;
        }
        rows.push(QueryRow {
            cells: postgres_cells(&row),
        });
    }

    Ok(QueryResult {
        columns,
        rows,
        rows_affected: 0,
        elapsed_ms: start.elapsed().as_millis(),
        truncated,
    })
}

async fn collect_mysql_rows(pool: &MySqlPool, sql: &str, max_rows: usize) -> Result<QueryResult> {
    let start = Instant::now();
    let statement = pool.prepare(AssertSqlSafe(sql).into_sql_str()).await?;
    let columns = columns_from_sqlx(statement.columns());
    let mut stream = statement.query().fetch(pool);
    let mut rows = Vec::new();
    let mut truncated = false;

    while let Some(row) = stream.try_next().await? {
        if rows.len() >= max_rows {
            truncated = true;
            break;
        }
        rows.push(QueryRow {
            cells: mysql_cells(&row),
        });
    }

    Ok(QueryResult {
        columns,
        rows,
        rows_affected: 0,
        elapsed_ms: start.elapsed().as_millis(),
        truncated,
    })
}

async fn load_sqlite_schema(pool: &SqlitePool) -> Result<DatabaseSchema> {
    let table_rows = sqlx::query(
        r#"
        select name, type
        from sqlite_master
        where type in ('table', 'view')
          and name not like 'sqlite_%'
        order by name
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut tables = Vec::new();
    for table_row in table_rows {
        let table_name = table_row.try_get::<String, _>("name")?;
        let table_kind = table_row.try_get::<String, _>("type")?;
        let pragma = format!(
            "pragma table_info({})",
            quote_identifier_double(&table_name)?
        );
        let column_rows = sqlx::query(AssertSqlSafe(pragma)).fetch_all(pool).await?;
        let mut columns = Vec::new();

        for column_row in column_rows {
            let ordinal = column_row.try_get::<i64, _>("cid")?.max(0) as u32;
            let not_null = column_row.try_get::<i64, _>("notnull")? != 0;
            columns.push(ColumnInfo {
                name: column_row.try_get("name")?,
                type_name: column_row.try_get("type")?,
                nullable: !not_null,
                ordinal,
            });
        }

        tables.push(SchemaTable {
            schema: None,
            name: table_name,
            kind: normalize_table_kind(&table_kind),
            columns,
        });
    }

    Ok(DatabaseSchema { tables })
}

async fn load_postgres_schema(pool: &PgPool) -> Result<DatabaseSchema> {
    let rows = sqlx::query(
        r#"
        select
            c.table_schema,
            c.table_name,
            t.table_type,
            c.column_name,
            c.data_type,
            c.is_nullable,
            c.ordinal_position
        from information_schema.columns c
        join information_schema.tables t
          on t.table_schema = c.table_schema
         and t.table_name = c.table_name
        where c.table_schema not in ('pg_catalog', 'information_schema')
        order by c.table_schema, c.table_name, c.ordinal_position
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(DatabaseSchema {
        tables: grouped_schema_rows(rows.into_iter().map(|row| {
            Ok((
                Some(row.try_get::<String, _>("table_schema")?),
                row.try_get::<String, _>("table_name")?,
                normalize_table_kind(&row.try_get::<String, _>("table_type")?),
                ColumnInfo {
                    name: row.try_get("column_name")?,
                    type_name: row.try_get("data_type")?,
                    nullable: row.try_get::<String, _>("is_nullable")? == "YES",
                    ordinal: row.try_get::<i32, _>("ordinal_position")?.max(0) as u32,
                },
            ))
        }))?,
    })
}

async fn load_mysql_schema(pool: &MySqlPool) -> Result<DatabaseSchema> {
    let rows = sqlx::query(
        r#"
        select
            c.table_schema,
            c.table_name,
            t.table_type,
            c.column_name,
            c.data_type,
            c.is_nullable,
            c.ordinal_position
        from information_schema.columns c
        join information_schema.tables t
          on t.table_schema = c.table_schema
         and t.table_name = c.table_name
        where c.table_schema = database()
        order by c.table_schema, c.table_name, c.ordinal_position
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(DatabaseSchema {
        tables: grouped_schema_rows(rows.into_iter().map(|row| {
            Ok((
                Some(row.try_get::<String, _>("table_schema")?),
                row.try_get::<String, _>("table_name")?,
                normalize_table_kind(&row.try_get::<String, _>("table_type")?),
                ColumnInfo {
                    name: row.try_get("column_name")?,
                    type_name: row.try_get("data_type")?,
                    nullable: row.try_get::<String, _>("is_nullable")? == "YES",
                    ordinal: row.try_get::<i32, _>("ordinal_position")?.max(0) as u32,
                },
            ))
        }))?,
    })
}

fn grouped_schema_rows(
    rows: impl Iterator<Item = Result<(Option<String>, String, String, ColumnInfo)>>,
) -> Result<Vec<SchemaTable>> {
    let mut grouped: BTreeMap<(Option<String>, String, String), Vec<ColumnInfo>> = BTreeMap::new();

    for row in rows {
        let (schema, table, kind, column) = row?;
        grouped
            .entry((schema, table, kind))
            .or_default()
            .push(column);
    }

    Ok(grouped
        .into_iter()
        .map(|((schema, name, kind), columns)| SchemaTable {
            schema,
            name,
            kind,
            columns,
        })
        .collect())
}

fn normalize_table_kind(kind: &str) -> String {
    match kind.to_ascii_lowercase().as_str() {
        "base table" | "table" => "table".to_string(),
        "view" => "view".to_string(),
        other => other.replace(' ', "_"),
    }
}

fn columns_from_sqlx<C>(columns: &[C]) -> Vec<Column>
where
    C: SqlxColumn,
{
    columns
        .iter()
        .map(|column| Column {
            name: column.name().to_string(),
            type_name: column.type_info().name().to_string(),
            nullable: None,
        })
        .collect()
}

fn sqlite_cells(row: &SqliteRow) -> Vec<CellValue> {
    (0..row.len())
        .map(|index| sqlite_cell(row, index))
        .collect()
}

fn postgres_cells(row: &PgRow) -> Vec<CellValue> {
    (0..row.len())
        .map(|index| postgres_cell(row, index))
        .collect()
}

fn mysql_cells(row: &MySqlRow) -> Vec<CellValue> {
    (0..row.len()).map(|index| mysql_cell(row, index)).collect()
}

fn sqlite_cell(row: &SqliteRow, index: usize) -> CellValue {
    if let Ok(value) = row.try_get::<Option<i64>, _>(index) {
        return value.map(CellValue::Integer).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<f64>, _>(index) {
        return value.map(CellValue::Float).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<bool>, _>(index) {
        return value.map(CellValue::Boolean).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<String>, _>(index) {
        return value.map(CellValue::Text).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<Vec<u8>>, _>(index) {
        return value.map(CellValue::Bytes).unwrap_or(CellValue::Null);
    }
    CellValue::Null
}

fn postgres_cell(row: &PgRow, index: usize) -> CellValue {
    if let Ok(value) = row.try_get::<Option<bool>, _>(index) {
        return value.map(CellValue::Boolean).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<i64>, _>(index) {
        return value.map(CellValue::Integer).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<i32>, _>(index) {
        return value
            .map(|value| CellValue::Integer(i64::from(value)))
            .unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<f64>, _>(index) {
        return value.map(CellValue::Float).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<chrono::NaiveDate>, _>(index) {
        return value.map(CellValue::Date).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<chrono::NaiveTime>, _>(index) {
        return value.map(CellValue::Time).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<chrono::NaiveDateTime>, _>(index) {
        return value.map(CellValue::DateTime).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(index) {
        return value.map(CellValue::Timestamp).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<serde_json::Value>, _>(index) {
        return value.map(CellValue::Json).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<String>, _>(index) {
        return value.map(CellValue::Text).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<Vec<u8>>, _>(index) {
        return value.map(CellValue::Bytes).unwrap_or(CellValue::Null);
    }
    CellValue::Null
}

fn mysql_cell(row: &MySqlRow, index: usize) -> CellValue {
    if let Ok(value) = row.try_get::<Option<bool>, _>(index) {
        return value.map(CellValue::Boolean).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<i64>, _>(index) {
        return value.map(CellValue::Integer).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<i32>, _>(index) {
        return value
            .map(|value| CellValue::Integer(i64::from(value)))
            .unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<f64>, _>(index) {
        return value.map(CellValue::Float).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<chrono::NaiveDate>, _>(index) {
        return value.map(CellValue::Date).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<chrono::NaiveTime>, _>(index) {
        return value.map(CellValue::Time).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<chrono::NaiveDateTime>, _>(index) {
        return value.map(CellValue::DateTime).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<serde_json::Value>, _>(index) {
        return value.map(CellValue::Json).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<String>, _>(index) {
        return value.map(CellValue::Text).unwrap_or(CellValue::Null);
    }
    if let Ok(value) = row.try_get::<Option<Vec<u8>>, _>(index) {
        return value.map(CellValue::Bytes).unwrap_or(CellValue::Null);
    }
    CellValue::Null
}

fn query_returns_rows(sql: &str) -> bool {
    let Some(first_word) = sql.split_whitespace().next() else {
        return false;
    };

    matches!(
        first_word.to_ascii_lowercase().as_str(),
        "select" | "with" | "show" | "pragma" | "explain" | "describe" | "values"
    )
}

fn quote_path_double(schema: Option<&str>, table: &str) -> Result<String> {
    let table = quote_identifier_double(table)?;
    Ok(if let Some(schema) = schema {
        format!("{}.{}", quote_identifier_double(schema)?, table)
    } else {
        table
    })
}

fn quote_path_backtick(schema: Option<&str>, table: &str) -> Result<String> {
    let table = quote_identifier_backtick(table)?;
    Ok(if let Some(schema) = schema {
        format!("{}.{}", quote_identifier_backtick(schema)?, table)
    } else {
        table
    })
}

fn quote_identifier_double(identifier: &str) -> Result<String> {
    validate_identifier(identifier)?;
    Ok(format!("\"{identifier}\""))
}

fn quote_identifier_backtick(identifier: &str) -> Result<String> {
    validate_identifier(identifier)?;
    Ok(format!("`{identifier}`"))
}

fn validate_identifier(identifier: &str) -> Result<()> {
    if identifier.is_empty()
        || !identifier
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(EngineError::InvalidIdentifier(identifier.to_string()));
    }
    Ok(())
}

fn pg_ssl_mode(mode: SslMode) -> PgSslMode {
    match mode {
        SslMode::Disabled => PgSslMode::Disable,
        SslMode::Preferred => PgSslMode::Prefer,
        SslMode::Required => PgSslMode::Require,
    }
}

fn mysql_ssl_mode(mode: SslMode) -> MySqlSslMode {
    match mode {
        SslMode::Disabled => MySqlSslMode::Disabled,
        SslMode::Preferred => MySqlSslMode::Preferred,
        SslMode::Required => MySqlSslMode::Required,
    }
}

fn postgres_pool_options(url: &str) -> PgPoolOptions {
    let mut options = PgPoolOptions::new().max_connections(postgres_connection_limit(url));

    if let Some(pool_timeout) = postgres_query_value(url, "pool_timeout")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
    {
        options = options.acquire_timeout(std::time::Duration::from_secs(pool_timeout));
    }

    options
}

fn postgres_connection_limit(url: &str) -> u32 {
    postgres_query_value(url, "connection_limit")
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(5)
}

fn postgres_statement_cache_capacity(url: &str) -> Option<usize> {
    postgres_query_value(url, "statement_cache_size")
        .or_else(|| postgres_query_value(url, "statement-cache-capacity"))
        .and_then(|value| value.parse::<usize>().ok())
        .or_else(|| {
            let pgbouncer = postgres_query_value(url, "pgbouncer")?;
            pgbouncer.eq_ignore_ascii_case("true").then_some(0)
        })
}

fn postgres_query_value(url: &str, key: &str) -> Option<String> {
    let url = url::Url::parse(url).ok()?;
    url.query_pairs()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, value)| value.into_owned())
}
