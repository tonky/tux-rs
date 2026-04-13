//! Init system service file tests.
//!
//! Validates that service files in `dist/` are well-formed and contain all
//! required directives. These tests catch regressions when adding or modifying
//! service files for new init systems.

use std::collections::HashMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Returns the workspace root (parent of tux-daemon/).
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

// ---------------------------------------------------------------------------
// systemd
// ---------------------------------------------------------------------------

fn parse_systemd_unit(content: &str) -> HashMap<String, HashMap<String, String>> {
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current_section = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].to_string();
            sections.entry(current_section.clone()).or_default();
        } else if let Some((key, value)) = line.split_once('=') {
            sections
                .entry(current_section.clone())
                .or_default()
                .insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    sections
}

#[test]
fn systemd_service_file_exists() {
    let path = workspace_root().join("dist/tux-daemon.service");
    assert!(path.exists(), "dist/tux-daemon.service must exist");
}

#[test]
fn systemd_service_has_required_fields() {
    let path = workspace_root().join("dist/tux-daemon.service");
    let content = fs::read_to_string(&path).expect("failed to read systemd service file");
    let unit = parse_systemd_unit(&content);

    // [Unit] section
    let unit_section = unit.get("Unit").expect("missing [Unit] section");
    assert!(
        unit_section.contains_key("Description"),
        "missing Description in [Unit]"
    );
    assert!(
        unit_section.contains_key("After"),
        "missing After in [Unit] — daemon needs dbus"
    );

    // [Service] section
    let service = unit.get("Service").expect("missing [Service] section");
    assert_eq!(
        service.get("Type").map(|s| s.as_str()),
        Some("simple"),
        "daemon runs in foreground, Type must be 'simple'"
    );
    assert!(
        service.contains_key("ExecStart"),
        "missing ExecStart in [Service]"
    );
    assert!(
        service.contains_key("Restart"),
        "missing Restart in [Service]"
    );

    // [Install] section
    let install = unit.get("Install").expect("missing [Install] section");
    assert!(
        install.contains_key("WantedBy"),
        "missing WantedBy in [Install]"
    );
}

#[test]
fn systemd_service_execstart_points_to_daemon() {
    let path = workspace_root().join("dist/tux-daemon.service");
    let content = fs::read_to_string(&path).unwrap();
    let unit = parse_systemd_unit(&content);

    let exec = unit["Service"].get("ExecStart").expect("missing ExecStart");
    assert!(
        exec.contains("tux-daemon"),
        "ExecStart should reference tux-daemon binary, got: {exec}"
    );
}

// ---------------------------------------------------------------------------
// Dinit
// ---------------------------------------------------------------------------

fn parse_dinit_service(content: &str) -> HashMap<String, String> {
    let mut props = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            props.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    props
}

#[test]
fn dinit_service_file_exists() {
    let path = workspace_root().join("dist/tux-daemon.dinit");
    assert!(path.exists(), "dist/tux-daemon.dinit must exist");
}

#[test]
fn dinit_service_has_required_fields() {
    let path = workspace_root().join("dist/tux-daemon.dinit");
    let content = fs::read_to_string(&path).expect("failed to read dinit service file");
    let props = parse_dinit_service(&content);

    assert_eq!(
        props.get("type").map(|s| s.as_str()),
        Some("process"),
        "Dinit type must be 'process' (foreground daemon)"
    );
    assert!(
        props.contains_key("command"),
        "missing 'command' — dinit needs to know what to run"
    );
    assert!(
        props.contains_key("depends-on"),
        "missing 'depends-on' — daemon requires dbus"
    );
    assert_eq!(
        props.get("restart").map(|s| s.as_str()),
        Some("true"),
        "restart should be enabled"
    );
}

#[test]
fn dinit_service_command_points_to_daemon() {
    let path = workspace_root().join("dist/tux-daemon.dinit");
    let content = fs::read_to_string(&path).unwrap();
    let props = parse_dinit_service(&content);

    let cmd = props.get("command").expect("missing command");
    assert!(
        cmd.contains("tux-daemon"),
        "command should reference tux-daemon binary, got: {cmd}"
    );
}

#[test]
fn dinit_service_depends_on_dbus() {
    let path = workspace_root().join("dist/tux-daemon.dinit");
    let content = fs::read_to_string(&path).unwrap();
    let props = parse_dinit_service(&content);

    let dep = props.get("depends-on").expect("missing depends-on");
    assert!(
        dep.contains("dbus"),
        "dinit service should depend on dbus, got: {dep}"
    );
}

// ---------------------------------------------------------------------------
// runit
// ---------------------------------------------------------------------------

