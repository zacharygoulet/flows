use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use indoc::indoc;

fn load_with_flow(flow_contents: Option<&str>) -> Output {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::create_dir_all(root.join(".flows/instances")).unwrap();
    fs::write(
        root.join(".flows/instances/run.toml"),
        indoc! {r#"
            flow = "workflow"
            state = "Design"
        "#},
    )
    .unwrap();

    if let Some(contents) = flow_contents {
        fs::create_dir_all(root.join(".flows")).unwrap();
        fs::write(root.join(".flows/workflow.toml"), contents).unwrap();
    }

    run_cli(root, &["status", "run"])
}

fn run_cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flows"))
        .args(args)
        .current_dir(root)
        .env("XDG_STATE_HOME", root.join("global"))
        .output()
        .unwrap()
}

fn write_local_flow(root: &Path) {
    write_flow(&root.join(".flows/workflow.toml"));
}

fn write_global_flow(root: &Path) {
    write_flow(&root.join("global/flows/workflow.toml"));
}

fn write_flow(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        indoc! {r#"
            initial_state = "Design"
            [[states]]
            name = "Design"
            next = []
        "#},
    )
    .unwrap();
}

fn write_movable_flow(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        indoc! {r#"
            initial_state = "Design"
            [[states]]
            name = "Design"
            next = ["Review"]
            [[states]]
            name = "Review"
            next = []
            [[states]]
            name = "Done"
            next = []
        "#},
    )
    .unwrap();
}

#[test]
fn rename_moves_local_instance_and_preserves_its_data() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    assert!(
        run_cli(root, &["start", "workflow", "run", "--owner", "agent"])
            .status
            .success()
    );
    assert!(
        run_cli(root, &["context", "set", "run", "goal", "finish"])
            .status
            .success()
    );

    let renamed = run_cli(root, &["rename", "run", "paused-review"]);
    assert!(
        renamed.status.success(),
        "{}",
        String::from_utf8_lossy(&renamed.stderr)
    );
    assert!(!root.join(".flows/instances/run.toml").exists());
    let status = run_cli(root, &["status", "paused-review"]);
    assert!(status.status.success());
    let text = String::from_utf8_lossy(&status.stdout);
    assert!(text.contains("agent"), "{text}");
    assert!(text.contains("goal = "), "{text}");
}

#[test]
fn help_explains_flow_then_lists_clap_commands() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let help = run_cli(root, &["help"]);
    assert!(help.status.success(), "{}", String::from_utf8_lossy(&help.stderr));
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(
        text.starts_with("The `flows` tool helps AI agents follow configured, deterministic flows."),
        "{text}"
    );
    assert!(
        text.find("Autonomous autonomy works independently").unwrap() < text.find("Usage: flows").unwrap()
    );
    for command in [
        "start",
        "next",
        "jump",
        "status",
        "list",
        "context",
        "rename",
        "autonomy",
        "new-flow-from-template",
    ] {
        assert!(text.contains(command), "{text}");
    }
    assert!(text.contains("Use 'flows help <command>'"), "{text}");

    let flag = run_cli(root, &["--help"]);
    assert!(flag.status.success());
    assert_eq!(help.stdout, flag.stdout);
}

#[test]
fn help_for_command_shows_clap_generated_arguments() {
    let directory = tempfile::tempdir().unwrap();
    let help = run_cli(directory.path(), &["help", "autonomy", "set"]);
    assert!(help.status.success(), "{}", String::from_utf8_lossy(&help.stderr));
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(
        text.contains("Usage: flows autonomy set <INSTANCE_NAME> <LEVEL> <STATE> [END_STATE]"),
        "{text}"
    );
    assert!(
        text.contains("Inclusive end state; omit to change only the first state"),
        "{text}"
    );
    assert!(text.contains("guided, steered, or autonomous"), "{text}");
}

