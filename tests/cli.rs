use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use indoc::indoc;

fn load_with_flow(flow_contents: Option<&str>) -> Output {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::create_dir_all(root.join(".flow/instances")).unwrap();
    fs::write(
        root.join(".flow/instances/run.toml"),
        indoc! {r#"
            flow = "workflow"
            step = "Design"
        "#},
    )
    .unwrap();

    if let Some(contents) = flow_contents {
        fs::create_dir_all(root.join(".flow")).unwrap();
        fs::write(root.join(".flow/workflow.toml"), contents).unwrap();
    }

    run_cli(root, &["show", "run"])
}

const AUTONOMY_HINT: &str =
    "Interview the user about upcoming decisions when the selected autonomy requires it.\n";

fn run_cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flow"))
        .args(args)
        .current_dir(root)
        .env("XDG_STATE_HOME", root.join("global"))
        .output()
        .unwrap()
}

fn write_local_flow(root: &Path) {
    write_flow(&root.join(".flow/workflow.toml"));
}

fn write_global_flow(root: &Path) {
    write_flow(&root.join("global/flow/workflow.toml"));
}

fn write_flow(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        indoc! {r#"
            initial_step = "Design"
            [[steps]]
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
            initial_step = "Design"
            [[steps]]
            name = "Design"
            next = ["Review"]
            [[steps]]
            name = "Review"
            next = []
            [[steps]]
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
    assert!(!root.join(".flow/instances/run.toml").exists());
    let status = run_cli(root, &["show", "paused-review"]);
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
        text.starts_with("The `flow` tool helps AI agents follow configured, deterministic flows."),
        "{text}"
    );
    assert!(
        text.find("Autonomous autonomy works independently").unwrap() < text.find("Usage: flow").unwrap()
    );
    for command in [
        "start",
        "next",
        "jump",
        "show",
        "list",
        "context",
        "rename",
        "autonomy",
        "route",
        "create-flow",
    ] {
        assert!(text.contains(command), "{text}");
    }
    assert!(text.contains("Use 'flow help <command>'"), "{text}");

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
        text.contains("Usage: flow autonomy set")
            && text.contains("--step <STEP>")
            && text.contains("--from <FROM>")
            && text.contains("--through <THROUGH>"),
        "{text}"
    );
    assert!(text.contains("Last step of the range"), "{text}");
    assert!(text.contains("guided, steered, or autonomous"), "{text}");
}

#[test]
fn status_printing_commands_advertise_their_output() {
    let directory = tempfile::tempdir().unwrap();
    for args in [
        &["help", "start"][..],
        &["help", "claim"],
        &["help", "next"],
        &["help", "jump"],
        &["help", "reset"],
    ] {
        let help = run_cli(directory.path(), args);
        assert!(
            help.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&help.stderr)
        );
        assert!(
            String::from_utf8_lossy(&help.stdout).contains("show"),
            "{args:?}: {}",
            String::from_utf8_lossy(&help.stdout)
        );
        assert!(String::from_utf8_lossy(&help.stdout).contains("status"));
    }
}

#[test]
fn next_requires_an_instance_and_help_documents_optional_target() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let missing = run_cli(root, &["next"]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("<INSTANCE_NAME>"));
    assert!(!root.join(".flow").exists());

    let help = run_cli(root, &["help", "next"]);
    assert!(help.status.success());
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(
        text.contains("Usage: flow next <INSTANCE_NAME> [TARGET]"),
        "{text}"
    );
    assert!(
        text.contains("omit only when exactly one destination is available"),
        "{text}"
    );
}

#[test]
fn status_help_documents_optional_preview_step() {
    let directory = tempfile::tempdir().unwrap();
    let help = run_cli(directory.path(), &["help", "show"]);
    assert!(help.status.success());
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(
        text.contains("Usage: flow show [OPTIONS] <INSTANCE_NAME>") && text.contains("--step <STEP>"),
        "{text}"
    );
    assert!(
        text.contains("Step to inspect without changing the instance"),
        "{text}"
    );
}

