use cosmic_data_engine::{ConnectionProfile, DatabaseKind, SslMode};

#[test]
fn postgres_row_decoder_handles_uuid_columns_before_text_fallback() {
    let source = include_str!("../src/database.rs");
    let start = source
        .find("fn postgres_cell(")
        .expect("postgres cell decoder should exist");
    let end = source[start..]
        .find("fn mysql_cell(")
        .map(|offset| start + offset)
        .expect("mysql cell decoder should follow postgres decoder");
    let postgres_cell_source = &source[start..end];
    let uuid_decode = "row.try_get::<Option<uuid::Uuid>, _>(index)";
    let text_decode = "row.try_get::<Option<String>, _>(index)";

    let uuid_position = postgres_cell_source
        .find(uuid_decode)
        .expect("postgres UUID columns should decode to a displayable cell value");
    let text_position = postgres_cell_source
        .find(text_decode)
        .expect("postgres text fallback should remain available");

    assert!(
        uuid_position < text_position,
        "UUID decoding must run before the generic text fallback"
    );
}

#[test]
fn sqlite_profile_requires_a_file_path() {
    let profile = ConnectionProfile::new_sqlite("Local", "");

    assert!(profile.validate().is_err());
}

#[test]
fn network_profile_generates_a_stable_credential_reference_without_password() {
    let profile = ConnectionProfile::new_network(
        "Warehouse",
        DatabaseKind::Postgres,
        "localhost",
        5432,
        "analytics",
        "paulo",
        SslMode::Preferred,
    );

    let credential = profile.credential_ref();

    assert_eq!(credential.service, "cosmic-data-explorer");
    assert!(credential.account.contains("warehouse"));
    assert!(credential.account.contains("paulo"));
    assert!(!format!("{profile:?}").contains("password"));
}
