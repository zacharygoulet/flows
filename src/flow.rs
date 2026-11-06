use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    Autonomy, FlowName, Instance, InstanceName,
    file_writer::FileWriter,
    storage::{Scope, Storage},
};
use rust_utils::raise::{self, RaiseContext, RaiseExt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, getset::Getters)]
#[serde(deny_unknown_fields)]
#[getset(get = "pub")]
pub struct Flow {
    #[serde(skip, default = "FlowName::placeholder")]
    name: FlowName,
    initial_state: StateName,
    states: Vec<State>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    details: Option<String>,
    #[serde(default)]
    steps: Vec<String>,
    #[serde(default = "default_use_global")]
    use_global: bool,
}

fn default_use_global() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct State {
    pub name: String,
    #[serde(default)]
    pub autonomy: Autonomy,
    #[serde(default)]
    pub next: Vec<StateName>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub steps: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StateName(pub String);

impl Flow {
    pub fn new(name: FlowName, initial_state: StateName, states: Vec<State>) -> Self {
        let flow = Self {
            name,
            initial_state,
            states,
            summary: None,
            details: None,
            steps: Vec::new(),
            use_global: true,
        };
        flow.validate_at(None);
        flow
    }

    pub fn into_new_instance(self, name: InstanceName) -> Instance {
        Instance::new(name, self)
    }

    pub fn validate(&self) {
        self.validate_at(None);
    }

    fn validate_at(&self, context: Option<&Path>) {
        let raise_validation_error = |message: String| match context {
            Some(path) => raise::raise(format!("validating {}: {message}", path.display())),
            None => raise::raise(message),
        };
        let mut all_state_names = HashSet::new();
        for state in &self.states {
            if state.name.trim().is_empty() {
                raise_validation_error("state names cannot be empty".to_owned());
            }
            if !all_state_names.insert(state.name.as_str()) {
                raise_validation_error(format!("duplicate state: {}", state.name));
            }
        }

        if !all_state_names.contains(self.initial_state.0.as_str()) {
            raise_validation_error(format!("unknown initial state: {}", self.initial_state.0));
        }

        for state in &self.states {
            for next in &state.next {
                if !all_state_names.contains(next.0.as_str()) {
                    raise_validation_error(format!(
                        "state {} points to unknown next state {}",
                        state.name, next.0
                    ));
                }
            }
        }
    }

    pub fn create_from_template(name: FlowName, contents: &str, scope: Scope) -> PathBuf {
        let path = Storage::current().new_flow_path(name.as_str(), scope);
        Self::parse(&path, contents);
        FileWriter::from(path.clone()).write_new(contents.as_bytes());
        path
    }

    pub fn load_from_name(name: &str) -> Self {
        let name = FlowName::parse(name).raise();
        let path = Storage::current().find_flow(name.as_str());
        Self::parse_from_path(&path)
    }

    pub fn load_global_from_name(name: &str) -> Self {
        let name = FlowName::parse(name).raise();
        Self::parse_from_path(&Storage::current().find_global_flow(name.as_str()))
    }

    pub fn ensure_global_flow(&self) {
        Self::load_global_from_name(self.name.as_str());
    }

    pub fn install_global_from_local_if_missing(&self) {
        let storage = Storage::current();
        let destination = storage.global_flow_path(self.name.as_str());
        if destination.exists() {
            Self::parse_from_path(&destination);
            return;
        }

        let source = storage.find_local_flow(self.name.as_str());
        let contents =
            fs::read_to_string(&source).raise_with_context(|| format!("reading {}", source.display()));
        Self::parse(&source, &contents);
        FileWriter::from(destination).write_new(contents.as_bytes());
    }

    fn parse_from_path(path: &Path) -> Self {
        let contents = fs::read_to_string(path).raise_with_context(|| format!("reading {}", path.display()));
        Self::parse(path, &contents)
    }

    fn parse(path: &Path, contents: &str) -> Self {
        let mut flow: Flow =
            toml::from_str(contents).raise_with_context(|| format!("parsing {}", path.display()));

        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .expect("flow paths must have a UTF-8 filename");
        flow.name = FlowName::parse(name).raise();
        flow.validate_at(Some(path));
        flow
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use indoc::indoc;

    use super::{Flow, State, StateName};
    use crate::FlowName;
    use rust_utils::raise::catch_raised;

    fn raised_message<T: std::fmt::Debug>(operation: impl FnOnce() -> T) -> String {
        catch_raised(std::panic::AssertUnwindSafe(operation))
            .unwrap_err()
            .to_string()
    }

    fn state(name: &str, next: Vec<StateName>) -> State {
        State {
            name: name.to_owned(),
            autonomy: Default::default(),
            next,
            summary: None,
            details: None,
            steps: Vec::new(),
        }
    }

    fn flow(initial_state: &str, states: Vec<State>) -> Flow {
        Flow {
            name: FlowName::parse("example").unwrap(),
            initial_state: StateName(initial_state.to_owned()),
            states,
            summary: None,
            details: None,
            steps: Vec::new(),
            use_global: true,
        }
    }

    #[test]
    fn missing_next_defaults_to_no_transitions() {
        let flow: Flow = toml::from_str("initial_state = \"Done\"\n\n[[states]]\nname = \"Done\"\n").unwrap();
        flow.validate();
        assert!(flow.states()[0].next.is_empty());
        assert!(*flow.use_global());
    }

    #[test]
    fn can_opt_out_of_global_guidance() {
        let flow: Flow =
            toml::from_str("use_global = false\ninitial_state = 'Done'\n[[states]]\nname = 'Done'\n")
                .unwrap();
        assert!(!flow.use_global());
    }

    #[test]
    fn rejects_empty_state_names() {
        let flow = flow(" ", vec![state(" ", vec![])]);
        assert!(raised_message(|| flow.validate()).contains("state names cannot be empty"));
    }

    #[test]
    fn rejects_duplicate_names() {
        let flow = flow("draft", vec![state("draft", vec![]), state("draft", vec![])]);
        assert!(raised_message(|| flow.validate()).contains("duplicate state"));
    }

    #[test]
    fn rejects_unknown_next_state() {
        let flow = flow("draft", vec![state("draft", vec![StateName("missing".into())])]);
        assert!(raised_message(|| flow.validate()).contains("unknown next state"));
    }

    #[test]
    fn rejects_unknown_initial_state() {
        let flow = flow("Missing", vec![state("Design", vec![])]);

        assert!(raised_message(|| flow.validate()).contains("unknown initial state"));
    }

    #[test]
    fn rejects_redundant_name_in_flow() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workflow");
        fs::write(
            &path,
            indoc! {r#"
                initial_state = "Design"
                name = "other"
                [[states]]
                name = "Design"
                next = []
            "#},
        )
        .unwrap();

        assert!(raised_message(|| Flow::parse_from_path(&path)).contains("unknown field"));
    }
}