#[test]
fn step_overrides_help_shows_nested_commands() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    for (args, usage) in [
        (
            &["help", "route", "initial", "set"][..],
            "Usage: flow route initial set <INSTANCE_NAME> <STEP>",
        ),
        (
            &["help", "route", "final", "add"],
            "Usage: flow route final add <INSTANCE_NAME> <STEP>",
        ),
        (
            &["help", "route", "final", "remove"],
            "Usage: flow route final remove <INSTANCE_NAME> <STEP>",
        ),
        (
            &["help", "route", "skip"],
            "Usage: flow route skip <INSTANCE_NAME> <STEP>",
        ),
        (
            &["help", "route", "unskip"],
            "Usage: flow route unskip <INSTANCE_NAME> <STEP>",
        ),
    ] {
        let output = run_cli(root, args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(usage),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    let top_level = String::from_utf8(run_cli(root, &["help"]).stdout).unwrap();
    for old in ["initial", "final", "skip", "unskip"] {
        assert!(
            !top_level
                .lines()
                .any(|line| line.trim_start().starts_with(&format!("{old} "))),
            "{top_level}"
        );
    }
}

#[test]
fn loads_instance_with_separate_flow_file() {
    let output = load_with_flow(Some(indoc! {r#"
            initial_step = "Design"
            [[steps]]
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
        status.starts_with("Instance Status\n\nflow: workflow\ncurrent step: Design\nautonomy: guided ("),
        "{status}"
    );
    let guidance = flow::GlobalGuidance::bundled();
    let mut expected_global_checklist = String::from("Global checklist:\n");
    for item in guidance.checklist() {
        expected_global_checklist.push_str(&format!("- {item}\n"));
    }
    expected_global_checklist.push('\n');
    assert!(status.contains(&expected_global_checklist), "{status}");
}

#[test]
fn reports_missing_flow_from_instance_deserialization() {
    let output = load_with_flow(None);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains(r#"flow "workflow" not found"#), "{error}");
    assert!(error.contains(".flow/workflow.toml"), "{error}");
}

#[test]
fn rejects_legacy_instance_key() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    fs::create_dir_all(root.join(".flow/instances")).unwrap();
    fs::write(
        root.join(".flow/instances/run.toml"),
        "definition = 'workflow'\nstep = 'Design'\n",
    )
    .unwrap();

    let output = run_cli(root, &["show", "run"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown field `definition`"));
}

#[test]
fn rejects_legacy_step_schema_without_modifying_instances() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    fs::create_dir_all(root.join(".flow/instances")).unwrap();
    let path = root.join(".flow/instances/run.toml");
    for (contents, field) in [
        ("flow = 'workflow'\nstate = 'Design'\n", "state"),
        ("flow = 'workflow'\nstep = 'Design'\nstate = 'Design'\n", "state"),
        (
            "flow = 'workflow'\nstep = 'Design'\n[state_overrides]\nfinals = ['Design']\n",
            "state_overrides",
        ),
    ] {
        fs::write(&path, contents).unwrap();
        let output = run_cli(root, &["show", "run"]);
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(&format!("unknown field `{field}`")), "{error}");
        assert_eq!(fs::read_to_string(&path).unwrap(), contents);
    }
}

#[test]
fn rejects_legacy_step_commands() {
    let directory = tempfile::tempdir().unwrap();
    for args in [&["list", "states"][..], &["state-overrides", "--help"]] {
        let output = run_cli(directory.path(), args);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized subcommand"));
    }
}

#[test]
fn rejects_legacy_flow_directory() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_flow(&root.join(".flow/definitions/workflow.toml"));

    let output = run_cli(root, &["start", "workflow", "run"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("flow \"workflow\" not found"));
}

#[test]
fn reports_invalid_flow_from_instance_deserialization() {
    let output = load_with_flow(Some(indoc! {r#"
            initial_step = "Design"
            [[steps]]
            name = "Design"
            next = ["Missing"]
        "#}));
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("unknown next step Missing"), "{error}");
    assert!(error.contains(".flow/workflow.toml"), "{error}");
}

#[test]
fn new_flow_from_template_creates_a_valid_local_flow() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();

    let created = run_cli(root, &["create-flow", "workflow"]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let flow_path = root.join(".flow/workflow.toml");
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

    let created = run_cli(root, &["create-flow", "workflow", "-g"]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let flow_path = root.join("global/flow/workflow.toml");
    let original = fs::read_to_string(&flow_path).unwrap();
    assert!(!root.join(".flow/workflow.toml").exists());

    let duplicate = run_cli(root, &["create-flow", "workflow", "-g"]);
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
    let instance_path = root.join(".flow/instances/run.toml");
    assert_eq!(
        fs::read_to_string(&instance_path).unwrap(),
        indoc! {r#"
            flow = "workflow"
            step = "Design"

            [autonomy]
            Design = "guided"
        "#}
    );

    let loaded = run_cli(root, &["show", "run"]);
    assert!(
        loaded.status.success(),
        "{}",
        String::from_utf8_lossy(&loaded.stderr)
    );
    assert!(String::from_utf8_lossy(&loaded.stdout).contains("step: Design"));
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

    let status = run_cli(root, &["list", "instances"]);
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&status.stdout),
        "INSTANCE    FLOW      OWNER    STEP\n----------  --------  -------  ----\nglobal-run  workflow  unowned  Design\nlocal-run   workflow  unowned  Design\n"
    );
}

#[test]
fn lists_no_flows_when_storage_is_empty() {
    let directory = tempfile::tempdir().unwrap();
    let output = run_cli(directory.path(), &["list", "flows"]);
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "no flows found\n");
}

#[test]
fn lists_flows_with_summaries_and_local_shadowing() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let local = root.join(".flow/workflow.toml");
    let global = root.join("global/flow/workflow.toml");
    write_flow(&local);
    write_flow(&global);
    fs::write(
        &local,
        "summary = 'Local workflow'\ninitial_step = 'Done'\n[[steps]]\nname = 'Done'\n",
    )
    .unwrap();
    fs::write(&global, "invalid TOML = ").unwrap();
    write_flow(&root.join("global/flow/other.toml"));

    let output = run_cli(root, &["list", "flows"]);
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
fn lists_steps_with_summaries_in_flow_order() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let path = root.join(".flow/workflow.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "initial_step = 'Draft'\n[[steps]]\nname = 'Draft'\nsummary = 'Start here'\n[[steps]]\nname = 'Review'\n").unwrap();

    let output = run_cli(root, &["list", "steps", "workflow"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "STEP    SUMMARY\n------  -------\nDraft   Start here\nReview\n"
    );
}

#[test]
fn status_displays_flow_step_and_next_step_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let flow_path = root.join(".flow/workflow.toml");
    fs::create_dir_all(flow_path.parent().unwrap()).unwrap();
    fs::write(
        &flow_path,
        indoc! {r#"
            use_global = false
            summary = "Whole workflow summary"
            details = "Flow-wide details"
            checklist = ["Always do this", "Consider this when relevant"]
            initial_step = "Draft"

            [[steps]]
            name = "Draft"
            summary = "Prepare the work"
            details = "Current-step details"
            checklist = ["Confirm the goal", "Check for existing work"]
            next = ["Review"]

            [[steps]]
            name = "Review"
            summary = "Review the work"
            next = []
        "#},
    )
    .unwrap();
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());

    let status = run_cli(root, &["show", "run"]);
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
            current step: Draft (Prepare the work)
            autonomy: guided (Confirm step boundaries and each emerging design choice.)
            owner: unowned

            Flow details:
            Flow-wide details

            Step details:
            Current-step details

            Flow checklist:
            - Always do this
            - Consider this when relevant

            Step checklist:
            - Confirm the goal
            - Check for existing work

            Next steps:
              - Review: Review the work
        "}
    );
}

#[test]
fn status_preview_shows_selected_guidance_without_changing_instance() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let path = root.join(".flow/workflow.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        indoc! {r#"
        use_global = false
        summary = "Workflow summary"
        details = "Flow guidance"
        checklist = ["Flow checklist item"]
        initial_step = "Design"
        [[steps]]
        name = "Design"
        summary = "Planning"
        details = "Design guidance"
        checklist = ["Design checklist item"]
        next = ["Review"]
        [[steps]]
        name = "Review"
        summary = "Checking"
        details = "Review guidance"
        checklist = ["Review checklist item"]
        next = ["Done"]
        [[steps]]
        name = "Done"
        summary = "Finished"
        next = []
    "#},
    )
    .unwrap();
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    assert!(
        run_cli(root, &["autonomy", "set", "run", "steered", "--step", "Review"])
            .status
            .success()
    );
    let instance_path = root.join(".flow/instances/run.toml");
    let before = fs::read(&instance_path).unwrap();
    let current = run_cli(root, &["show", "run"]).stdout;
    let preview = run_cli(root, &["show", "run", "--step", "Review"]);
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&preview.stdout),
        indoc! {"\
        Step Preview

        flow: workflow (Workflow summary)
        current step: Design (Planning)
        inspected step: Review (Checking)
        autonomy: steered (Proceed with routine work; ask on consequential choices.)
        owner: unowned

        Flow details:
        Flow guidance

        Step details:
        Review guidance

        Flow checklist:
        - Flow checklist item

        Step checklist:
        - Review checklist item

        Next steps:
          - Done: Finished
    "}
    );
    assert_eq!(fs::read(&instance_path).unwrap(), before);
    let future_final = String::from_utf8(run_cli(root, &["show", "run", "--step", "Done"]).stdout).unwrap();
    assert!(!future_final.contains("flow complete"), "{future_final}");
    assert_eq!(run_cli(root, &["show", "run"]).stdout, current);
    let invalid = run_cli(root, &["show", "run", "--step", "Unknown"]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("unknown step \"Unknown\""));
    assert!(invalid.stdout.is_empty());
    assert_eq!(fs::read(&instance_path).unwrap(), before);
}

#[test]
fn status_preview_respects_final_and_skip_overrides() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let flow_path = root.join(".flow/workflow.toml");
    fs::create_dir_all(flow_path.parent().unwrap()).unwrap();
    fs::write(
        &flow_path,
        indoc! {r#"
        initial_step = "Design"
        [[steps]]
        name = "Design"
        next = ["Review"]
        [[steps]]
        name = "Review"
        next = ["Done"]
        [[steps]]
        name = "Done"
        next = []
    "#},
    )
    .unwrap();
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    assert!(
        run_cli(root, &["route", "skip", "run", "Review"])
            .status
            .success()
    );
    let path = root.join(".flow/instances/run.toml");
    let before = fs::read(&path).unwrap();
    let preview = run_cli(root, &["show", "run", "--step", "Design"]);
    let text = String::from_utf8_lossy(&preview.stdout);
    assert!(preview.status.success(), "{text}");
    assert!(
        text.contains("current step: Design\ninspected step: Design"),
        "{text}"
    );
    assert!(text.contains("Next steps:\n  - Done\n"), "{text}");
    assert!(!text.contains("  - Review"), "{text}");
    assert_eq!(fs::read(&path).unwrap(), before);

    assert!(
        run_cli(root, &["route", "final", "add", "run", "Design"])
            .status
            .success()
    );
    let before = fs::read(&path).unwrap();
    let preview = run_cli(root, &["show", "run", "--step", "Design"]);
    let text = String::from_utf8_lossy(&preview.stdout);
    assert!(preview.status.success(), "{text}");
    assert!(
        text.contains("Next steps:\n  This is the final step. Run `flow complete run` to complete the flow."),
        "{text}"
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    // A preview of another step must not inherit the current step's final override.
    let other = run_cli(root, &["show", "run", "--step", "Review"]);
    let text = String::from_utf8_lossy(&other.stdout);
    assert!(other.status.success(), "{text}");
    assert!(
        text.contains("current step: Design\ninspected step: Review"),
        "{text}"
    );
    assert!(text.contains("Next steps:\n  - Done"), "{text}");
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn status_reports_when_no_instances_exist() {
    let directory = tempfile::tempdir().unwrap();
    let status = run_cli(directory.path(), &["list", "instances"]);
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&status.stdout), "no instances found\n");
}

fn write_branching_flow(root: &Path) {
    let path = root.join(".flow/workflow.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        indoc! {r#"
            initial_step = "Start"
            [[steps]]
            name = "Start"
            next = ["First", "Second"]
            [[steps]]
            name = "Second"
            next = []
            [[steps]]
            name = "Expanded"
            next = []
            [[steps]]
            name = "First"
            next = ["Expanded", "Second"]
        "#},
    )
    .unwrap();
}

#[test]
fn next_without_target_rejects_ambiguity_without_mutation() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_branching_flow(root);
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    let path = root.join(".flow/instances/run.toml");
    let before = fs::read(&path).unwrap();
    let implicit = run_cli(root, &["next", "run"]);
    assert!(!implicit.status.success());
    let error = String::from_utf8_lossy(&implicit.stderr);
    assert!(error.contains("First") && error.contains("Second"), "{error}");
    assert_eq!(before, fs::read(&path).unwrap());
    assert!(run_cli(root, &["next", "run", "First"]).status.success());
}