fn parse_runit_exec_line(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("exec ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

#[test]
fn runit_service_files_exist() {
    let root = workspace_root().join("dist/tux-daemon.runit");
    assert!(root.exists(), "dist/tux-daemon.runit must exist");
    assert!(
        root.join("run").exists(),
        "dist/tux-daemon.runit/run must exist"
    );
    assert!(
        root.join("finish").exists(),
        "dist/tux-daemon.runit/finish must exist"
    );
}

#[test]
fn runit_run_script_execs_daemon() {
    let path = workspace_root().join("dist/tux-daemon.runit/run");
    let content = fs::read_to_string(&path).expect("failed to read runit run script");

    assert!(
        content.lines().next().unwrap_or("").starts_with("#!"),
        "runit run script should start with a shebang"
    );

    let exec = parse_runit_exec_line(&content).expect("missing exec line in runit run script");
    assert!(
        exec.contains("tux-daemon"),
        "runit run script should exec tux-daemon, got: {exec}"
    );

    assert!(
        content.contains("/run/dbus/system_bus_socket"),
        "runit run script should guard startup on dbus socket readiness"
    );
}

#[test]
fn runit_finish_script_is_non_empty() {
    let path = workspace_root().join("dist/tux-daemon.runit/finish");
    let content = fs::read_to_string(&path).expect("failed to read runit finish script");
    assert!(
        !content.trim().is_empty(),
        "runit finish script should not be empty"
    );
}

#[cfg(unix)]
#[test]
fn runit_service_scripts_are_executable() {
    let run_path = workspace_root().join("dist/tux-daemon.runit/run");
    let finish_path = workspace_root().join("dist/tux-daemon.runit/finish");

    let run_mode = fs::metadata(run_path).unwrap().permissions().mode();
    let finish_mode = fs::metadata(finish_path).unwrap().permissions().mode();

    assert_ne!(run_mode & 0o111, 0, "runit run script must be executable");
    assert_ne!(
        finish_mode & 0o111,
        0,
        "runit finish script must be executable"
    );
}

// ---------------------------------------------------------------------------
// Cross-init consistency
// ---------------------------------------------------------------------------

#[test]
fn all_init_services_reference_same_binary_path() {
    let root = workspace_root().join("dist");

    let systemd_content = fs::read_to_string(root.join("tux-daemon.service")).unwrap();
    let systemd = parse_systemd_unit(&systemd_content);
    let systemd_exec = systemd["Service"]["ExecStart"].as_str();

    let dinit_content = fs::read_to_string(root.join("tux-daemon.dinit")).unwrap();
    let dinit = parse_dinit_service(&dinit_content);
    let dinit_cmd = dinit["command"].as_str();

    let runit_content = fs::read_to_string(root.join("tux-daemon.runit/run")).unwrap();
    let runit_cmd = parse_runit_exec_line(&runit_content).expect("missing runit exec line");

    assert_eq!(
        systemd_exec, dinit_cmd,
        "all init system service files should use the same binary path"
    );
    assert_eq!(
        systemd_exec, runit_cmd,
        "all init system service files should use the same binary path"
    );
}

#[test]
fn all_init_services_depend_on_dbus() {
    let root = workspace_root().join("dist");

    let systemd_content = fs::read_to_string(root.join("tux-daemon.service")).unwrap();
    let systemd = parse_systemd_unit(&systemd_content);
    let systemd_deps = format!(
        "{} {}",
        systemd["Unit"].get("After").unwrap_or(&String::new()),
        systemd["Unit"].get("Requires").unwrap_or(&String::new()),
    );
    assert!(
        systemd_deps.contains("dbus"),
        "systemd service must depend on dbus"
    );

    let dinit_content = fs::read_to_string(root.join("tux-daemon.dinit")).unwrap();
    let dinit = parse_dinit_service(&dinit_content);
    assert!(
        dinit
            .get("depends-on")
            .map(|v| v.contains("dbus"))
            .unwrap_or(false),
        "dinit service must depend on dbus"
    );

    // Runit has no declarative dependency graph; the run script guards
    // startup on the dbus system bus socket to reduce ordering races.
    let runit_content = fs::read_to_string(root.join("tux-daemon.runit/run")).unwrap();
    assert!(
        runit_content.contains("/run/dbus/system_bus_socket"),
        "runit run script should gate startup on dbus socket readiness"
    );
}

// ---------------------------------------------------------------------------
// Feature gate sanity
// ---------------------------------------------------------------------------

/// When the `systemd` feature is enabled, sd-notify should be available.
#[cfg(feature = "systemd")]
#[test]
fn systemd_feature_enables_sd_notify() {
    // If this compiles, the sd-notify crate is linked.
    let _ = sd_notify::NotifyState::Ready;
}

/// When the `systemd` feature is disabled, this module should still compile
/// without any sd-notify references. This test exists as a compile-time
/// assertion — if it compiles, the feature gate is working.
#[cfg(not(feature = "systemd"))]
#[test]
fn no_systemd_feature_compiles_without_sd_notify() {
    // Intentionally empty: the fact that this test file compiles without
    // the systemd feature proves that no ungated sd-notify usage exists
    // in the test binary's dependency graph.
}
