use std::{fs, path::Path, process::Command};

use flows::{Autonomy, Flow};
use indoc::indoc;

const FLOW: &str = indoc! {r#"
    initial_state = "Design"
    [[states]]
    name = "Design"
    autonomy = "steered"
    next = ["Build"]
    [[states]]
    name = "Build"
    autonomy = "autonomous"
    next = ["Done"]
    [[states]]
    name = "Done"
"#};

fn run_cli(root: &Path, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_flows"))
        .args(args)
        .current_dir(root)
        .env("XDG_STATE_HOME", root.join("global"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn setup() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir_all(directory.path().join(".flows/instances")).unwrap();
    fs::write(directory.path().join(".flows/workflow.toml"), FLOW).unwrap();
    directory
}

fn saved_instance(root: &Path) -> toml::Value {
    toml::from_str(&fs::read_to_string(root.join(".flows/instances/run.toml")).unwrap()).unwrap()
}

#[test]
fn parses_all_autonomy_levels_and_defaults_to_guided() {
    let flow: Flow = toml::from_str(FLOW).unwrap();
    assert_eq!(flow.states()[0].autonomy, Autonomy::Steered);
    assert_eq!(flow.states()[1].autonomy, Autonomy::Autonomous);
    assert_eq!(flow.states()[2].autonomy, Autonomy::Guided);

    let flow: Flow = toml::from_str(&FLOW.replace("steered", "guided")).unwrap();
    assert_eq!(flow.states()[0].autonomy, Autonomy::Guided);
}

#[test]
fn rejects_invalid_autonomy_values() {
    for value in ["\"unsupervised\"", "\"Steered\"", "1", "false"] {
        let contents = FLOW.replace("\"steered\"", value);
        assert!(toml::from_str::<Flow>(&contents).is_err(), "{value}");
    }
}

#[test]
fn start_snapshots_defaults_and_status_uses_them() {
    let directory = setup();
    let root = directory.path();
    let start = run_cli(root, &["start", "workflow", "run"]);
    assert!(start.contains("autonomy: steered ("), "{start}");
    let saved = saved_instance(root);
    assert_eq!(saved["autonomy"]["Design"].as_str(), Some("steered"));
    assert_eq!(saved["autonomy"]["Build"].as_str(), Some("autonomous"));
    assert_eq!(saved["autonomy"]["Done"].as_str(), Some("guided"));

    let preview = run_cli(root, &["status", "run", "Build"]);
    assert!(preview.contains("autonomy: autonomous ("), "{preview}");
    assert_eq!(saved_instance(root), saved);
    let moved = run_cli(root, &["next", "run", "Build"]);
    assert!(moved.contains("autonomy: autonomous ("), "{moved}");
}

#[test]
fn legacy_missing_entries_use_defaults_without_overwriting_saved_values() {
    for autonomy in ["", "\n[autonomy]\nDesign = 'guided'\n"] {
        let directory = setup();
        let root = directory.path();
        let path = root.join(".flows/instances/run.toml");
        let original = format!("flow = 'workflow'\nstate = 'Design'\n{autonomy}");
        fs::write(&path, &original).unwrap();

        let expected = if autonomy.is_empty() { "steered" } else { "guided" };
        let status = run_cli(root, &["status", "run"]);
        assert!(status.contains(&format!("autonomy: {expected} (")), "{status}");
        let preview = run_cli(root, &["status", "run", "Build"]);
        assert!(preview.contains("autonomy: autonomous ("), "{preview}");
        assert_eq!(fs::read_to_string(&path).unwrap(), original);

        run_cli(root, &["context", "set", "run", "goal", "finish"]);
        let saved = saved_instance(root);
        assert_eq!(saved["autonomy"]["Design"].as_str(), Some(expected));
        assert_eq!(saved["autonomy"]["Build"].as_str(), Some("autonomous"));
        assert_eq!(saved["autonomy"]["Done"].as_str(), Some("guided"));
    }
}

#[test]
fn changed_flow_defaults_preserve_snapshots_and_explicit_overrides() {
    let directory = setup();
    let root = directory.path();
    run_cli(root, &["start", "workflow", "run"]);
    run_cli(root, &["autonomy", "set", "run", "guided", "Build"]);
    let saved = saved_instance(root);

    fs::write(
        root.join(".flows/workflow.toml"),
        FLOW.replace("steered", "autonomous"),
    )
    .unwrap();
    let status = run_cli(root, &["status", "run"]);
    assert!(status.contains("autonomy: steered ("), "{status}");
    let preview = run_cli(root, &["status", "run", "Build"]);
    assert!(preview.contains("autonomy: guided ("), "{preview}");
    assert_eq!(saved_instance(root), saved);

    run_cli(root, &["context", "set", "run", "goal", "finish"]);
    assert_eq!(saved_instance(root)["autonomy"], saved["autonomy"]);
    let overridden = run_cli(root, &["autonomy", "set", "run", "guided", "Design"]);
    assert!(overridden.contains("set autonomy to guided"), "{overridden}");
    let status = run_cli(root, &["status", "run"]);
    assert!(status.contains("autonomy: guided ("), "{status}");
}