#[test]
fn next_ambiguity_counts_effective_destinations_after_skips() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_branching_flow(root);
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    assert!(run_cli(root, &["route", "skip", "run", "First"]).status.success());
    let path = root.join(".flow/instances/run.toml");
    let before = fs::read(&path).unwrap();
    let next = run_cli(root, &["next", "run"]);
    assert!(!next.status.success());
    let error = String::from_utf8_lossy(&next.stderr);
    assert!(error.contains("Expanded") && error.contains("Second"), "{error}");
    assert_eq!(before, fs::read(&path).unwrap());
    assert!(
        !run_cli(root, &["route", "skip", "run", "Expanded"])
            .status
            .success()
    );
    assert!(run_cli(root, &["next", "run", "Expanded"]).status.success());
}

#[test]
fn next_without_target_matches_explicit_single_destination() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flow/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    let implicit = run_cli(root, &["next", "run"]);
    assert!(implicit.status.success());
    let path = root.join(".flow/instances/run.toml");
    let after = fs::read(&path).unwrap();
    assert!(run_cli(root, &["jump", "run", "Design"]).status.success());
    let explicit = run_cli(root, &["next", "run", "Review"]);
    assert!(explicit.status.success());
    assert_eq!(implicit.stdout, explicit.stdout);
    assert_eq!(after, fs::read(&path).unwrap());
}

#[test]
fn next_explicit_nonfirst_target_and_invalid_target_keep_existing_behavior() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_branching_flow(root);
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    let path = root.join(".flow/instances/run.toml");
    let before = fs::read(&path).unwrap();
    let invalid = run_cli(root, &["next", "run", "Expanded"]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("cannot move next to \"Expanded\""));
    assert_eq!(before, fs::read(&path).unwrap());

    let next = run_cli(root, &["next", "run", "Second"]);
    assert!(next.status.success(), "{}", String::from_utf8_lossy(&next.stderr));
    assert!(String::from_utf8_lossy(&next.stdout).contains("step: Second"));
    assert!(fs::read_to_string(path).unwrap().contains("step = \"Second\""));
}