#[test]
fn loads_instance_with_separate_flow_file() {
    let output = load_with_flow(Some(indoc! {r#"
            initial_state = "Design"
            [[states]]
            name = "Design"
            next = []
        "#}));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let status = String::from_utf8_lossy(&output.stdout);
    assert!(
        status.starts_with("Instance Status\n\nflow: workflow\ncurrent state: Design\nautonomy: guided ("),
        "{status}"
    );
    assert!(
        status.contains("Global steps:\n- Drive the flow proactively"),
        "{status}"
    );
}

#[test]
fn reports_missing_flow_from_instance_deserialization() {
    let output = load_with_flow(None);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains(r#"flow "workflow" not found"#), "{error}");
    assert!(error.contains(".flows/workflow.toml"), "{error}");
}

#[test]
fn rejects_legacy_instance_key() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    fs::create_dir_all(root.join(".flows/instances")).unwrap();
    fs::write(
        root.join(".flows/instances/run.toml"),
        "definition = 'workflow'\nstate = 'Design'\n",
    )
    .unwrap();

    let output = run_cli(root, &["status", "run"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing field `flow`"));
}

#[test]
fn rejects_legacy_flow_directory() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_flow(&root.join(".flows/definitions/workflow.toml"));

    let output = run_cli(root, &["start", "workflow", "run"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("flow \"workflow\" not found"));
}

#[test]
fn reports_invalid_flow_from_instance_deserialization() {
    let output = load_with_flow(Some(indoc! {r#"
            initial_state = "Design"
            [[states]]
            name = "Design"
            next = ["Missing"]
        "#}));
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("unknown next state Missing"), "{error}");
    assert!(error.contains(".flows/workflow.toml"), "{error}");
}

#[test]
fn new_flow_from_template_creates_a_valid_local_flow() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();

    let created = run_cli(root, &["new-flow-from-template", "workflow"]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let flow_path = root.join(".flows/workflow.toml");
    assert_eq!(
        fs::read_to_string(&flow_path).unwrap(),
        include_str!("../src/assets/flow.toml")
    );

    let started = run_cli(root, &["start", "workflow", "run"]);
    assert!(
        started.status.success(),
        "{}",
        String::from_utf8_lossy(&started.stderr)
    );
    assert!(String::from_utf8_lossy(&started.stdout).contains("created instance run at"));
}

#[test]
fn new_flow_from_template_supports_global_scope_without_overwriting() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();

    let created = run_cli(root, &["new-flow-from-template", "workflow", "-g"]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let flow_path = root.join("global/flows/workflow.toml");
    let original = fs::read_to_string(&flow_path).unwrap();
    assert!(!root.join(".flows/workflow.toml").exists());

    let duplicate = run_cli(root, &["new-flow-from-template", "workflow", "-g"]);
    assert!(!duplicate.status.success());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("already exists"));
    assert_eq!(fs::read_to_string(flow_path).unwrap(), original);

    let started = run_cli(root, &["start", "workflow", "run", "-g"]);
    assert!(
        started.status.success(),
        "{}",
        String::from_utf8_lossy(&started.stderr)
    );
}

#[test]
fn new_creates_local_instance_that_can_be_loaded() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);

    let output = run_cli(root, &["start", "workflow", "run"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("created instance run at"));
    let instance_path = root.join(".flows/instances/run.toml");
    assert_eq!(
        fs::read_to_string(&instance_path).unwrap(),
        indoc! {r#"
            flow = "workflow"
            state = "Design"

            [autonomy]
            Design = "guided"
        "#}
    );

    let loaded = run_cli(root, &["status", "run"]);
    assert!(
        loaded.status.success(),
        "{}",
        String::from_utf8_lossy(&loaded.stderr)
    );
    assert!(String::from_utf8_lossy(&loaded.stdout).contains("state: Design"));
}

#[test]
fn status_lists_local_and_global_instances_in_name_order() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    write_global_flow(root);
    assert!(
        run_cli(root, &["start", "workflow", "local-run"])
            .status
            .success()
    );
    assert!(
        run_cli(root, &["start", "workflow", "global-run", "-g"])
            .status
            .success()
    );

    let status = run_cli(root, &["status"]);
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&status.stdout),
        "INSTANCE    FLOW      OWNER    STATE\n----------  --------  -------  -----\nglobal-run  workflow  unowned  Design\nlocal-run   workflow  unowned  Design\n"
    );
}

#[test]
fn lists_no_flows_when_storage_is_empty() {
    let directory = tempfile::tempdir().unwrap();
    let output = run_cli(directory.path(), &["list"]);
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "no flows found\n");
}

#[test]
fn lists_flows_with_summaries_and_local_shadowing() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let local = root.join(".flows/workflow.toml");
    let global = root.join("global/flows/workflow.toml");
    write_flow(&local);
    write_flow(&global);
    fs::write(
        &local,
        "summary = 'Local workflow'\ninitial_state = 'Done'\n[[states]]\nname = 'Done'\n",
    )
    .unwrap();
    fs::write(&global, "invalid TOML = ").unwrap();
    write_flow(&root.join("global/flows/other.toml"));

    let output = run_cli(root, &["list"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "FLOW      SUMMARY\n--------  -------\nother\nworkflow  Local workflow\n"
    );
}

#[test]
fn lists_states_with_summaries_in_flow_order() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let path = root.join(".flows/workflow.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "initial_state = 'Draft'\n[[states]]\nname = 'Draft'\nsummary = 'Start here'\n[[states]]\nname = 'Review'\n").unwrap();

    let output = run_cli(root, &["list", "states", "workflow"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "STATE   SUMMARY\n------  -------\nDraft   Start here\nReview\n"
    );
}

#[test]
fn status_displays_flow_state_and_next_state_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let flow_path = root.join(".flows/workflow.toml");
    fs::create_dir_all(flow_path.parent().unwrap()).unwrap();
    fs::write(
        &flow_path,
        indoc! {r#"
            use_global = false
            summary = "Whole workflow summary"
            details = "Flow-wide details"
            steps = ["Always do this", "Consider this when relevant"]
            initial_state = "Draft"

            [[states]]
            name = "Draft"
            summary = "Prepare the work"
            details = "Current-state details"
            steps = ["Confirm the goal", "Check for existing work"]
            next = ["Review"]

            [[states]]
            name = "Review"
            summary = "Review the work"
            next = []
        "#},
    )
    .unwrap();
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());

    let status = run_cli(root, &["status", "run"]);
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert!(!String::from_utf8_lossy(&status.stdout).contains("global:"));
    assert_eq!(
        String::from_utf8_lossy(&status.stdout),
        indoc! {"\
            Instance Status

            flow: workflow (Whole workflow summary)
            current state: Draft (Prepare the work)
            autonomy: guided (Pause for user approval at state boundaries and significant decisions.)
            owner: unowned

            Flow details:
            Flow-wide details

            State details:
            Current-state details

            Flow steps:
            - Always do this
            - Consider this when relevant

            State steps:
            - Confirm the goal
            - Check for existing work

            Next states:
              - Review: Review the work
        "}
    );
}

#[test]
fn status_reports_when_no_instances_exist() {
    let directory = tempfile::tempdir().unwrap();
    let status = run_cli(directory.path(), &["status"]);
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&status.stdout), "no instances found\n");
}

#[test]
fn next_and_jump_persist_local_instance_state() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flows/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());

    let next = run_cli(root, &["next", "run", "Review"]);
    assert!(next.status.success(), "{}", String::from_utf8_lossy(&next.stderr));
    let loaded = run_cli(root, &["status", "run"]);
    assert!(loaded.status.success());
    assert!(String::from_utf8_lossy(&loaded.stdout).contains("state: Review"));

    let jump = run_cli(root, &["jump", "run", "Done"]);
    assert!(jump.status.success(), "{}", String::from_utf8_lossy(&jump.stderr));
    assert_eq!(
        fs::read_to_string(root.join(".flows/instances/run.toml")).unwrap(),
        indoc! {r#"
            flow = "workflow"
            state = "Done"

            [autonomy]
            Design = "guided"
            Done = "guided"
            Review = "guided"
        "#}
    );
}

