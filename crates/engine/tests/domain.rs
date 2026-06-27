use cosmic_data_engine::{ConnectionProfile, DatabaseKind, SslMode};

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
