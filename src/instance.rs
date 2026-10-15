use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use crate::{
    Flow, FlowName, InstanceName, State, StateName,
    file_writer::FileWriter,
    storage::{Scope, Storage},
};
use rust_utils::raise::{self, RaiseContext, RaiseExt};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Deserialize, Serialize, getset::Getters)]
#[getset(get = "pub")]
pub struct Instance {
    #[serde(skip, default = "InstanceName::placeholder")]
    name: InstanceName,
    #[serde(deserialize_with = "load_flow", serialize_with = "save_flow_name")]
    flow: Flow,
    state: StateName,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    context: BTreeMap<String, String>,
    #[serde(default)]
    autonomy: BTreeMap<String, Autonomy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    owner: Option<String>,
}

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, strum::Display, strum::EnumString,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum Autonomy {
    Autonomous,
    Steered,
    #[default]
    Guided,
}

pub enum Move {
    Next(StateName),
    JumpTo(StateName),
}

pub enum ContextUpdate {
    Set { key: String, value: String },
    Remove { key: String },
}

#[derive(Debug)]
pub enum InstanceSavePolicy {
    Local,
    Global { copy_local_flow_to_global: bool },
}

impl InstanceSavePolicy {
    pub fn from_cli_args(global: bool, copy_flow: bool) -> Self {
        if global || copy_flow {
            Self::Global { copy_local_flow_to_global: copy_flow }
        } else {
            Self::Local
        }
    }

    fn scope(&self) -> Scope {
        match self {
            Self::Local => Scope::Local,
            Self::Global { .. } => Scope::Global,
        }
    }

    fn prepare_flow(&self, flow: &Flow) {
        match self {
            Self::Local => {}
            Self::Global { copy_local_flow_to_global: true } => {
                flow.install_global_from_local_if_missing();
            }
            Self::Global { copy_local_flow_to_global: false } => flow.ensure_global_flow(),
        }
    }
}

impl Instance {
    pub fn status(&self) -> String {
        format!("{}", crate::output::InstanceStatus(self))
    }

    pub fn new(name: InstanceName, flow: Flow) -> Self {
        flow.validate();
        let state = flow.initial_state().clone();
        let autonomy = flow
            .states()
            .iter()
            .map(|state| (state.name.clone(), Autonomy::Guided))
            .collect();
        let instance = Self { name, flow, state, context: BTreeMap::new(), autonomy, owner: None };
        instance.validate_current_state();
        instance
    }

    pub fn apply_move(&mut self, movement: Move) {
        let target = match movement {
            Move::Next(target) => {
                let current = self
                    .flow
                    .states()
                    .iter()
                    .find(|state| state.name == self.state.0)
                    .expect("current state was validated");
                if !current.next.iter().any(|next| next.0 == target.0) {
                    raise::raise(format!(
                        "state {:?} cannot move next to {:?} in flow {:?}",
                        self.state.0,
                        target.0,
                        self.flow.name()
                    ));
                }
                target
            }
            Move::JumpTo(target) => {
                if !self.flow.states().iter().any(|state| state.name == target.0) {
                    raise::raise(format!(
                        "cannot move to unknown state {:?} in flow {:?}",
                        target.0,
                        self.flow.name()
                    ));
                }
                target
            }
        };
        self.state = target;
    }

    pub fn move_to(mut self, movement: Move) -> Self {
        self.apply_move(movement);
        self.save_existing();
        self
    }

    pub fn with_owner(mut self, owner: String) -> Self {
        self.owner = Some(owner);
        self
    }

    pub fn claim(mut self, owner: String) -> Self {
        match self.owner.as_deref() {
            Some(existing) if existing == owner => return self,
            Some(existing) => raise::raise(format!("instance {:?} is owned by {existing:?}", self.name)),
            None => self.owner = Some(owner),
        }
        self.save_existing();
        self
    }

    pub fn release(mut self) -> Self {
        self.owner = None;
        self.save_existing();
        self
    }

    pub fn rename(mut self, new_name: InstanceName) -> Self {
        let storage = Storage::current();
        let old_path = storage.find_instance(self.name.as_str());
        let new_path = storage.rename_instance_path(self.name.as_str(), new_name.as_str());
        self.name = new_name;
        fs::rename(&old_path, &new_path).raise_with_context(|| {
            format!(
                "renaming instance file {} to {}",
                old_path.display(),
                new_path.display()
            )
        });
        self
    }

    pub fn stop_from_name(name: &str) {
        let name = InstanceName::parse(name).raise();
        let path = Storage::current().find_instance(name.as_str());
        fs::remove_file(&path).raise_with_context(|| format!("deleting instance file {}", path.display()));
    }