#[test]
fn context_updates_survive_moves_and_appear_in_status() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flows/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    let path = root.join(".flows/instances/run.toml");
    assert!(!fs::read_to_string(&path).unwrap().contains("[context]"));

    for args in [
        ["context", "set", "run", "decision", "draft"],
        ["context", "set", "run", "review notes", "line 1\nline 2"],
        ["context", "set", "run", "decision", "approved"],
    ] {
        let output = run_cli(root, &args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let saved = fs::read_to_string(&path).unwrap();
    let document: toml::Value = toml::from_str(&saved).unwrap();
    assert_eq!(document["context"]["decision"].as_str(), Some("approved"));
    assert_eq!(
        document["context"]["review notes"].as_str(),
        Some("line 1\nline 2")
    );

    let status = run_cli(root, &["status", "run"]);
    assert!(status.status.success());
    let output = String::from_utf8_lossy(&status.stdout);
    assert!(output.contains("context:\n"), "{output}");
    assert!(output.contains("decision = \"approved\""), "{output}");
    assert!(output.contains("review notes"), "{output}");

    assert!(run_cli(root, &["next", "run", "Review"]).status.success());
    let moved: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(moved["context"], document["context"]);

    assert!(
        run_cli(root, &["context", "remove", "run", "decision"])
            .status
            .success()
    );
    let saved: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(saved["context"].get("decision").is_none());
    assert_eq!(saved["context"]["review notes"].as_str(), Some("line 1\nline 2"));
}

#[test]
fn context_values_are_limited_to_150_characters() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flows/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());

    let exact_limit = "a".repeat(150);
    let accepted = run_cli(root, &["context", "set", "run", "plan", &exact_limit]);
    assert!(accepted.status.success());

    let too_long = "a".repeat(151);
    let rejected = run_cli(root, &["context", "set", "run", "plan", &too_long]);
    assert!(!rejected.status.success());
    let error = String::from_utf8_lossy(&rejected.stderr);
    assert!(error.contains("at most 150 characters"), "{error}");
    assert!(error.contains("compact reminders and pointers"), "{error}");

    let saved: toml::Value =
        toml::from_str(&fs::read_to_string(root.join(".flows/instances/run.toml")).unwrap()).unwrap();
    assert_eq!(saved["context"]["plan"].as_str(), Some(exact_limit.as_str()));
}

