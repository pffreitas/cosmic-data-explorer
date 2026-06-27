const APP_SLINT: &str = include_str!("../ui/app.slint");
const BUILD_RS: &str = include_str!("../build.rs");

#[test]
fn workbench_shell_contains_active_connections_sidebar_and_settings_modal() {
    for expected in [
        "active-connections",
        "settings-button",
        "settings-open",
        "Connection Settings",
        "Add Connection",
        "PostgreSQL",
        "SQLite",
    ] {
        assert!(
            APP_SLINT.contains(expected),
            "missing UI contract marker: {expected}"
        );
    }
}

#[test]
fn phase_one_uses_native_widgets_without_fake_system_chrome() {
    assert!(
        BUILD_RS.contains(".with_style(\"native\".into())"),
        "desktop build must explicitly select Slint's native widget style"
    );
    assert!(
        APP_SLINT.contains("macos-titlebar-reserved"),
        "UI should reserve space for native macOS titlebar/chrome instead of drawing it"
    );
    assert!(
        APP_SLINT.contains("macos-material-ready"),
        "UI should mark surfaces that are intended to become native material-backed in phase 2"
    );
    assert!(
        !APP_SLINT.contains("TrafficLight"),
        "Slint should not draw fake macOS traffic-light window controls"
    );
    assert!(
        !APP_SLINT.contains("dot-color: #ff5f57"),
        "Slint should not hard-code fake close/minimize/zoom controls"
    );
}
