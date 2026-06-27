use cosmic_data_engine::{
    CellValue, ConnectionProfile, DatabaseConnector, QueryRequest, SqlxDatabaseConnector,
};

#[tokio::test]
async fn sqlite_connector_executes_queries_and_previews_tables() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("sample.sqlite");
    let profile = ConnectionProfile::new_sqlite("Local", &db);
    let connector = SqlxDatabaseConnector;
    let session = connector.connect(&profile, None).await.unwrap();

    session
        .execute_query(QueryRequest::new(
            profile.id.clone(),
            "create table users (id integer primary key, name text)",
            100,
        ))
        .await
        .unwrap();
    session
        .execute_query(QueryRequest::new(
            profile.id.clone(),
            "insert into users (name) values ('Ada')",
            100,
        ))
        .await
        .unwrap();
    session
        .execute_query(QueryRequest::new(
            profile.id.clone(),
            "create table empty_users (id integer primary key, name text)",
            100,
        ))
        .await
        .unwrap();

    let result = session
        .execute_query(QueryRequest::new(
            profile.id.clone(),
            "select id, name from users",
            100,
        ))
        .await
        .unwrap();

    assert_eq!(
        result
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        vec!["id", "name"]
    );
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].cells[1], CellValue::Text("Ada".to_string()));

    let preview = session.preview_table(None, "users", 50).await.unwrap();
    assert_eq!(preview.rows.len(), 1);

    let empty_preview = session
        .preview_table(None, "empty_users", 50)
        .await
        .unwrap();
    assert_eq!(
        empty_preview
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        vec!["id", "name"]
    );
    assert_eq!(empty_preview.rows.len(), 0);

    let schema = session.load_schema().await.unwrap();
    let users = schema
        .tables
        .iter()
        .find(|table| table.name == "users")
        .expect("users table should be listed");
    assert_eq!(users.kind, "table");
    assert_eq!(users.columns.len(), 2);
}