#[test]
fn autonomy_range_is_persisted_per_state_and_follows_moves() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flows/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());

    let range = run_cli(root, &["autonomy", "set", "run", "steered", "Design", "Review"]);
    assert!(
        range.status.success(),
        "{}",
        String::from_utf8_lossy(&range.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&range.stdout),
        "set autonomy to steered for states:\n  - Design\n  - Review\n"
    );
    let single = run_cli(root, &["autonomy", "set", "run", "autonomous", "Review"]);
    assert!(
        single.status.success(),
        "{}",
        String::from_utf8_lossy(&single.stderr)
    );

    assert_eq!(
        String::from_utf8_lossy(&single.stdout),
        "set autonomy to autonomous for states:\n  - Review\n"
    );
    let path = root.join(".flows/instances/run.toml");
    let saved: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["autonomy"]["Design"].as_str(), Some("steered"));
    assert_eq!(saved["autonomy"]["Review"].as_str(), Some("autonomous"));
    assert_eq!(saved["autonomy"]["Done"].as_str(), Some("guided"));

    let status = run_cli(root, &["status", "run"]);
    assert!(String::from_utf8_lossy(&status.stdout).contains("current state: Design\nautonomy: steered ("));
    assert!(run_cli(root, &["next", "run", "Review"]).status.success());
    let status = run_cli(root, &["status", "run"]);
    let output = String::from_utf8_lossy(&status.stdout);
    assert!(
        output.contains("current state: Review\nautonomy: autonomous ("),
        "{output}"
    );
    assert!(
        output.contains("Continue independently; stop only for fundamental blockers."),
        "{output}"
    );
    assert!(run_cli(root, &["jump", "run", "Done"]).status.success());
    assert!(
        String::from_utf8_lossy(&run_cli(root, &["status", "run"]).stdout)
            .contains("current state: Done\nautonomy: guided (")
    );
}

#[test]
fn autonomy_range_includes_all_routes_and_loops_before_end_but_stops_at_end() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let path = root.join(".flows/workflow.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        indoc! {r#"
        initial_state = "Start"
        [[states]]
        name = "End"
        next = ["After"]
        [[states]]
        name = "Right"
        next = ["End"]
        [[states]]
        name = "Start"
        next = ["Left", "Right", "Dead End"]
        [[states]]
        name = "Dead End"
        next = ["Dead End"]
        [[states]]
        name = "Loop"
        next = ["Left"]
        [[states]]
        name = "After"
        next = ["End"]
        [[states]]
        name = "Left"
        next = ["Loop", "End"]
        [[states]]
        name = "Unrelated"
    "#},
    )
    .unwrap();
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());

    let range = run_cli(root, &["autonomy", "set", "run", "steered", "Start", "End"]);
    assert!(
        range.status.success(),
        "{}",
        String::from_utf8_lossy(&range.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&range.stdout),
        "set autonomy to steered for states:\n  - End\n  - Right\n  - Start\n  - Loop\n  - Left\n"
    );
    let saved: toml::Value =
        toml::from_str(&fs::read_to_string(root.join(".flows/instances/run.toml")).unwrap()).unwrap();
    for name in ["End", "Right", "Start", "Loop", "Left"] {
        assert_eq!(saved["autonomy"][name].as_str(), Some("steered"), "{name}");
    }
    for name in ["Dead End", "After", "Unrelated"] {
        assert_eq!(saved["autonomy"][name].as_str(), Some("guided"), "{name}");
    }

    let same = run_cli(root, &["autonomy", "set", "run", "autonomous", "End", "End"]);
    assert_eq!(
        String::from_utf8_lossy(&same.stdout),
        "set autonomy to autonomous for states:\n  - End\n"
    );
    let original = fs::read_to_string(root.join(".flows/instances/run.toml")).unwrap();
    let unreachable = run_cli(root, &["autonomy", "set", "run", "steered", "Start", "Unrelated"]);
    assert!(!unreachable.status.success());
    assert!(String::from_utf8_lossy(&unreachable.stderr).contains("no next path"));
    assert_eq!(
        fs::read_to_string(root.join(".flows/instances/run.toml")).unwrap(),
        original
    );
}

