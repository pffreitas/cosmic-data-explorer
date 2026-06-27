slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;
    app.set_status_text("Ready".into());
    app.set_query_text("select 1;".into());

    let weak_app = app.as_weak();
    app.on_execute_query(move |sql| {
        if let Some(app) = weak_app.upgrade() {
            let sql = sql.trim();
            let status = if sql.is_empty() {
                "Enter SQL to execute".to_string()
            } else {
                format!("Prepared query ({} chars)", sql.len())
            };
            app.set_status_text(status.into());
        }
    });

    app.run()
}