#[test]
fn next_without_target_at_terminal_or_custom_final_does_not_modify_storage() {
    for setup in [
        &["jump", "run", "Review"][..],
        &["route", "final", "add", "run", "Design"],
    ] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        write_movable_flow(&root.join(".flow/workflow.toml"));
        assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
        assert!(run_cli(root, setup).status.success());
        let path = root.join(".flow/instances/run.toml");
        let before = fs::read(&path).unwrap();
        let flow_before = fs::read(root.join(".flow/workflow.toml")).unwrap();
        let next = run_cli(root, &["next", "run"]);
        assert!(!next.status.success());
        assert!(
            String::from_utf8_lossy(&next.stderr).contains("instance run has no available next transitions")
        );
        assert!(next.stdout.is_empty());
        assert_eq!(before, fs::read(path).unwrap());
        assert_eq!(flow_before, fs::read(root.join(".flow/workflow.toml")).unwrap());
    }
}

#[test]
fn next_and_jump_persist_local_instance_step() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flow/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());

    let next = run_cli(root, &["next", "run", "Review"]);
    assert!(next.status.success(), "{}", String::from_utf8_lossy(&next.stderr));
    let loaded = run_cli(root, &["show", "run"]);
    assert!(loaded.status.success());
    assert!(String::from_utf8_lossy(&loaded.stdout).contains("step: Review"));

    let jump = run_cli(root, &["jump", "run", "Done"]);
    assert!(jump.status.success(), "{}", String::from_utf8_lossy(&jump.stderr));
    assert_eq!(
        fs::read_to_string(root.join(".flow/instances/run.toml")).unwrap(),
        indoc! {r#"
            flow = "workflow"
            step = "Done"

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
    write_movable_flow(&root.join(".flow/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    let path = root.join(".flow/instances/run.toml");
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

    let status = run_cli(root, &["show", "run"]);
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
    write_movable_flow(&root.join(".flow/workflow.toml"));
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
        toml::from_str(&fs::read_to_string(root.join(".flow/instances/run.toml")).unwrap()).unwrap();
    assert_eq!(saved["context"]["plan"].as_str(), Some(exact_limit.as_str()));
}

#[test]
fn autonomy_range_is_persisted_per_step_and_follows_moves() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flow/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());

    let range = run_cli(
        root,
        &[
            "autonomy",
            "set",
            "run",
            "steered",
            "--from",
            "Design",
            "--through",
            "Review",
        ],
    );
    assert!(
        range.status.success(),
        "{}",
        String::from_utf8_lossy(&range.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&range.stdout),
        format!("set autonomy to steered for steps:\n  - Design\n  - Review\n{AUTONOMY_HINT}")
    );
    let single = run_cli(
        root,
        &["autonomy", "set", "run", "autonomous", "--step", "Review"],
    );
    assert!(
        single.status.success(),
        "{}",
        String::from_utf8_lossy(&single.stderr)
    );

    assert_eq!(
        String::from_utf8_lossy(&single.stdout),
        format!("set autonomy to autonomous for steps:\n  - Review\n{AUTONOMY_HINT}")
    );
    let path = root.join(".flow/instances/run.toml");
    let saved: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["autonomy"]["Design"].as_str(), Some("steered"));
    assert_eq!(saved["autonomy"]["Review"].as_str(), Some("autonomous"));
    assert_eq!(saved["autonomy"]["Done"].as_str(), Some("guided"));

    let status = run_cli(root, &["show", "run"]);
    assert!(String::from_utf8_lossy(&status.stdout).contains("current step: Design\nautonomy: steered ("));
    assert!(run_cli(root, &["next", "run", "Review"]).status.success());
    let status = run_cli(root, &["show", "run"]);
    let output = String::from_utf8_lossy(&status.stdout);
    assert!(
        output.contains("current step: Review\nautonomy: autonomous ("),
        "{output}"
    );
    assert!(
        output.contains("Work independently; escalate fundamental blockers or reserved decisions."),
        "{output}"
    );
    assert!(run_cli(root, &["jump", "run", "Done"]).status.success());
    assert!(
        String::from_utf8_lossy(&run_cli(root, &["show", "run"]).stdout)
            .contains("current step: Done\nautonomy: guided (")
    );
}

#[test]
fn autonomy_range_includes_all_routes_and_loops_before_end_but_stops_at_end() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let path = root.join(".flow/workflow.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        indoc! {r#"
        initial_step = "Start"
        [[steps]]
        name = "End"
        next = ["After"]
        [[steps]]
        name = "Right"
        next = ["End"]
        [[steps]]
        name = "Start"
        next = ["Left", "Right", "Dead End"]
        [[steps]]
        name = "Dead End"
        next = ["Dead End"]
        [[steps]]
        name = "Loop"
        next = ["Left"]
        [[steps]]
        name = "After"
        next = ["End"]
        [[steps]]
        name = "Left"
        next = ["Loop", "End"]
        [[steps]]
        name = "Unrelated"
    "#},
    )
    .unwrap();
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());

    let range = run_cli(
        root,
        &[
            "autonomy",
            "set",
            "run",
            "steered",
            "--from",
            "Start",
            "--through",
            "End",
        ],
    );
    assert!(
        range.status.success(),
        "{}",
        String::from_utf8_lossy(&range.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&range.stdout),
        format!(
            "set autonomy to steered for steps:\n  - End\n  - Right\n  - Start\n  - Loop\n  - Left\n{AUTONOMY_HINT}"
        )
    );
    let saved: toml::Value =
        toml::from_str(&fs::read_to_string(root.join(".flow/instances/run.toml")).unwrap()).unwrap();
    for name in ["End", "Right", "Start", "Loop", "Left"] {
        assert_eq!(saved["autonomy"][name].as_str(), Some("steered"), "{name}");
    }
    for name in ["Dead End", "After", "Unrelated"] {
        assert_eq!(saved["autonomy"][name].as_str(), Some("guided"), "{name}");
    }

    let same = run_cli(
        root,
        &[
            "autonomy",
            "set",
            "run",
            "autonomous",
            "--from",
            "End",
            "--through",
            "End",
        ],
    );
    assert_eq!(
        String::from_utf8_lossy(&same.stdout),
        format!("set autonomy to autonomous for steps:\n  - End\n{AUTONOMY_HINT}")
    );
    let original = fs::read_to_string(root.join(".flow/instances/run.toml")).unwrap();
    let unreachable = run_cli(
        root,
        &[
            "autonomy",
            "set",
            "run",
            "steered",
            "--from",
            "Start",
            "--through",
            "Unrelated",
        ],
    );
    assert!(!unreachable.status.success());
    assert!(String::from_utf8_lossy(&unreachable.stderr).contains("no next path"));
    assert_eq!(
        fs::read_to_string(root.join(".flow/instances/run.toml")).unwrap(),
        original
    );
}

