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
        std::ffi::CString::new(r#"{"connectionId":"scratch","sql":"   ","maxRows":100}"#).unwrap();

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
    assert!(
        json.contains("not available for query execution yet"),
        "{json}"
    );
}

#[test]
fn open_connection_json_returns_success_for_scratch() {
    let request = std::ffi::CString::new(r#"{"connectionId":"scratch"}"#).unwrap();

    let ptr = cosmic_native_bridge::cosmic_open_connection_json(request.as_ptr());
    assert!(!ptr.is_null());

    let json = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_str()
        .unwrap()
        .to_string();
    unsafe {
        cosmic_native_bridge::cosmic_string_free(ptr);
    }

    assert!(json.contains(r#""ok":true"#), "{json}");
}

#[test]
fn load_schema_json_returns_scratch_tables() {
    let request = std::ffi::CString::new(r#"{"connectionId":"scratch"}"#).unwrap();

    let ptr = cosmic_native_bridge::cosmic_load_schema_json(request.as_ptr());
    assert!(!ptr.is_null());

    let json = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_str()
        .unwrap()
        .to_string();
    unsafe {
        cosmic_native_bridge::cosmic_string_free(ptr);
    }

    assert!(json.contains(r#""ok":true"#), "{json}");
    assert!(json.contains(r#""name":"users""#), "{json}");
    assert!(json.contains(r#""kind":"table""#), "{json}");
    assert!(json.contains(r#""columnCount":2"#), "{json}");
}

#[test]
fn preview_table_json_returns_top_rows() {
    let request = std::ffi::CString::new(
        r#"{"connectionId":"scratch","schema":null,"table":"users","maxRows":50}"#,
    )
    .unwrap();

    let ptr = cosmic_native_bridge::cosmic_preview_table_json(request.as_ptr());
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