#[test]
fn autonomy_rejects_invalid_ranges_without_changing_instance() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flows/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    let path = root.join(".flows/instances/run.toml");
    let original = fs::read_to_string(&path).unwrap();

    for (args, message) in [
        (
            vec!["autonomy", "set", "run", "steered", "Done", "Design"],
            "no next path",
        ),
        (
            vec!["autonomy", "set", "run", "steered", "Missing"],
            "unknown state",
        ),
        (
            vec!["autonomy", "set", "run", "steered", "Design", "Missing"],
            "unknown state",
        ),
        (
            vec!["autonomy", "set", "run", "unsupervised", "Design"],
            "Matching variant not found",
        ),
    ] {
        let output = run_cli(root, &args);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(message));
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }
}

#[test]
fn autonomy_updates_global_instance_and_handles_state_names_with_spaces() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let flow = root.join("global/flows/workflow.toml");
    fs::create_dir_all(flow.parent().unwrap()).unwrap();
    fs::write(
        &flow,
        "initial_state = 'In Progress'\n[[states]]\nname = 'In Progress'\n[[states]]\nname = 'Done'\n",
    )
    .unwrap();
    assert!(
        run_cli(root, &["start", "workflow", "run", "-g"])
            .status
            .success()
    );

    let set = run_cli(root, &["autonomy", "set", "run", "steered", "In Progress"]);
    assert!(set.status.success(), "{}", String::from_utf8_lossy(&set.stderr));
    assert!(!root.join(".flows/instances/run.toml").exists());
    let saved = fs::read_to_string(root.join("global/flows/instances/run.toml")).unwrap();
    let document: toml::Value = toml::from_str(&saved).unwrap();
    assert_eq!(document["autonomy"]["In Progress"].as_str(), Some("steered"));
    assert!(
        String::from_utf8_lossy(&run_cli(root, &["status", "run"]).stdout)
            .contains("current state: In Progress\nautonomy: steered (")
    );
}

#[test]
fn legacy_instances_gain_guided_autonomy_for_every_state() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flows/workflow.toml"));
    let path = root.join(".flows/instances/run.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "flow = 'workflow'\nstate = 'Design'\n").unwrap();

    let status = run_cli(root, &["status", "run"]);
    assert!(status.status.success());
    assert!(String::from_utf8_lossy(&status.stdout).contains("autonomy: guided"));
    assert!(
        run_cli(root, &["autonomy", "set", "run", "steered", "Review"])
            .status
            .success()
    );
    let saved: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["autonomy"]["Design"].as_str(), Some("guided"));
    assert_eq!(saved["autonomy"]["Review"].as_str(), Some("steered"));
    assert_eq!(saved["autonomy"]["Done"].as_str(), Some("guided"));
}

#[test]
fn removing_missing_context_does_not_change_instance() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    let path = root.join(".flows/instances/run.toml");
    let original = fs::read_to_string(&path).unwrap();

    let result = run_cli(root, &["context", "remove", "run", "missing"]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("context key \"missing\" not found"));
    assert_eq!(fs::read_to_string(path).unwrap(), original);
}

#[test]
fn context_updates_global_instance_in_place() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_global_flow(root);
    assert!(
        run_cli(root, &["start", "workflow", "run", "-g"])
            .status
            .success()
    );

    let result = run_cli(root, &["context", "set", "run", "goal", "finish"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!root.join(".flows/instances/run.toml").exists());
    let saved = fs::read_to_string(root.join("global/flows/instances/run.toml")).unwrap();
    let document: toml::Value = toml::from_str(&saved).unwrap();
    assert_eq!(document["context"]["goal"].as_str(), Some("finish"));
}

#[test]
fn rejected_moves_leave_instance_file_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flows/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    let path = root.join(".flows/instances/run.toml");
    let original = fs::read_to_string(&path).unwrap();

    let next = run_cli(root, &["next", "run", "Done"]);
    assert!(!next.status.success());
    assert!(String::from_utf8_lossy(&next.stderr).contains("cannot move next"));
    let jump = run_cli(root, &["jump", "run", "Missing"]);
    assert!(!jump.status.success());
    assert!(String::from_utf8_lossy(&jump.stderr).contains("unknown state"));
    assert_eq!(fs::read_to_string(path).unwrap(), original);
}