#[test]
fn autonomy_rejects_invalid_ranges_without_changing_instance() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flow/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    let path = root.join(".flow/instances/run.toml");
    let original = fs::read_to_string(&path).unwrap();

    for (args, message) in [
        (
            vec![
                "autonomy",
                "set",
                "run",
                "steered",
                "--from",
                "Done",
                "--through",
                "Design",
            ],
            "no next path",
        ),
        (
            vec!["autonomy", "set", "run", "steered", "--step", "Missing"],
            "unknown step",
        ),
        (
            vec![
                "autonomy",
                "set",
                "run",
                "steered",
                "--from",
                "Design",
                "--through",
                "Missing",
            ],
            "unknown step",
        ),
        (
            vec!["autonomy", "set", "run", "unsupervised", "--step", "Design"],
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
fn autonomy_updates_global_instance_and_handles_step_names_with_spaces() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let flow = root.join("global/flow/workflow.toml");
    fs::create_dir_all(flow.parent().unwrap()).unwrap();
    fs::write(
        &flow,
        "initial_step = 'In Progress'\n[[steps]]\nname = 'In Progress'\n[[steps]]\nname = 'Done'\n",
    )
    .unwrap();
    assert!(
        run_cli(root, &["start", "workflow", "run", "-g"])
            .status
            .success()
    );

    let set = run_cli(
        root,
        &["autonomy", "set", "run", "steered", "--step", "In Progress"],
    );
    assert!(set.status.success(), "{}", String::from_utf8_lossy(&set.stderr));
    assert!(!root.join(".flow/instances/run.toml").exists());
    let saved = fs::read_to_string(root.join("global/flow/instances/run.toml")).unwrap();
    let document: toml::Value = toml::from_str(&saved).unwrap();
    assert_eq!(document["autonomy"]["In Progress"].as_str(), Some("steered"));
    assert!(
        String::from_utf8_lossy(&run_cli(root, &["show", "run"]).stdout)
            .contains("current step: In Progress\nautonomy: steered (")
    );
}

#[test]
fn legacy_instances_gain_guided_autonomy_for_every_step() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flow/workflow.toml"));
    let path = root.join(".flow/instances/run.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "flow = 'workflow'\nstep = 'Design'\n").unwrap();

    let status = run_cli(root, &["show", "run"]);
    assert!(status.status.success());
    assert!(String::from_utf8_lossy(&status.stdout).contains("autonomy: guided"));
    assert!(
        run_cli(root, &["autonomy", "set", "run", "steered", "--step", "Review"])
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
    let path = root.join(".flow/instances/run.toml");
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
    assert!(!root.join(".flow/instances/run.toml").exists());
    let saved = fs::read_to_string(root.join("global/flow/instances/run.toml")).unwrap();
    let document: toml::Value = toml::from_str(&saved).unwrap();
    assert_eq!(document["context"]["goal"].as_str(), Some("finish"));
}

#[test]
fn rejected_moves_leave_instance_file_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flow/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    let path = root.join(".flow/instances/run.toml");
    let original = fs::read_to_string(&path).unwrap();

    let next = run_cli(root, &["next", "run", "Done"]);
    assert!(!next.status.success());
    assert!(String::from_utf8_lossy(&next.stderr).contains("cannot move next"));
    let jump = run_cli(root, &["jump", "run", "Missing"]);
    assert!(!jump.status.success());
    assert!(String::from_utf8_lossy(&jump.stderr).contains("unknown step"));
    assert_eq!(fs::read_to_string(path).unwrap(), original);
}

#[test]
fn movement_updates_global_instance_without_creating_a_local_one() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join("global/flow/workflow.toml"));
    assert!(
        run_cli(root, &["start", "workflow", "run", "-g"])
            .status
            .success()
    );

    let next = run_cli(root, &["next", "run", "Review"]);
    assert!(next.status.success(), "{}", String::from_utf8_lossy(&next.stderr));
    assert!(!root.join(".flow/instances/run.toml").exists());
    assert!(
        fs::read_to_string(root.join("global/flow/instances/run.toml"))
            .unwrap()
            .contains("step = \"Review\"")
    );
}

#[test]
fn new_rejects_collision_in_global_scope() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    let existing = root.join("global/flow/instances/run.toml");
    fs::create_dir_all(existing.parent().unwrap()).unwrap();
    fs::write(&existing, "original").unwrap();

    let output = run_cli(root, &["start", "workflow", "run"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert!(!root.join(".flow/instances/run.toml").exists());
    assert_eq!(fs::read_to_string(existing).unwrap(), "original");
}

#[test]
fn new_requires_flow_before_creating_instance_file() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();

    let output = run_cli(root, &["start", "workflow", "run"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("flow \"workflow\" not found"));
    assert!(!root.join(".flow/instances").exists());
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
        root.join(".flow/instances")
            .join(format!("{name}.toml"))
            .is_file()
    );

    let loaded = run_cli(root, &["show", name]);
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
        fs::read_to_string(root.join("global/flow/instances/run.toml")).unwrap(),
        indoc! {r#"
            flow = "workflow"
            step = "Design"

            [autonomy]
            Design = "guided"
        "#}
    );
    assert!(!root.join(".flow/instances/run.toml").exists());
    let loaded = run_cli(root, &["show", "run"]);
    assert!(
        loaded.status.success(),
        "{}",
        String::from_utf8_lossy(&loaded.stderr)
    );
    assert!(String::from_utf8_lossy(&loaded.stdout).contains("step: Design"));
}

