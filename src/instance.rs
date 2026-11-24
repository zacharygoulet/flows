use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs,
    path::{Path, PathBuf},
};

use crate::{
    Flow, FlowName, InstanceName, Step, StepName,
    file_writer::FileWriter,
    storage::{Scope, Storage},
};
use rust_utils::raise::{self, RaiseContext, RaiseExt};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Deserialize, Serialize, getset::Getters)]
#[serde(deny_unknown_fields)]
#[getset(get = "pub")]
pub struct Instance {
    #[serde(skip, default = "InstanceName::placeholder")]
    name: InstanceName,
    #[serde(deserialize_with = "load_flow", serialize_with = "save_flow_name")]
    flow: Flow,
    step: StepName,
    #[serde(default, skip_serializing_if = "StepOverrides::is_empty")]
    step_overrides: StepOverrides,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    context: BTreeMap<String, String>,
    #[serde(default)]
    autonomy: BTreeMap<String, Autonomy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    owner: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct StepOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    initial: Option<StepName>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    finals: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    skipped: BTreeSet<String>,
}

impl StepOverrides {
    fn is_empty(&self) -> bool {
        self.initial.is_none() && self.finals.is_empty() && self.skipped.is_empty()
    }
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
    Next(StepName),
    JumpTo(StepName),
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

    pub fn status_for_step(&self, name: &str) -> String {
        self.require_step(name);
        format!("{}", crate::output::StepPreview(self, name))
    }

    pub fn new(name: InstanceName, flow: Flow) -> Self {
        flow.validate();
        let step = flow.initial_step().clone();
        let autonomy = flow
            .steps()
            .iter()
            .map(|step| (step.name.clone(), step.autonomy))
            .collect();
        let instance = Self {
            name,
            flow,
            step,
            step_overrides: StepOverrides::default(),
            context: BTreeMap::new(),
            autonomy,
            owner: None,
        };
        instance.validate_current_step();
        instance
    }

    pub fn configured_initial(&self) -> &StepName {
        self.step_overrides
            .initial
            .as_ref()
            .unwrap_or_else(|| self.flow.initial_step())
    }

    pub(crate) fn is_final(&self, name: &str) -> bool {
        self.step_overrides.finals.contains(name)
            || self.find_step(name).is_some_and(|step| step.next.is_empty())
    }

    pub fn effective_next(&self) -> Vec<&Step> {
        self.effective_next_for(&self.step.0)
    }

    pub fn effective_next_for(&self, name: &str) -> Vec<&Step> {
        if self.is_final(name) {
            return Vec::new();
        }
        let current = self.find_step(name).expect("step was validated");
        let mut visited = HashSet::new();
        let mut result = Vec::new();
        for next in &current.next {
            self.expand_next(&next.0, &mut visited, &mut result);
        }
        result
    }