#[test]
fn movement_updates_global_instance_without_creating_a_local_one() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join("global/flows/workflow.toml"));
    assert!(
        run_cli(root, &["start", "workflow", "run", "-g"])
            .status
            .success()
    );

    let next = run_cli(root, &["next", "run", "Review"]);
    assert!(next.status.success(), "{}", String::from_utf8_lossy(&next.stderr));
    assert!(!root.join(".flows/instances/run.toml").exists());
    assert!(
        fs::read_to_string(root.join("global/flows/instances/run.toml"))
            .unwrap()
            .contains("state = \"Review\"")
    );
}

#[test]
fn new_rejects_collision_in_global_scope() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    let existing = root.join("global/flows/instances/run.toml");
    fs::create_dir_all(existing.parent().unwrap()).unwrap();
    fs::write(&existing, "original").unwrap();

    let output = run_cli(root, &["start", "workflow", "run"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert!(!root.join(".flows/instances/run.toml").exists());
    assert_eq!(fs::read_to_string(existing).unwrap(), "original");
}

#[test]
fn new_requires_flow_before_creating_instance_file() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();

    let output = run_cli(root, &["start", "workflow", "run"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("flow \"workflow\" not found"));
    assert!(!root.join(".flows/instances").exists());
}

#[test]
fn new_generates_flow_prefixed_instance_name_that_can_be_loaded() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);

    let created = run_cli(root, &["start", "workflow"]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let output = String::from_utf8_lossy(&created.stdout);
    let name = output
        .strip_prefix("created instance ")
        .unwrap()
        .split_once(" at ")
        .unwrap()
        .0;
    let generated_id = name.strip_prefix("workflow-").unwrap();
    assert_eq!(generated_id.len(), 32);
    assert!(
        generated_id
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    );
    assert!(
        root.join(".flows/instances")
            .join(format!("{name}.toml"))
            .is_file()
    );

    let loaded = run_cli(root, &["status", name]);
    assert!(
        loaded.status.success(),
        "{}",
        String::from_utf8_lossy(&loaded.stderr)
    );
    assert!(String::from_utf8_lossy(&loaded.stdout).contains("Instance Status\n\nflow: workflow\n"));
}

#[test]
fn new_global_creates_instance_with_global_flow() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_global_flow(root);

    let output = run_cli(root, &["start", "workflow", "run", "-g"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("global/flows/instances/run.toml")).unwrap(),
        indoc! {r#"
            flow = "workflow"
            state = "Design"

            [autonomy]
            Design = "guided"
        "#}
    );
    assert!(!root.join(".flows/instances/run.toml").exists());
    let loaded = run_cli(root, &["status", "run"]);
    assert!(
        loaded.status.success(),
        "{}",
        String::from_utf8_lossy(&loaded.stderr)
    );
    assert!(String::from_utf8_lossy(&loaded.stdout).contains("state: Design"));
}

#[test]
fn new_copy_flow_installs_local_flow_and_creates_global_instance() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    let local_flow = root.join(".flows/workflow.toml");

    let output = run_cli(root, &["start", "workflow", "run", "-G"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("global/flows/workflow.toml")).unwrap(),
        fs::read_to_string(local_flow).unwrap()
    );
    assert!(!root.join(".flows/instances/run.toml").exists());
    let loaded = run_cli(root, &["status", "run"]);
    assert!(
        loaded.status.success(),
        "{}",
        String::from_utf8_lossy(&loaded.stderr)
    );
}

#[test]
fn new_copy_flow_keeps_existing_global_flow() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    let global_flow = root.join("global/flows/workflow.toml");
    fs::create_dir_all(global_flow.parent().unwrap()).unwrap();
    let existing = indoc! {r#"
        initial_state = "Review"
        [[states]]
        name = "Review"
        next = []
    "#};
    fs::write(&global_flow, existing).unwrap();

    let output = run_cli(root, &["start", "workflow", "run", "-G"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(global_flow).unwrap(), existing);
    assert!(root.join("global/flows/instances/run.toml").is_file());
}

#[test]
fn new_copy_flow_checks_instance_collision_before_copy() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    let existing = root.join(".flows/instances/run.toml");
    fs::create_dir_all(existing.parent().unwrap()).unwrap();
    fs::write(&existing, "original").unwrap();

    let output = run_cli(root, &["start", "workflow", "run", "-G"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert!(!root.join("global/flows/workflow.toml").exists());
    assert_eq!(fs::read_to_string(existing).unwrap(), "original");
}

#[test]
fn new_copy_flow_rejects_invalid_existing_global_flow() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    let global_flow = root.join("global/flows/workflow.toml");
    fs::create_dir_all(global_flow.parent().unwrap()).unwrap();
    fs::write(&global_flow, "invalid toml = ").unwrap();

    let output = run_cli(root, &["start", "workflow", "run", "-G"]);
    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(global_flow).unwrap(), "invalid toml = ");
    assert!(!root.join("global/flows/instances/run.toml").exists());
}