#[test]
fn new_copy_flow_installs_local_flow_and_creates_global_instance() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    let local_flow = root.join(".flow/workflow.toml");

    let output = run_cli(root, &["start", "workflow", "run", "-G"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("global/flow/workflow.toml")).unwrap(),
        fs::read_to_string(local_flow).unwrap()
    );
    assert!(!root.join(".flow/instances/run.toml").exists());
    let loaded = run_cli(root, &["show", "run"]);
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
    let global_flow = root.join("global/flow/workflow.toml");
    fs::create_dir_all(global_flow.parent().unwrap()).unwrap();
    let existing = indoc! {r#"
        initial_step = "Review"
        [[steps]]
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
    assert!(root.join("global/flow/instances/run.toml").is_file());
}

#[test]
fn start_rejects_conflicting_global_flags_without_writes() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    let flow_path = root.join(".flow/workflow.toml");
    let before = fs::read(&flow_path).unwrap();

    for flags in [
        ["--global", "--copy-flow"],
        ["--copy-flow", "--global"],
        ["-g", "-G"],
        ["-G", "-g"],
    ] {
        let output = run_cli(root, &["start", "workflow", "run", flags[0], flags[1]]);
        assert!(!output.status.success(), "{flags:?}");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("cannot be used with"), "{error}");
        assert!(
            error.contains("--global") && error.contains("--copy-flow"),
            "{error}"
        );
        assert!(output.stdout.is_empty());
        assert_eq!(fs::read(&flow_path).unwrap(), before);
        assert_eq!(fs::read_dir(root.join(".flow")).unwrap().count(), 1);
        assert!(!root.join("global").exists());
    }
}

#[test]
fn new_copy_flow_checks_instance_collision_before_copy() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    let existing = root.join(".flow/instances/run.toml");
    fs::create_dir_all(existing.parent().unwrap()).unwrap();
    fs::write(&existing, "original").unwrap();

    let output = run_cli(root, &["start", "workflow", "run", "-G"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert!(!root.join("global/flow/workflow.toml").exists());
    assert_eq!(fs::read_to_string(existing).unwrap(), "original");
}

#[test]
fn new_copy_flow_rejects_invalid_existing_global_flow() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    let global_flow = root.join("global/flow/workflow.toml");
    fs::create_dir_all(global_flow.parent().unwrap()).unwrap();
    fs::write(&global_flow, "invalid toml = ").unwrap();

    let output = run_cli(root, &["start", "workflow", "run", "-G"]);
    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(global_flow).unwrap(), "invalid toml = ");
    assert!(!root.join("global/flow/instances/run.toml").exists());
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
    assert!(error.contains("global/flow/workflow.toml"), "{error}");
    assert!(!root.join("global/flow/instances/run.toml").exists());
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
    let path = root.join(".flow/instances/run.toml");
    let saved: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["owner"].as_str(), Some("agent-a"));
    assert!(String::from_utf8_lossy(&run_cli(root, &["list", "instances"]).stdout).contains("agent-a"));

    assert!(
        run_cli(root, &["claim", "run", "--owner", "agent-a"])
            .status
            .success()
    );
    let rejected = run_cli(root, &["claim", "run", "--owner", "agent-b"]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("owned by \"agent-a\""));

    assert!(run_cli(root, &["release", "run"]).status.success());
    let saved: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(saved.get("owner").is_none());
    assert!(
        run_cli(root, &["claim", "run", "--owner", "agent-b"])
            .status
            .success()
    );
    assert!(run_cli(root, &["delete", "run"]).status.success());
    assert!(!path.exists());
}

