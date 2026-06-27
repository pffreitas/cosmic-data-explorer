use cosmic_data_engine::{ConnectionConfig, ConnectionProfile, DatabaseKind};

const POSTGRES_URL: &str = "postgres://admin_bees-force-bug-hunt-hackathon:)cbL%2BHH1%26e_A@psqls-beesdev-portal.postgres.database.azure.com/pg-bees-force-bug-hunt-hackathon?sslmode=require&connection_limit=10&pool_timeout=20&connect_timeout=10&statement_timeout=0&idle_in_transaction_session_timeout=0&pgbouncer=true&statement_cache_size=0&server_lifetime=3600&server_idle_timeout=900";

#[test]
fn postgres_connection_string_creates_sanitized_profile_and_extracts_password() {
    let parsed = ConnectionProfile::new_postgres_connection_string("Hackathon", POSTGRES_URL)
        .expect("valid postgres connection string");

    assert_eq!(parsed.password.as_deref(), Some(")cbL+HH1&e_A"));
    assert_eq!(parsed.profile.display_name, "Hackathon");
    assert_eq!(parsed.profile.kind, DatabaseKind::Postgres);
    parsed.profile.validate().unwrap();

    let ConnectionConfig::PostgresUrl { url } = &parsed.profile.config else {
        panic!("expected postgres URL config");
    };

    assert!(url.starts_with("postgres://admin_bees-force-bug-hunt-hackathon@"));
    assert!(url.contains("sslmode=require"));
    assert!(url.contains("pgbouncer=true"));
    assert!(url.contains("server_idle_timeout=900"));
    assert!(!url.contains(")cbL"));
    assert!(!url.contains("%2BHH1"));
}

#[test]
fn postgres_connection_string_rejects_unsupported_schemes() {
    let error = ConnectionProfile::new_postgres_connection_string(
        "Analytics",
        "mysql://root:secret@localhost/events",
    )
    .expect_err("mysql URLs are out of scope");

    assert!(error.to_string().contains("PostgreSQL connection string"));
}

#[test]
fn postgres_connection_string_rejects_missing_required_parts() {
    let error = ConnectionProfile::new_postgres_connection_string(
        "No Database",
        "postgres://admin:secret@localhost",
    )
    .expect_err("database name is required");

    assert!(error.to_string().contains("database"));
}

#[test]
fn postgres_connection_string_requires_a_display_name() {
    let error = ConnectionProfile::new_postgres_connection_string(" ", POSTGRES_URL)
        .expect_err("display name is required");

    assert!(error.to_string().contains("display name"));
}