    pub fn complete_from_name(name: &str) {
        let instance = Self::load_from_name(name);
        let current_state = instance
            .flow
            .states()
            .iter()
            .find(|state| state.name == instance.state.0)
            .expect("current state was validated");
        if !current_state.next.is_empty() {
            raise::raise(format!(
                "cannot complete instance {:?} in state {:?}: state has next transitions",
                instance.name, instance.state.0
            ));
        }
        Self::stop_from_name(instance.name.as_str());
    }

    pub fn update_context(mut self, update: ContextUpdate) -> Self {
        match update {
            ContextUpdate::Set { key, value } => {
                self.context.insert(key, value);
            }
            ContextUpdate::Remove { key } => {
                if self.context.remove(&key).is_none() {
                    raise::raise(format!(
                        "context key {key:?} not found in instance {:?}",
                        self.name
                    ));
                }
            }
        }
        self.save_existing();
        self
    }

    pub fn set_autonomy(mut self, level: Autonomy, start: &str, end: Option<&str>) -> Vec<String> {
        let affected = self.select_autonomy_states(start, end);
        for name in &affected {
            self.autonomy.insert(name.clone(), level);
        }
        self.save_existing();
        affected
    }

    fn select_autonomy_states(&self, start: &str, end: Option<&str>) -> Vec<String> {
        let states = self.flow.states();
        for name in [Some(start), end].into_iter().flatten() {
            if !states.iter().any(|state| state.name == name) {
                raise::raise(format!("unknown state {name:?} in flow {:?}", self.flow.name()));
            }
        }

        let Some(end) = end else {
            return vec![start.to_owned()];
        };

        states_on_next_paths(states, start, end).unwrap_or_else(|| {
            raise::raise(format!(
                "no next path from {start:?} to {end:?} in flow {:?}",
                self.flow.name()
            ))
        })
    }

    pub fn current_autonomy(&self) -> Autonomy {
        *self
            .autonomy
            .get(&self.state.0)
            .expect("current state autonomy was initialized")
    }

    pub fn save_new(&self, policy: InstanceSavePolicy) -> PathBuf {
        let path = Storage::current().new_instance_path(self.name.as_str(), policy.scope());
        policy.prepare_flow(&self.flow);
        self.save_to_path(&path);
        path
    }

    pub fn load_from_name(name: &str) -> Self {
        let name = InstanceName::parse(name).raise();
        let path = Storage::current().find_instance(name.as_str());
        let mut instance = Self::deserialize_from_path(&path);
        instance.name = name;
        instance.validate_current_state();
        for state in instance.flow.states() {
            instance.autonomy.entry(state.name.clone()).or_default();
        }
        for name in instance.autonomy.keys() {
            if !instance.flow.states().iter().any(|state| &state.name == name) {
                raise::raise(format!(
                    "instance {:?} has autonomy for unknown state {name:?}",
                    instance.name
                ));
            }
        }
        instance
    }

    pub fn load_all() -> Vec<Self> {
        Storage::current()
            .instance_names()
            .iter()
            .map(|name| Self::load_from_name(name.as_str()))
            .collect()
    }

    fn save_existing(&self) {
        let path = Storage::current().find_instance(self.name.as_str());
        FileWriter::from(path).replace_existing(self.serialized_contents().as_bytes());
    }

    fn save_to_path(&self, path: &Path) {
        FileWriter::from(path.to_path_buf()).write_new(self.serialized_contents().as_bytes());
    }

    fn serialized_contents(&self) -> String {
        toml::to_string(self).raise_with_context(|| format!("serializing instance {:?}", self.name))
    }

    fn deserialize_from_path(path: &Path) -> Self {
        let contents = fs::read_to_string(path).raise_with_context(|| format!("reading {}", path.display()));
        toml::from_str(&contents).raise_with_context(|| format!("loading instance {}", path.display()))
    }

    fn validate_current_state(&self) {
        if !self.flow.states().iter().any(|state| state.name == self.state.0) {
            raise::raise(format!(
                "instance {:?} refers to unknown state {:?} in flow {:?}",
                self.name,
                self.state.0,
                self.flow.name()
            ));
        }
    }
}

fn states_on_next_paths(states: &[State], start: &str, end: &str) -> Option<Vec<String>> {
    let mut reachable_from_start = HashSet::new();
    let mut to_visit = vec![start];
    while let Some(name) = to_visit.pop() {
        if !reachable_from_start.insert(name) || name == end {
            continue;
        }
        let state = states
            .iter()
            .find(|state| state.name == name)
            .expect("state was validated");
        to_visit.extend(state.next.iter().map(|next| next.0.as_str()));
    }

    if !reachable_from_start.contains(end) {
        return None;
    }

    let mut can_reach_end = HashSet::new();
    let mut to_visit = vec![end];
    while let Some(name) = to_visit.pop() {
        if !can_reach_end.insert(name) {
            continue;
        }
        to_visit.extend(
            states
                .iter()
                .filter(|state| state.next.iter().any(|next| next.0 == name))
                .map(|state| state.name.as_str()),
        );
    }
    Some(
        states
            .iter()
            .filter(|state| {
                reachable_from_start.contains(state.name.as_str())
                    && can_reach_end.contains(state.name.as_str())
            })
            .map(|state| state.name.clone())
            .collect(),
    )
}