#[test]
fn complete_deletes_an_instance_only_at_a_terminal_step() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    let path = root.join(".flow/instances/run.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "flow = 'workflow'\nstep = 'Design'\n").unwrap();

    let status = String::from_utf8(run_cli(root, &["show", "run"]).stdout).unwrap();
    assert!(
        status
            .contains("Next steps:\n  This is the final step. Run `flow complete run` to complete the flow."),
        "{status}"
    );
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
fn complete_preserves_an_instance_with_available_next_steps() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flow/workflow.toml"));
    let path = root.join(".flow/instances/run.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "flow = 'workflow'\nstep = 'Design'\n").unwrap();

    let completed = run_cli(root, &["complete", "run"]);
    assert!(!completed.status.success());
    assert!(String::from_utf8_lossy(&completed.stderr).contains("step has next transitions"));
    assert!(path.exists());
}

#[test]
fn stop_can_delete_an_instance_when_its_flow_is_missing() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let path = root.join(".flow/instances/run.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "flow = 'missing'\nstep = 'Unknown'\n").unwrap();

    let stopped = run_cli(root, &["delete", "run"]);
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
    let global_flow = root.join("global/flow/workflow.toml");
    fs::create_dir_all(global_flow.parent().unwrap()).unwrap();
    fs::write(&global_flow, "invalid toml = ").unwrap();

    let output = run_cli(root, &["start", "workflow", "run", "-g"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("global/flow/workflow.toml"));
    assert!(!root.join("global/flow/instances/run.toml").exists());
}

#[test]
fn instance_configuration_persists_and_controls_status_next_jump_and_complete() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let flow = root.join(".flow/workflow.toml");
    fs::create_dir_all(flow.parent().unwrap()).unwrap();
    fs::write(
        &flow,
        indoc! {r#"
        initial_step = "Start"
        use_global = false
        [[steps]]
        name = "Start"
        next = ["A", "B", "Finish"]
        [[steps]]
        name = "A"
        next = ["B", "Finish"]
        [[steps]]
        name = "B"
        next = ["Finish", "A"]
        [[steps]]
        name = "Finish"
        next = []
    "#},
    )
    .unwrap();
    let path = root.join(".flow/instances/run.toml");
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    assert!(
        !fs::read_to_string(&path).unwrap().contains("[step_overrides]"),
        "empty overrides should not be serialized"
    );
    for args in [&["route", "skip", "run", "A"][..], &["route", "skip", "run", "B"]] {
        let result = run_cli(root, args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let saved: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["step_overrides"]["skipped"].as_array().unwrap().len(), 2);
    assert!(saved.get("skipped").is_none(), "{saved}");
    let status = run_cli(root, &["show", "run"]);
    let text = String::from_utf8_lossy(&status.stdout);
    assert!(text.contains("Next steps:\n  - Finish\n"), "{text}");
    assert!(!run_cli(root, &["next", "run", "A"]).status.success());
    assert!(!run_cli(root, &["jump", "run", "B"]).status.success());
    assert!(run_cli(root, &["next", "run"]).status.success());
    assert!(run_cli(root, &["complete", "run"]).status.success());
    assert!(!path.exists());
}

#[test]
fn initial_and_custom_final_are_reversible_and_failed_commands_leave_file_untouched() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flow/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    let path = root.join(".flow/instances/run.toml");
    assert!(
        run_cli(root, &["route", "initial", "set", "run", "Review"])
            .status
            .success()
    );
    let contents = fs::read_to_string(&path).unwrap();
    let saved: toml::Value = toml::from_str(&contents).unwrap();
    assert_eq!(saved["step_overrides"]["initial"].as_str(), Some("Review"));
    assert!(saved.get("initial").is_none(), "{contents}");
    assert_eq!(saved["step"].as_str(), Some("Design"));
    assert!(run_cli(root, &["reset", "run"]).status.success());
    assert!(
        String::from_utf8_lossy(&run_cli(root, &["show", "run"]).stdout).contains("current step: Review")
    );
    assert!(run_cli(root, &["jump", "run", "Design"]).status.success());
    assert!(
        run_cli(root, &["route", "initial", "set", "run", "Review"])
            .status
            .success()
    );
    assert!(
        String::from_utf8_lossy(&run_cli(root, &["show", "run"]).stdout).contains("current step: Design")
    );
    assert!(run_cli(root, &["reset", "run"]).status.success());
    assert!(
        run_cli(root, &["route", "final", "add", "run", "Review"])
            .status
            .success()
    );
    let saved: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        saved["step_overrides"]["finals"].as_array().unwrap()[0].as_str(),
        Some("Review")
    );
    assert!(saved.get("finals").is_none(), "{saved}");
    let status = String::from_utf8(run_cli(root, &["show", "run"]).stdout).unwrap();
    assert!(
        status
            .contains("Next steps:\n  This is the final step. Run `flow complete run` to complete the flow."),
        "{status}"
    );
    assert!(
        run_cli(root, &["route", "final", "remove", "run", "Review"])
            .status
            .success()
    );
    assert!(
        run_cli(root, &["route", "initial", "set", "run", "Design"])
            .status
            .success()
    );
    let contents = fs::read_to_string(&path).unwrap();
    assert!(!contents.contains("[step_overrides]"), "{contents}");
    for args in [
        &["route", "skip", "run", "Design"][..],
        &["route", "skip", "run", "Missing"],
        &["route", "initial", "set", "run", "Missing"],
        &["route", "final", "remove", "run", "Review"],
        &["route", "unskip", "run", "Review"],
    ] {
        let before = fs::read(&path).unwrap();
        assert!(!run_cli(root, args).status.success(), "{args:?}");
        assert_eq!(fs::read(&path).unwrap(), before, "{args:?}");
    }
    assert!(
        run_cli(root, &["route", "final", "add", "run", "Review"])
            .status
            .success()
    );
    let before = fs::read(&path).unwrap();
    assert!(
        !run_cli(root, &["route", "skip", "run", "Review"])
            .status
            .success()
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(
        run_cli(root, &["route", "final", "remove", "run", "Review"])
            .status
            .success()
    );
    let before = fs::read(&path).unwrap();
    assert!(
        !run_cli(root, &["route", "skip", "run", "Review"])
            .status
            .success()
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(
        run_cli(root, &["route", "initial", "set", "run", "Review"])
            .status
            .success()
    );
}

#[test]
fn custom_final_allows_completion_despite_flow_transitions() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flow/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    assert!(
        run_cli(root, &["route", "final", "add", "run", "Design"])
            .status
            .success()
    );
    assert!(run_cli(root, &["complete", "run"]).status.success());
    assert!(!root.join(".flow/instances/run.toml").exists());
}

#[test]
fn skipped_cycle_does_not_make_a_nonfinal_step_completable() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let flow = root.join(".flow/workflow.toml");
    fs::create_dir_all(flow.parent().unwrap()).unwrap();
    fs::write(
        &flow,
        "initial_step = 'Start'\nuse_global = false\n[[steps]]\nname = 'Start'\nnext = ['Loop']\n[[steps]]\nname = 'Loop'\nnext = ['Loop']\n",
    )
    .unwrap();
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    assert!(run_cli(root, &["route", "skip", "run", "Loop"]).status.success());
    let path = root.join(".flow/instances/run.toml");
    let before = fs::read(&path).unwrap();
    let status = String::from_utf8(run_cli(root, &["show", "run"]).stdout).unwrap();
    assert!(!status.contains("This is the final step"), "{status}");
    let next = run_cli(root, &["next", "run"]);
    assert!(!next.status.success());
    assert!(String::from_utf8_lossy(&next.stderr).contains("instance run has no available next transitions"));
    assert!(next.stdout.is_empty());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!run_cli(root, &["complete", "run"]).status.success());
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn old_instance_without_overrides_loads_and_nested_overrides_round_trip() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let flow = root.join(".flow/workflow.toml");
    fs::create_dir_all(flow.parent().unwrap()).unwrap();
    fs::write(&flow, "initial_step = 'Design'\n[[steps]]\nname = 'Design'\nnext = ['Review']\n[[steps]]\nname = 'Review'\nnext = ['Done']\n[[steps]]\nname = 'Done'\n").unwrap();
    let path = root.join(".flow/instances/run.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "flow = 'workflow'\nstep = 'Design'\n").unwrap();
    assert!(run_cli(root, &["show", "run"]).status.success());
    assert!(
        run_cli(root, &["claim", "run", "--owner", "operator"])
            .status
            .success()
    );
    assert!(!fs::read_to_string(&path).unwrap().contains("[step_overrides]"));
    fs::write(&path, "flow = 'workflow'\nstep = 'Design'\n[step_overrides]\ninitial = 'Done'\nfinals = ['Design']\nskipped = ['Review']\n").unwrap();
    assert!(run_cli(root, &["show", "run"]).status.success());
    assert!(
        run_cli(root, &["route", "initial", "set", "run", "Done"])
            .status
            .success()
    );
    let contents = fs::read_to_string(&path).unwrap();
    let saved: toml::Value = toml::from_str(&contents).unwrap();
    assert_eq!(
        saved["step_overrides"]["skipped"].as_array().unwrap()[0].as_str(),
        Some("Review")
    );
    assert!(
        run_cli(root, &["route", "unskip", "run", "Review"])
            .status
            .success()
    );
    let contents = fs::read_to_string(&path).unwrap();
    let saved: toml::Value = toml::from_str(&contents).unwrap();
    assert_eq!(saved["step_overrides"]["initial"].as_str(), Some("Done"));
    assert_eq!(
        saved["step_overrides"]["finals"].as_array().unwrap()[0].as_str(),
        Some("Design")
    );
    assert!(saved["step_overrides"].get("skipped").is_none(), "{contents}");
    assert!(
        saved.get("initial").is_none() && saved.get("finals").is_none(),
        "{contents}"
    );
}

#[test]
fn invalid_persisted_instance_configuration_is_rejected_on_load() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flow/workflow.toml"));
    let path = root.join(".flow/instances/run.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    for extra in [
        "initial = 'Missing'\n",
        "finals = ['Missing']\n",
        "skipped = ['Missing']\n",
        "skipped = ['Design']\n",
        "skipped = ['Review']\n",
        "skipped = ['Review']\nfinals = ['Review']\n",
    ] {
        fs::write(
            &path,
            format!("flow = 'workflow'\nstep = 'Design'\n[step_overrides]\n{extra}"),
        )
        .unwrap();
        let output = run_cli(root, &["show", "run"]);
        assert!(!output.status.success(), "{extra}");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            format!("flow = 'workflow'\nstep = 'Design'\n[step_overrides]\n{extra}")
        );
    }
}

