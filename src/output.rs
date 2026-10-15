use std::{
    fmt::{self, Write},
    path::Path,
};

use crate::{Autonomy, Flow, FlowName, GlobalGuidance, Instance};

pub struct InstancesTable<'a>(pub &'a [Instance]);
pub struct FlowsTable<'a>(pub &'a [Flow]);
pub struct StatesTable<'a>(pub &'a Flow);

impl fmt::Display for InstancesTable<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let instances = self.0;
        if instances.is_empty() {
            return writeln!(formatter, "no instances found");
        }

        let instance_width = instances
            .iter()
            .map(|instance| instance.name().as_str().len())
            .max()
            .unwrap_or(0)
            .max("INSTANCE".len());
        let flow_width = instances
            .iter()
            .map(|instance| instance.flow().name().as_str().len())
            .max()
            .unwrap_or(0)
            .max("FLOW".len());
        let owner_width = instances
            .iter()
            .map(|instance| instance.owner().as_deref().unwrap_or("unowned").len())
            .max()
            .unwrap_or(0)
            .max("OWNER".len());
        writeln!(
            formatter,
            "{:<instance_width$}  {:<flow_width$}  {:<owner_width$}  STATE",
            "INSTANCE", "FLOW", "OWNER"
        )?;
        writeln!(
            formatter,
            "{}  {}  {}  -----",
            "-".repeat(instance_width),
            "-".repeat(flow_width),
            "-".repeat(owner_width)
        )?;
        for instance in instances {
            writeln!(
                formatter,
                "{:<instance_width$}  {:<flow_width$}  {:<owner_width$}  {}",
                instance.name(),
                instance.flow().name(),
                instance.owner().as_deref().unwrap_or("unowned"),
                instance.state().0
            )?;
        }
        Ok(())
    }
}

impl fmt::Display for FlowsTable<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return writeln!(formatter, "no flows found");
        }
        write_list_table(
            formatter,
            "FLOW",
            self.0
                .iter()
                .map(|flow| (flow.name().as_str(), flow.summary().as_deref().unwrap_or(""))),
        )
    }
}

impl fmt::Display for StatesTable<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_list_table(
            formatter,
            "STATE",
            self.0
                .states()
                .iter()
                .map(|state| (state.name.as_str(), state.summary.as_deref().unwrap_or(""))),
        )
    }
}

fn write_list_table<'a>(
    formatter: &mut fmt::Formatter<'_>,
    header: &str,
    rows: impl Iterator<Item = (&'a str, &'a str)>,
) -> fmt::Result {
    let rows: Vec<_> = rows.collect();
    let width = rows
        .iter()
        .map(|(name, _)| name.chars().count())
        .max()
        .unwrap_or(0)
        .max(header.len());
    writeln!(formatter, "{header:<width$}  SUMMARY")?;
    writeln!(formatter, "{}  -------", "-".repeat(width))?;
    for (name, summary) in rows {
        if summary.is_empty() {
            writeln!(formatter, "{name}")?;
        } else {
            writeln!(formatter, "{name:<width$}  {summary}")?;
        }
    }
    Ok(())
}

pub fn created_instance(instance: &Instance, path: &Path) -> String {
    format!("created instance {} at {}", instance.name(), path.display())
}

pub fn created_flow(name: &FlowName, path: &Path) -> String {
    format!("created flow {} at {}", name, path.display())
}

pub fn moved_instance(instance: &Instance) -> String {
    format!(
        "instance {} is now in state {}",
        instance.name(),
        instance.state().0
    )
}

pub fn autonomy_changed(level: Autonomy, changed: &[String]) -> String {
    let mut message = format!("set autonomy to {level} for states:\n");
    for name in changed {
        writeln!(message, "  - {name}").expect("writing to a String cannot fail");
    }
    message
}

pub struct InstanceStatus<'a>(pub &'a Instance);

impl fmt::Display for InstanceStatus<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let instance = self.0;
        let flow = instance.flow();
        let state = flow
            .states()
            .iter()
            .find(|state| state.name == instance.state().0)
            .expect("the instance state was validated when it was loaded");

        writeln!(formatter, "Instance Status")?;
        writeln!(formatter)?;
        write_summary_line(formatter, "flow", flow.name().as_str(), flow.summary().as_deref())?;
        write_summary_line(formatter, "current state", &state.name, state.summary.as_deref())?;
        let autonomy = instance.current_autonomy();
        writeln!(
            formatter,
            "autonomy: {autonomy} ({})",
            autonomy_description(autonomy)
        )?;
        writeln!(
            formatter,
            "owner: {}",
            instance.owner().as_deref().unwrap_or("unowned")
        )?;

        if !instance.context().is_empty() {
            writeln!(formatter, "context:")?;
            formatter.write_str(
                &toml::to_string(instance.context()).expect("context strings serialize to TOML"),
            )?;
        }

        let global_guidance = flow.use_global().then(GlobalGuidance::bundled);
        if let Some(guidance) = &global_guidance {
            write_section(formatter, "Global details", guidance.details().as_deref())?;
        }
        write_section(formatter, "Flow details", flow.details().as_deref())?;
        write_section(formatter, "State details", state.details.as_deref())?;
        if let Some(guidance) = &global_guidance {
            write_steps_section(formatter, "Global steps", guidance.steps())?;
        }
        write_steps_section(formatter, "Flow steps", flow.steps())?;
        write_steps_section(formatter, "State steps", &state.steps)?;

        if !state.next.is_empty() {
            writeln!(formatter)?;
            writeln!(formatter, "Next states:")?;
            for next in &state.next {
                let next_state = flow
                    .states()
                    .iter()
                    .find(|candidate| candidate.name == next.0)
                    .expect("flow transitions were validated when it was loaded");
                match next_state.summary.as_deref() {
                    Some(summary) => writeln!(formatter, "  - {}: {summary}", next.0)?,
                    None => writeln!(formatter, "  - {}", next.0)?,
                }
            }
        }
        Ok(())
    }
}

fn write_summary_line(
    formatter: &mut fmt::Formatter<'_>,
    label: &str,
    value: &str,
    summary: Option<&str>,
) -> fmt::Result {
    match summary {
        Some(summary) => writeln!(formatter, "{label}: {value} ({summary})"),
        None => writeln!(formatter, "{label}: {value}"),
    }
}

fn autonomy_description(autonomy: Autonomy) -> &'static str {
    match autonomy {
        Autonomy::Guided => "Pause for user approval at state boundaries and significant decisions.",
        Autonomy::Steered => "Proceed by default; ask on consequential choices and report progress.",
        Autonomy::Autonomous => "Continue independently; stop only for fundamental blockers.",
    }
}

fn write_section(formatter: &mut fmt::Formatter<'_>, label: &str, value: Option<&str>) -> fmt::Result {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    writeln!(formatter)?;
    writeln!(formatter, "{label}:")?;
    writeln!(formatter, "{}", value.trim_end_matches('\n'))
}

fn write_steps_section(formatter: &mut fmt::Formatter<'_>, label: &str, steps: &[String]) -> fmt::Result {
    if steps.is_empty() {
        return Ok(());
    }
    writeln!(formatter)?;
    writeln!(formatter, "{label}:")?;
    for step in steps {
        writeln!(formatter, "- {step}")?;
    }
    Ok(())
}