fn load_flow<'de, D>(deserializer: D) -> std::result::Result<Flow, D::Error>
where
    D: Deserializer<'de>,
{
    let name = String::deserialize(deserializer)?;
    let name = FlowName::parse(name).map_err(serde::de::Error::custom)?;
    Ok(Flow::load_from_name(name.as_str()))
}

fn save_flow_name<S>(flow: &Flow, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(flow.name().as_str())
}

#[cfg(test)]
mod tests {
    use super::{Flow, Instance, Move, StateName, states_on_next_paths};
    use crate::{FlowName, InstanceName, State};
    use indoc::indoc;
    use rust_utils::raise::catch_raised;

    fn raised_message<T: std::fmt::Debug>(operation: impl FnOnce() -> T) -> String {
        catch_raised(std::panic::AssertUnwindSafe(operation))
            .unwrap_err()
            .to_string()
    }

    fn state(name: &str, next: Vec<StateName>) -> State {
        State { name: name.to_owned(), next, summary: None, details: None, steps: Vec::new() }
    }

    fn instance_with_transitions() -> Instance {
        Instance::new(
            InstanceName::parse("run").unwrap(),
            Flow::new(
                FlowName::parse("workflow").unwrap(),
                StateName("Draft".into()),
                vec![
                    state("Draft", vec![StateName("Review".into())]),
                    state("Review", vec![]),
                    state("Done", vec![]),
                ],
            ),
        )
    }

    #[test]
    fn next_path_selection_includes_branches_and_loops_before_end() {
        let states = vec![
            state("End", vec![StateName("After".into())]),
            state(
                "Start",
                vec![
                    StateName("Left".into()),
                    StateName("Right".into()),
                    StateName("Dead".into()),
                ],
            ),
            state("Left", vec![StateName("Loop".into()), StateName("End".into())]),
            state("Right", vec![StateName("End".into())]),
            state("Dead", vec![]),
            state("Loop", vec![StateName("Left".into())]),
            state("After", vec![StateName("End".into())]),
        ];

        assert_eq!(
            states_on_next_paths(&states, "Start", "End"),
            Some(
                vec!["End", "Start", "Left", "Right", "Loop"]
                    .into_iter()
                    .map(String::from)
                    .collect()
            )
        );
        assert_eq!(
            states_on_next_paths(&states, "Start", "Dead"),
            Some(vec!["Start", "Dead"].into_iter().map(String::from).collect())
        );
        assert_eq!(states_on_next_paths(&states, "Dead", "End"), None);
        assert_eq!(
            states_on_next_paths(&states, "End", "End"),
            Some(vec!["End".into()])
        );
    }

    #[test]
    fn next_follows_a_listed_transition() {
        let mut instance = instance_with_transitions();
        instance.apply_move(Move::Next(StateName("Review".into())));
        assert_eq!(instance.state.0, "Review");
    }

    #[test]
    fn next_rejects_an_unlisted_transition_without_changing_state() {
        let mut instance = instance_with_transitions();
        let error = raised_message(|| instance.apply_move(Move::Next(StateName("Done".into()))));
        assert!(error.contains("cannot move next"));
        assert_eq!(instance.state.0, "Draft");
    }

    #[test]
    fn jump_can_bypass_listed_transitions() {
        let mut instance = instance_with_transitions();
        instance.apply_move(Move::JumpTo(StateName("Done".into())));
        assert_eq!(instance.state.0, "Done");
    }

    #[test]
    fn jump_rejects_unknown_state_without_changing_state() {
        let mut instance = instance_with_transitions();
        let error = raised_message(|| instance.apply_move(Move::JumpTo(StateName("Missing".into()))));
        assert!(error.contains("unknown state"));
        assert_eq!(instance.state.0, "Draft");
    }

    #[test]
    fn constructor_rejects_invalid_flow() {
        let flow: Flow = toml::from_str(indoc! {r#"
            initial_state = "Missing"
            [[states]]
            name = "Design"
            next = []
        "#})
        .unwrap();

        assert!(
            raised_message(|| Instance::new(InstanceName::parse("run").unwrap(), flow))
                .contains("unknown initial state")
        );
    }

    #[test]
    fn verifies_current_state_belongs_to_flow() {
        let mut instance = instance_with_transitions();
        instance.state = StateName("Missing".into());
        assert!(raised_message(|| instance.validate_current_state()).contains("Missing"));
    }
}