    fn expand_next<'a>(&'a self, name: &str, visited: &mut HashSet<String>, result: &mut Vec<&'a Step>) {
        if !visited.insert(name.to_owned()) {
            return;
        }
        let step = self.find_step(name).expect("flow transitions were validated");
        if self.step_overrides.skipped.contains(name) {
            if !self.step_overrides.finals.contains(name) {
                for next in &step.next {
                    self.expand_next(&next.0, visited, result);
                }
            }
        } else {
            result.push(step);
        }
    }

    fn find_step(&self, name: &str) -> Option<&Step> {
        self.flow.steps().iter().find(|step| step.name == name)
    }

    fn require_step(&self, name: &str) {
        if self.find_step(name).is_none() {
            raise::raise(format!("unknown step {name:?} in flow {:?}", self.flow.name()));
        }
    }

    pub fn set_initial(mut self, name: String) -> Self {
        self.require_step(&name);
        if self.step_overrides.skipped.contains(&name) {
            raise::raise(format!("cannot set skipped step {name:?} as initial"));
        }
        self.step_overrides.initial = (name != self.flow.initial_step().0).then_some(StepName(name));
        self.save_existing();
        self
    }

    pub fn reset(mut self) -> Self {
        self.step = self.configured_initial().clone();
        self.save_existing();
        self
    }

    pub fn add_final(mut self, name: String) -> Self {
        self.require_step(&name);
        if self.step_overrides.skipped.contains(&name) {
            raise::raise(format!("cannot make skipped step {name:?} final"));
        }
        self.step_overrides.finals.insert(name);
        self.save_existing();
        self
    }

    pub fn remove_final(mut self, name: &str) -> Self {
        self.require_step(name);
        if !self.step_overrides.finals.remove(name) {
            raise::raise(format!("step {name:?} is not a custom final step"));
        }
        self.save_existing();
        self
    }

    pub fn skip(mut self, name: String) -> Self {
        self.require_step(&name);
        if name == self.step.0 || name == self.configured_initial().0 || self.is_final(&name) {
            raise::raise(format!("cannot skip current, initial, or final step {name:?}"));
        }
        self.step_overrides.skipped.insert(name);
        self.save_existing();
        self
    }

    pub fn unskip(mut self, name: &str) -> Self {
        self.require_step(name);
        if !self.step_overrides.skipped.remove(name) {
            raise::raise(format!("step {name:?} is not skipped"));
        }
        self.save_existing();
        self
    }

    pub fn apply_move(&mut self, movement: Move) {
        let target = match movement {
            Move::Next(target) => {
                if !self.effective_next().iter().any(|next| next.name == target.0) {
                    raise::raise(format!(
                        "step {:?} cannot move next to {:?} in flow {:?}",
                        self.step.0,
                        target.0,
                        self.flow.name()
                    ));
                }
                target
            }
            Move::JumpTo(target) => {
                self.require_step(&target.0);
                if self.step_overrides.skipped.contains(&target.0) {
                    raise::raise(format!("cannot jump to skipped step {:?}", target.0));
                }
                target
            }
        };
        self.step = target;
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
        if !instance.is_final(&instance.step.0) {
            raise::raise(format!(
                "cannot complete instance {:?} in step {:?}: step has next transitions",
                instance.name, instance.step.0
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
        let affected = self.select_autonomy_steps(start, end);
        for name in &affected {
            self.autonomy.insert(name.clone(), level);
        }
        self.save_existing();
        affected
    }

    fn select_autonomy_steps(&self, start: &str, end: Option<&str>) -> Vec<String> {
        let steps = self.flow.steps();
        for name in [Some(start), end].into_iter().flatten() {
            if !steps.iter().any(|step| step.name == name) {
                raise::raise(format!("unknown step {name:?} in flow {:?}", self.flow.name()));
            }
        }

        let Some(end) = end else {
            return vec![start.to_owned()];
        };

        steps_on_next_paths(steps, start, end).unwrap_or_else(|| {
            raise::raise(format!(
                "no next path from {start:?} to {end:?} in flow {:?}",
                self.flow.name()
            ))
        })
    }

    pub fn current_autonomy(&self) -> Autonomy {
        self.autonomy_for(&self.step.0)
    }

    pub fn autonomy_for(&self, name: &str) -> Autonomy {
        *self.autonomy.get(name).expect("step autonomy was initialized")
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
        instance.validate_current_step();
        instance.validate_configuration();
        for step in instance.flow.steps() {
            instance
                .autonomy
                .entry(step.name.clone())
                .or_insert(step.autonomy);
        }
        for name in instance.autonomy.keys() {
            if !instance.flow.steps().iter().any(|step| &step.name == name) {
                raise::raise(format!(
                    "instance {:?} has autonomy for unknown step {name:?}",
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

    fn validate_configuration(&self) {
        self.require_step(&self.configured_initial().0);
        for name in &self.step_overrides.finals {
            self.require_step(name);
        }
        for name in &self.step_overrides.skipped {
            self.require_step(name);
            if name == &self.step.0 || name == &self.configured_initial().0 || self.is_final(name) {
                raise::raise(format!(
                    "instance {:?} has invalid skipped step {name:?}: current, initial, or final",
                    self.name
                ));
            }
        }
    }

    fn validate_current_step(&self) {
        if !self.flow.steps().iter().any(|step| step.name == self.step.0) {
            raise::raise(format!(
                "instance {:?} refers to unknown step {:?} in flow {:?}",
                self.name,
                self.step.0,
                self.flow.name()
            ));
        }
    }
}

fn steps_on_next_paths(steps: &[Step], start: &str, end: &str) -> Option<Vec<String>> {
    let mut reachable_from_start = HashSet::new();
    let mut to_visit = vec![start];
    while let Some(name) = to_visit.pop() {
        if !reachable_from_start.insert(name) || name == end {
            continue;
        }
        let step = steps
            .iter()
            .find(|step| step.name == name)
            .expect("step was validated");
        to_visit.extend(step.next.iter().map(|next| next.0.as_str()));
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
            steps
                .iter()
                .filter(|step| step.next.iter().any(|next| next.0 == name))
                .map(|step| step.name.as_str()),
        );
    }
    Some(
        steps
            .iter()
            .filter(|step| {
                reachable_from_start.contains(step.name.as_str())
                    && can_reach_end.contains(step.name.as_str())
            })
            .map(|step| step.name.clone())
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
    use super::{Flow, Instance, Move, StepName, steps_on_next_paths};
    use crate::{FlowName, InstanceName, Step};
    use indoc::indoc;
    use rust_utils::raise::catch_raised;

    fn raised_message<T: std::fmt::Debug>(operation: impl FnOnce() -> T) -> String {
        catch_raised(std::panic::AssertUnwindSafe(operation))
            .unwrap_err()
            .to_string()
    }

    fn step(name: &str, next: Vec<StepName>) -> Step {
        Step {
            name: name.to_owned(),
            autonomy: Default::default(),
            next,
            summary: None,
            details: None,
            checklist: Vec::new(),
        }
    }

    fn instance_with_transitions() -> Instance {
        Instance::new(
            InstanceName::parse("run").unwrap(),
            Flow::new(
                FlowName::parse("workflow").unwrap(),
                StepName("Draft".into()),
                vec![
                    step("Draft", vec![StepName("Review".into())]),
                    step("Review", vec![]),
                    step("Done", vec![]),
                ],
            ),
        )
    }

    #[test]
    fn next_path_selection_includes_branches_and_loops_before_end() {
        let steps = vec![
            step("End", vec![StepName("After".into())]),
            step(
                "Start",
                vec![
                    StepName("Left".into()),
                    StepName("Right".into()),
                    StepName("Dead".into()),
                ],
            ),
            step("Left", vec![StepName("Loop".into()), StepName("End".into())]),
            step("Right", vec![StepName("End".into())]),
            step("Dead", vec![]),
            step("Loop", vec![StepName("Left".into())]),
            step("After", vec![StepName("End".into())]),
        ];

        assert_eq!(
            steps_on_next_paths(&steps, "Start", "End"),
            Some(
                vec!["End", "Start", "Left", "Right", "Loop"]
                    .into_iter()
                    .map(String::from)
                    .collect()
            )
        );
        assert_eq!(
            steps_on_next_paths(&steps, "Start", "Dead"),
            Some(vec!["Start", "Dead"].into_iter().map(String::from).collect())
        );
        assert_eq!(steps_on_next_paths(&steps, "Dead", "End"), None);
        assert_eq!(
            steps_on_next_paths(&steps, "End", "End"),
            Some(vec!["End".into()])
        );
    }

    #[test]
    fn next_follows_a_listed_transition() {
        let mut instance = instance_with_transitions();
        instance.apply_move(Move::Next(StepName("Review".into())));
        assert_eq!(instance.step.0, "Review");
    }

    #[test]
    fn next_rejects_an_unlisted_transition_without_changing_step() {
        let mut instance = instance_with_transitions();
        let error = raised_message(|| instance.apply_move(Move::Next(StepName("Done".into()))));
        assert!(error.contains("cannot move next"));
        assert_eq!(instance.step.0, "Draft");
    }

    #[test]
    fn jump_can_bypass_listed_transitions() {
        let mut instance = instance_with_transitions();
        instance.apply_move(Move::JumpTo(StepName("Done".into())));
        assert_eq!(instance.step.0, "Done");
    }

    #[test]
    fn jump_rejects_unknown_step_without_changing_step() {
        let mut instance = instance_with_transitions();
        let error = raised_message(|| instance.apply_move(Move::JumpTo(StepName("Missing".into()))));
        assert!(error.contains("unknown step"));
        assert_eq!(instance.step.0, "Draft");
    }

    #[test]
    fn constructor_rejects_invalid_flow() {
        let flow: Flow = toml::from_str(indoc! {r#"
            initial_step = "Missing"
            [[steps]]
            name = "Design"
            next = []
        "#})
        .unwrap();

        assert!(
            raised_message(|| Instance::new(InstanceName::parse("run").unwrap(), flow))
                .contains("unknown initial step")
        );
    }

    #[test]
    fn verifies_current_step_belongs_to_flow() {
        let mut instance = instance_with_transitions();
        instance.step = StepName("Missing".into());
        assert!(raised_message(|| instance.validate_current_step()).contains("Missing"));
    }

    #[test]
    fn skipped_branches_expand_in_order_once_and_survive_cycles() {
        let mut instance = Instance::new(
            InstanceName::parse("run").unwrap(),
            Flow::new(
                FlowName::parse("workflow").unwrap(),
                StepName("Start".into()),
                vec![
                    step(
                        "Start",
                        vec![StepName("A".into()), StepName("B".into()), StepName("End".into())],
                    ),
                    step("A", vec![StepName("B".into()), StepName("End".into())]),
                    step("B", vec![StepName("A".into()), StepName("End".into())]),
                    step("End", vec![]),
                ],
            ),
        );
        instance.step_overrides.skipped.extend(["A".into(), "B".into()]);
        assert_eq!(
            instance
                .effective_next()
                .iter()
                .map(|step| step.name.as_str())
                .collect::<Vec<_>>(),
            vec!["End"]
        );
        assert!(
            raised_message(|| instance.apply_move(Move::Next(StepName("A".into()))))
                .contains("cannot move next")
        );
        assert!(
            raised_message(|| instance.apply_move(Move::JumpTo(StepName("B".into())))).contains("skipped")
        );
        instance.apply_move(Move::Next(StepName("End".into())));
        assert!(instance.effective_next().is_empty());
    }

    #[test]
    fn self_transition_through_skipped_step_is_available() {
        let mut instance = Instance::new(
            InstanceName::parse("run").unwrap(),
            Flow::new(
                FlowName::parse("workflow").unwrap(),
                StepName("Start".into()),
                vec![
                    step("Start", vec![StepName("Loop".into())]),
                    step("Loop", vec![StepName("Start".into())]),
                ],
            ),
        );
        instance.step_overrides.skipped.insert("Loop".into());
        assert_eq!(instance.effective_next()[0].name, "Start");
        instance.apply_move(Move::Next(StepName("Start".into())));
    }

    #[test]
    fn skipped_cycle_without_a_final_step_is_not_completable() {
        let mut instance = Instance::new(
            InstanceName::parse("run").unwrap(),
            Flow::new(
                FlowName::parse("workflow").unwrap(),
                StepName("Start".into()),
                vec![
                    step("Start", vec![StepName("Loop".into())]),
                    step("Loop", vec![StepName("Loop".into())]),
                ],
            ),
        );
        instance.step_overrides.skipped.insert("Loop".into());
        assert!(instance.effective_next().is_empty());
        assert!(!instance.is_final("Start"));
    }

    #[test]
    fn custom_final_truncates_transitions_and_default_terminal_remains_terminal() {
        let mut instance = instance_with_transitions();
        instance.step_overrides.finals.insert("Draft".into());
        assert!(instance.effective_next().is_empty());
        instance.step_overrides.finals.clear();
        assert_eq!(instance.effective_next()[0].name, "Review");
        instance.apply_move(Move::JumpTo(StepName("Done".into())));
        assert!(instance.effective_next().is_empty());
    }
}
