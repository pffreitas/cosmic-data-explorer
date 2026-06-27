use std::{path::PathBuf, sync::Mutex};

use cosmic_data_engine::{
    ConnectionProfile, CredentialStore, DatabaseConnector, DatabaseSession, InMemoryCredentialStore,
};

struct RecordingConnector {
    tested_profiles: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl DatabaseConnector for RecordingConnector {
    async fn test_connection(
        &self,
        profile: &ConnectionProfile,
        password: Option<&str>,
    ) -> cosmic_data_engine::Result<()> {
        assert_eq!(password, Some("secret"));
        self.tested_profiles
            .lock()
            .unwrap()
            .push(profile.display_name.clone());
        Ok(())
    }

    async fn connect(
        &self,
        _profile: &ConnectionProfile,
        _password: Option<&str>,
    ) -> cosmic_data_engine::Result<DatabaseSession> {
        panic!("create connection should only test, not open a query session");
    }
}

#[tokio::test]
async fn service_tests_connection_before_saving_profile() {
    let storage_path = temp_storage_path();
    let credentials = InMemoryCredentialStore::default();
    let connector = RecordingConnector {
        tested_profiles: Mutex::new(Vec::new()),
    };
    let service = cosmic_native_bridge::ConnectionService::new(
        storage_path.clone(),
        credentials.clone(),
        connector,
    );

    let created = service
        .create_connection(
            "Hackathon",
            "postgres://admin:secret@localhost/hackathon?sslmode=require",
        )
        .await
        .expect("connection should be created");

    assert_eq!(created.name, "Hackathon");
    assert_eq!(created.kind, "PostgreSQL");
    assert_eq!(created.detail, "hackathon / admin");

    let listed = service.active_connections().await.unwrap();
    assert!(listed.iter().any(|connection| connection.id == created.id));

    let profile = service.resolve_profile(&created.id).await.unwrap();
    assert_eq!(
        credentials
            .get_password(&profile.credential_ref())
            .unwrap()
            .as_deref(),
        Some("secret")
    );
}

#[test]
fn ffi_create_connection_returns_failure_for_invalid_json() {
    let request = std::ffi::CString::new(r#"{"name":"","connectionString":"not a url"}"#).unwrap();

    let ptr = cosmic_native_bridge::cosmic_create_connection_json(request.as_ptr());
    assert!(!ptr.is_null());

    let json = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_str()
        .unwrap()
        .to_string();
    unsafe {
        cosmic_native_bridge::cosmic_string_free(ptr);
    }

    assert!(json.contains(r#""ok":false"#), "{json}");
    assert!(json.contains("display name"), "{json}");
}

fn temp_storage_path() -> PathBuf {
    tempfile::tempdir()
        .unwrap()
        .keep()
        .join("cosmic-data-explorer.sqlite")
}
