//! Core engine for Cosmic Data Explorer.

mod credentials;
mod database;
mod domain;
mod error;
mod highlight;
mod storage;

pub use credentials::{CredentialStore, InMemoryCredentialStore, KeyringCredentialStore};
pub use database::{
    ColumnInfo, DatabaseConnector, DatabaseSchema, DatabaseSession, SchemaTable,
    SqlxDatabaseConnector,
};
pub use domain::{
    CellValue, Column, ConnectionConfig, ConnectionProfile, CredentialRef, DatabaseKind,
    ParsedConnectionProfile, QueryHistoryEntry, QueryRequest, QueryResult, QueryRow, SslMode,
    TextRange,
};
pub use error::{EngineError, Result};
pub use highlight::{HighlightService, HighlightedDocument, HighlightedLine, HighlightedSpan};
pub use storage::AppStorage;
