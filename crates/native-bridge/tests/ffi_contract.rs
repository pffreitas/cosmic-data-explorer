#[test]
fn active_connections_json_returns_mock_connections() {
    let ptr = cosmic_native_bridge::cosmic_active_connections_json();
    assert!(!ptr.is_null());

    let json = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_str()
        .unwrap()
        .to_string();
    unsafe {
        cosmic_native_bridge::cosmic_string_free(ptr);
    }

    assert!(json.contains("Production"));
    assert!(json.contains("PostgreSQL"));
}

#[test]
fn execute_query_json_returns_sqlite_rows() {
    let request = std::ffi::CString::new(
        r#"{"connectionId":"scratch","sql":"select id, name from users order by id","maxRows":100}"#,
    )
    .unwrap();

    let ptr = cosmic_native_bridge::cosmic_execute_query_json(request.as_ptr());
    assert!(!ptr.is_null());

    let json = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_str()
        .unwrap()
        .to_string();
    unsafe {
        cosmic_native_bridge::cosmic_string_free(ptr);
    }

    assert!(json.contains(r#""ok":true"#), "{json}");
    assert!(json.contains("Ada"), "{json}");
    assert!(json.contains("Grace"), "{json}");
}

#[test]
fn execute_query_json_returns_failure_for_empty_sql() {
    let request =
        std::ffi::CString::new(r#"{"connectionId":"scratch","sql":"   ","maxRows":100}"#)
            .unwrap();

    let ptr = cosmic_native_bridge::cosmic_execute_query_json(request.as_ptr());
    assert!(!ptr.is_null());

    let json = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_str()
        .unwrap()
        .to_string();
    unsafe {
        cosmic_native_bridge::cosmic_string_free(ptr);
    }

    assert!(json.contains(r#""ok":false"#), "{json}");
    assert!(json.contains("SQL text is required"), "{json}");
}

#[test]
fn execute_query_json_returns_failure_for_unresolved_connections() {
    let request =
        std::ffi::CString::new(r#"{"connectionId":"production","sql":"select 1","maxRows":100}"#)
            .unwrap();

    let ptr = cosmic_native_bridge::cosmic_execute_query_json(request.as_ptr());
    assert!(!ptr.is_null());

    let json = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_str()
        .unwrap()
        .to_string();
    unsafe {
        cosmic_native_bridge::cosmic_string_free(ptr);
    }

    assert!(json.contains(r#""ok":false"#), "{json}");
    assert!(json.contains("not available for query execution yet"), "{json}");
}
