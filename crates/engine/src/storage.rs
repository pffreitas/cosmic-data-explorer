use std::{path::Path, str::FromStr};

use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};

use crate::{
    ConnectionConfig, ConnectionProfile, CredentialRef, DatabaseKind, QueryHistoryEntry, Result,
};

#[derive(Debug, Clone)]
pub struct AppStorage {
    pool: SqlitePool,
}

impl AppStorage {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path.as_ref())
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    pub fn default_database_path() -> Option<std::path::PathBuf> {
        ProjectDirs::from("dev", "Cosmic Data Explorer", "Cosmic Data Explorer")
            .map(|dirs| dirs.data_dir().join("cosmic-data-explorer.sqlite"))
    }

    pub async fn initialize(&self) -> Result<()> {
        sqlx::query(
            r#"
            create table if not exists connection_profiles (
                id text primary key,
                display_name text not null,
                kind text not null,
                config_json text not null,
                credential_service text not null,
                credential_account text not null,
                created_at text not null,
                updated_at text not null
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            create table if not exists query_history (
                id text primary key,
                connection_id text not null,
                sql text not null,
                executed_at text not null,
                elapsed_ms integer not null,
                rows_returned integer not null
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn save_profile(&self, profile: &ConnectionProfile) -> Result<()> {
        profile.validate()?;

        sqlx::query(
            r#"
            insert into connection_profiles (
                id,
                display_name,
                kind,
                config_json,
                credential_service,
                credential_account,
                created_at,
                updated_at
            )
            values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            on conflict(id) do update set
                display_name = excluded.display_name,
                kind = excluded.kind,
                config_json = excluded.config_json,
                credential_service = excluded.credential_service,
                credential_account = excluded.credential_account,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&profile.id)
        .bind(&profile.display_name)
        .bind(profile.kind.as_str())
        .bind(serde_json::to_string(&profile.config)?)
        .bind(&profile.credential_ref.service)
        .bind(&profile.credential_ref.account)
        .bind(profile.created_at.to_rfc3339())
        .bind(profile.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_profiles(&self) -> Result<Vec<ConnectionProfile>> {
        let rows = sqlx::query(
            r#"
            select id,
                   display_name,
                   kind,
                   config_json,
                   credential_service,
                   credential_account,
                   created_at,
                   updated_at
            from connection_profiles
            order by display_name collate nocase
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(profile_from_row).collect()
    }

    pub async fn delete_profile(&self, profile_id: &str) -> Result<()> {
        sqlx::query("delete from connection_profiles where id = ?1")
            .bind(profile_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn append_query_history(&self, entry: &QueryHistoryEntry) -> Result<()> {
        sqlx::query(
            r#"
            insert into query_history (
                id,
                connection_id,
                sql,
                executed_at,
                elapsed_ms,
                rows_returned
            )
            values (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&entry.id)
        .bind(&entry.connection_id)
        .bind(&entry.sql)
        .bind(entry.executed_at.to_rfc3339())
        .bind(i64::try_from(entry.elapsed_ms).unwrap_or(i64::MAX))
        .bind(i64::try_from(entry.rows_returned).unwrap_or(i64::MAX))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn recent_query_history(
        &self,
        connection_id: &str,
        limit: u32,
    ) -> Result<Vec<QueryHistoryEntry>> {
        let rows = sqlx::query(
            r#"
            select id, connection_id, sql, executed_at, elapsed_ms, rows_returned
            from query_history
            where connection_id = ?1
            order by executed_at desc
            limit ?2
            "#,
        )
        .bind(connection_id)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let executed_at = parse_utc(row.try_get::<String, _>("executed_at")?)?;
                let elapsed_ms = row.try_get::<i64, _>("elapsed_ms")?;
                let rows_returned = row.try_get::<i64, _>("rows_returned")?;

                Ok(QueryHistoryEntry {
                    id: row.try_get("id")?,
                    connection_id: row.try_get("connection_id")?,
                    sql: row.try_get("sql")?,
                    executed_at,
                    elapsed_ms: elapsed_ms.max(0) as u128,
                    rows_returned: rows_returned.max(0) as u64,
                })
            })
            .collect()
    }
}

fn profile_from_row(row: sqlx::sqlite::SqliteRow) -> Result<ConnectionProfile> {
    let kind = DatabaseKind::from_str(&row.try_get::<String, _>("kind")?)?;
    let config_json = row.try_get::<String, _>("config_json")?;
    let config = serde_json::from_str::<ConnectionConfig>(&config_json)?;

    Ok(ConnectionProfile {
        id: row.try_get("id")?,
        display_name: row.try_get("display_name")?,
        kind,
        config,
        credential_ref: CredentialRef {
            service: row.try_get("credential_service")?,
            account: row.try_get("credential_account")?,
        },
        created_at: parse_utc(row.try_get::<String, _>("created_at")?)?,
        updated_at: parse_utc(row.try_get::<String, _>("updated_at")?)?,
    })
}

fn parse_utc(value: String) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(&value)?.with_timezone(&Utc))
}