#[test]
fn new_global_requires_global_flow_even_if_local_exists() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);

    let output = run_cli(root, &["start", "workflow", "run", "-g"]);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("global flow \"workflow\" not found"), "{error}");
    assert!(error.contains("global/flows/workflow.toml"), "{error}");
    assert!(!root.join("global/flows/instances/run.toml").exists());
}

#[test]
fn ownership_can_be_claimed_released_and_stopped() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);

    let started = run_cli(root, &["start", "workflow", "run", "--owner", "agent-a"]);
    assert!(
        started.status.success(),
        "{}",
        String::from_utf8_lossy(&started.stderr)
    );
    let path = root.join(".flows/instances/run.toml");
    let saved: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["owner"].as_str(), Some("agent-a"));
    assert!(String::from_utf8_lossy(&run_cli(root, &["status"]).stdout).contains("agent-a"));

    assert!(
        run_cli(root, &["resume", "run", "--owner", "agent-a"])
            .status
            .success()
    );
    let rejected = run_cli(root, &["resume", "run", "--owner", "agent-b"]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("owned by \"agent-a\""));

    assert!(run_cli(root, &["pause", "run"]).status.success());
    let saved: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(saved.get("owner").is_none());
    assert!(
        run_cli(root, &["resume", "run", "--owner", "agent-b"])
            .status
            .success()
    );
    assert!(run_cli(root, &["stop", "run"]).status.success());
    assert!(!path.exists());
}

#[test]
fn complete_deletes_an_instance_only_at_a_terminal_state() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    let path = root.join(".flows/instances/run.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "flow = 'workflow'\nstate = 'Design'\n").unwrap();

    let completed = run_cli(root, &["complete", "run"]);
    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stderr)
    );
    assert!(String::from_utf8_lossy(&completed.stdout).contains("completed instance run"));
    assert!(!path.exists());
}

#[test]
fn complete_preserves_an_instance_with_available_next_states() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flows/workflow.toml"));
    let path = root.join(".flows/instances/run.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "flow = 'workflow'\nstate = 'Design'\n").unwrap();

    let completed = run_cli(root, &["complete", "run"]);
    assert!(!completed.status.success());
    assert!(String::from_utf8_lossy(&completed.stderr).contains("state has next transitions"));
    assert!(path.exists());
}

#[test]
fn stop_can_delete_an_instance_when_its_flow_is_missing() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let path = root.join(".flows/instances/run.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "flow = 'missing'\nstate = 'Unknown'\n").unwrap();

    let stopped = run_cli(root, &["stop", "run"]);
    assert!(
        stopped.status.success(),
        "{}",
        String::from_utf8_lossy(&stopped.stderr)
    );
    assert!(!path.exists());
}

#[test]
fn new_global_rejects_invalid_global_flow() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    let global_flow = root.join("global/flows/workflow.toml");
    fs::create_dir_all(global_flow.parent().unwrap()).unwrap();
    fs::write(&global_flow, "invalid toml = ").unwrap();

    let output = run_cli(root, &["start", "workflow", "run", "-g"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("global/flows/workflow.toml"));
    assert!(!root.join("global/flows/instances/run.toml").exists());
}