#[test]
fn bare_list_shows_help_without_reading_storage() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::create_dir_all(root.join(".flow")).unwrap();
    fs::write(root.join(".flow/broken.toml"), "not a flow").unwrap();
    let output = run_cli(root, &["list"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let help = String::from_utf8_lossy(&output.stdout);
    for word in ["Usage:", "flows", "instances", "steps"] {
        assert!(help.contains(word), "{help}");
    }
    assert!(!root.join(".flow/instances").exists());
}

#[test]
fn removed_commands_and_implicit_argument_forms_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flow/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    let path = root.join(".flow/instances/run.toml");
    let before = fs::read(&path).unwrap();
    for args in [
        &["status", "run"][..],
        &["resume", "run", "--owner", "agent"],
        &["pause", "run"],
        &["stop", "run"],
        &["step-overrides", "skip", "run", "Review"],
        &["new-flow-from-template", "other"],
        &["show"],
        &["show", "run", "Review"],
        &["route", "initial", "run", "Review"],
        &["autonomy", "set", "run", "guided", "Review"],
    ] {
        assert!(!run_cli(root, args).status.success(), "{args:?}");
        assert_eq!(fs::read(&path).unwrap(), before, "{args:?}");
    }
}

#[test]
fn autonomy_selection_forms_are_required_complete_and_exclusive() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_movable_flow(&root.join(".flow/workflow.toml"));
    assert!(run_cli(root, &["start", "workflow", "run"]).status.success());
    let path = root.join(".flow/instances/run.toml");
    let before = fs::read(&path).unwrap();
    for selection in [
        vec![],
        vec!["--from", "Design"],
        vec!["--through", "Review"],
        vec!["--step", "Review", "--from", "Design", "--through", "Review"],
        vec!["--step", "Review", "--through", "Review"],
        vec!["--step", "Review", "--from", "Design"],
    ] {
        let mut args = vec!["autonomy", "set", "run", "steered"];
        args.extend(selection);
        let output = run_cli(root, &args);
        assert!(!output.status.success(), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert_eq!(fs::read(&path).unwrap(), before, "{args:?}");
    }
}

#[test]
fn instances_use_local_first_flow_for_initialization_and_reload() {
    for flags in [vec![], vec!["-g"], vec!["-G"]] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::create_dir_all(root.join(".flow")).unwrap();
        fs::write(
            root.join(".flow/workflow.toml"),
            "initial_step = 'LocalOnly'\n[[steps]]\nname = 'LocalOnly'\n",
        )
        .unwrap();
        let global_flow = root.join("global/flow/workflow.toml");
        write_movable_flow(&global_flow);
        let original = fs::read(&global_flow).unwrap();
        let mut args = vec!["start", "workflow", "run"];
        args.extend(flags);
        let started = run_cli(root, &args);
        assert!(
            started.status.success(),
            "{}",
            String::from_utf8_lossy(&started.stderr)
        );
        assert!(String::from_utf8_lossy(&started.stdout).contains("current step: LocalOnly"));
        assert_eq!(fs::read(&global_flow).unwrap(), original);
        let shown = run_cli(root, &["show", "run"]);
        assert!(
            shown.status.success(),
            "{}",
            String::from_utf8_lossy(&shown.stderr)
        );
        assert!(String::from_utf8_lossy(&shown.stdout).contains("current step: LocalOnly"));
    }
}

#[test]
fn old_storage_paths_are_ignored_and_left_untouched() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    for path in [".flows/workflow.toml", "global/flows/workflow.toml"] {
        write_flow(&root.join(path));
    }
    let old_instance = root.join(".flows/instances/run.toml");
    fs::create_dir_all(old_instance.parent().unwrap()).unwrap();
    fs::write(&old_instance, "flow = 'workflow'\nstep = 'Design'\n").unwrap();
    let before = fs::read(&old_instance).unwrap();
    for args in [&["show", "run"][..], &["start", "workflow", "new"]] {
        assert!(!run_cli(root, args).status.success(), "{args:?}");
    }
    let listed = run_cli(root, &["list", "flows"]);
    assert_eq!(String::from_utf8_lossy(&listed.stdout), "no flows found\n");
    assert_eq!(fs::read(&old_instance).unwrap(), before);
    assert!(!root.join(".flow").exists());
    assert!(!root.join("global/flow").exists());
}

#[test]
fn name_validation_rejects_consecutive_dots_before_any_write() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_local_flow(root);
    for args in [
        &["create-flow", "a..b"][..],
        &["start", "workflow", "a..b"],
        &["show", "a..b"],
    ] {
        let output = run_cli(root, args);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("consecutive dots"));
    }
    assert!(!root.join(".flow/instances").exists());
}
