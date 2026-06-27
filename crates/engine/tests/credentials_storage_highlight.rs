use cosmic_data_engine::{
    AppStorage, ConnectionProfile, CredentialStore, DatabaseKind, HighlightService,
    InMemoryCredentialStore,
};

#[tokio::test]
async fn profile_metadata_round_trips_without_password() {
    let temp = tempfile::tempdir().unwrap();
    let storage = AppStorage::connect(temp.path().join("app.sqlite"))
        .await
        .unwrap();
    storage.initialize().await.unwrap();

    let profile = ConnectionProfile::new_sqlite("Local", temp.path().join("data.sqlite"));
    storage.save_profile(&profile).await.unwrap();

    let profiles = storage.list_profiles().await.unwrap();

    assert_eq!(profiles, vec![profile]);
    assert!(!format!("{profiles:?}").contains("password"));
}

#[test]
fn in_memory_credentials_store_and_delete_passwords() {
    let profile = ConnectionProfile::new_sqlite("Local", "data.sqlite");
    let store = InMemoryCredentialStore::default();
    let credential = profile.credential_ref();

    store.set_password(&credential, "secret").unwrap();
    assert_eq!(
        store.get_password(&credential).unwrap(),
        Some("secret".to_string())
    );
    store.delete_password(&credential).unwrap();
    assert_eq!(store.get_password(&credential).unwrap(), None);
}

#[test]
fn sql_highlighter_preserves_source_text() {
    let doc = HighlightService::default()
        .highlight_sql("select * from users", DatabaseKind::Postgres)
        .unwrap();

    assert_eq!(doc.plain_text(), "select * from users");
    assert!(!doc.lines[0].spans.is_empty());
}