#[test]
fn instance_configuration_persists_and_controls_status_next_jump_and_complete() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let flow = root.join(".flows/workflow.toml");
    fs::create_dir_all(flow.parent().unwrap()).unwrap();
    fs::write(
        &flow,
        indoc! {r#"
        initial_state = "Start"
        use_global = false
        [[states]]
        name = "Start"
        next = ["A", "B", "Finish"]
        [[states]]
        name = "A"
        next = ["B", "Finish"]
        [[states]]
        name = "B"
        next = ["Finish", "A"]
        [[states]]
        name = "Finish"
        next = []
    "#},
    )
    .unwrap();
    let path = root.join(".flows/instances/run.toml");
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    for args in [&["skip", "run", "A"][..], &["skip", "run", "B"]] {
        let result = run_cli(root, args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let status = run_cli(root, &["status", "run"]);
    let text = String::from_utf8_lossy(&status.stdout);
    assert!(text.contains("Next states:\n  - Finish\n"), "{text}");
    assert!(!run_cli(root, &["next", "run", "A"]).status.success());
    assert!(!run_cli(root, &["jump", "run", "B"]).status.success());
    assert!(run_cli(root, &["next", "run", "Finish"]).status.success());
    assert!(run_cli(root, &["complete", "run"]).status.success());
    assert!(!path.exists());
}

#[test]
fn initial_and_custom_final_are_reversible_and_failed_commands_leave_file_untouched() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flows/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    let path = root.join(".flows/instances/run.toml");
    assert!(run_cli(root, &["initial", "run", "Review"]).status.success());
    let contents = fs::read_to_string(&path).unwrap();
    assert!(contents.contains("initial_state = \"Review\""), "{contents}");
    assert!(contents.contains("state = \"Review\""), "{contents}");
    assert!(run_cli(root, &["jump", "run", "Design"]).status.success());
    assert!(run_cli(root, &["initial", "run", "Review"]).status.success());
    assert!(run_cli(root, &["final", "add", "run", "Review"]).status.success());
    let status = String::from_utf8(run_cli(root, &["status", "run"]).stdout).unwrap();
    assert!(!status.contains("Next states:"), "{status}");
    assert!(
        run_cli(root, &["final", "remove", "run", "Review"])
            .status
            .success()
    );
    assert!(run_cli(root, &["initial", "run", "Design"]).status.success());
    let contents = fs::read_to_string(&path).unwrap();
    assert!(!contents.contains("initial_state"), "{contents}");
    assert!(!contents.contains("final_states"), "{contents}");
    for args in [
        &["skip", "run", "Design"][..],
        &["skip", "run", "Missing"],
        &["initial", "run", "Missing"],
        &["final", "remove", "run", "Review"],
        &["unskip", "run", "Review"],
    ] {
        let before = fs::read(&path).unwrap();
        assert!(!run_cli(root, args).status.success(), "{args:?}");
        assert_eq!(fs::read(&path).unwrap(), before, "{args:?}");
    }
    assert!(run_cli(root, &["final", "add", "run", "Review"]).status.success());
    let before = fs::read(&path).unwrap();
    assert!(!run_cli(root, &["skip", "run", "Review"]).status.success());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(
        run_cli(root, &["final", "remove", "run", "Review"])
            .status
            .success()
    );
    let before = fs::read(&path).unwrap();
    assert!(!run_cli(root, &["skip", "run", "Review"]).status.success());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(run_cli(root, &["initial", "run", "Review"]).status.success());
}

#[test]
fn custom_final_allows_completion_despite_flow_transitions() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flows/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    assert!(run_cli(root, &["final", "add", "run", "Design"]).status.success());
    assert!(run_cli(root, &["complete", "run"]).status.success());
    assert!(!root.join(".flows/instances/run.toml").exists());
}

#[test]
fn skipped_cycle_does_not_make_a_nonfinal_state_completable() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let flow = root.join(".flows/workflow.toml");
    fs::create_dir_all(flow.parent().unwrap()).unwrap();
    fs::write(
        &flow,
        "initial_state = 'Start'\nuse_global = false\n[[states]]\nname = 'Start'\nnext = ['Loop']\n[[states]]\nname = 'Loop'\nnext = ['Loop']\n",
    )
    .unwrap();
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    assert!(run_cli(root, &["skip", "run", "Loop"]).status.success());
    let path = root.join(".flows/instances/run.toml");
    let before = fs::read(&path).unwrap();
    assert!(!run_cli(root, &["complete", "run"]).status.success());
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn invalid_persisted_instance_configuration_is_rejected_on_load() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flows/workflow.toml"));
    let path = root.join(".flows/instances/run.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    for extra in [
        "initial_state = 'Missing'\n",
        "final_states = ['Missing']\n",
        "skipped_states = ['Missing']\n",
        "skipped_states = ['Design']\n",
        "skipped_states = ['Review']\n",
        "skipped_states = ['Review']\nfinal_states = ['Review']\n",
    ] {
        fs::write(&path, format!("flow = 'workflow'\nstate = 'Design'\n{extra}")).unwrap();
        let output = run_cli(root, &["status", "run"]);
        assert!(!output.status.success(), "{extra}");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            format!("flow = 'workflow'\nstate = 'Design'\n{extra}")
        );
    }
}
