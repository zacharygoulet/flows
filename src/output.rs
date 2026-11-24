use std::{
    fmt::{self, Write},
    path::Path,
};

use crate::{Autonomy, Flow, FlowName, GlobalGuidance, Instance};

pub struct InstancesTable<'a>(pub &'a [Instance]);
pub struct FlowsTable<'a>(pub &'a [Flow]);
pub struct StepsTable<'a>(pub &'a Flow);

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
            "{:<instance_width$}  {:<flow_width$}  {:<owner_width$}  STEP",
            "INSTANCE", "FLOW", "OWNER"
        )?;
        writeln!(
            formatter,
            "{}  {}  {}  ----",
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
                instance.step().0
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

impl fmt::Display for StepsTable<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_list_table(
            formatter,
            "STEP",
            self.0
                .steps()
                .iter()
                .map(|step| (step.name.as_str(), step.summary.as_deref().unwrap_or(""))),
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
        "instance {} is now in step {}",
        instance.name(),
        instance.step().0
    )
}

pub fn autonomy_changed(level: Autonomy, changed: &[String]) -> String {
    let mut message = format!("set autonomy to {level} for steps:\n");
    for name in changed {
        writeln!(message, "  - {name}").expect("writing to a String cannot fail");
    }
    message.push_str("Interview the user about upcoming decisions when the selected autonomy requires it.\n");
    message
}

pub struct InstanceStatus<'a>(pub &'a Instance);
pub struct StepPreview<'a>(pub &'a Instance, pub &'a str);

impl fmt::Display for InstanceStatus<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_status(formatter, self.0, None)
    }
}

impl fmt::Display for StepPreview<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_status(formatter, self.0, Some(self.1))
    }
}

fn write_status(
    formatter: &mut fmt::Formatter<'_>,
    instance: &Instance,
    preview: Option<&str>,
) -> fmt::Result {
    let flow = instance.flow();
    let current = flow
        .steps()
        .iter()
        .find(|step| step.name == instance.step().0)
        .expect("the instance step was validated when it was loaded");
    let step = preview
        .map(|name| {
            flow.steps()
                .iter()
                .find(|step| step.name == name)
                .expect("preview step was validated")
        })
        .unwrap_or(current);

    writeln!(
        formatter,
        "{}",
        if preview.is_some() {
            "Step Preview"
        } else {
            "Instance Status"
        }
    )?;
    writeln!(formatter)?;
    write_summary_line(formatter, "flow", flow.name().as_str(), flow.summary().as_deref())?;
    write_summary_line(
        formatter,
        "current step",
        &current.name,
        current.summary.as_deref(),
    )?;
    if preview.is_some() {
        write_summary_line(formatter, "inspected step", &step.name, step.summary.as_deref())?;
    }
    let autonomy = instance.autonomy_for(&step.name);
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
        formatter
            .write_str(&toml::to_string(instance.context()).expect("context strings serialize to TOML"))?;
    }

    let global_guidance = flow.use_global().then(GlobalGuidance::bundled);
    if let Some(guidance) = &global_guidance {
        write_section(formatter, "Global details", guidance.details().as_deref())?;
    }
    write_section(formatter, "Flow details", flow.details().as_deref())?;
    write_section(formatter, "Step details", step.details.as_deref())?;
    if let Some(guidance) = &global_guidance {
        write_checklist_section(formatter, "Global checklist", guidance.checklist())?;
    }
    write_checklist_section(formatter, "Flow checklist", flow.checklist())?;
    write_checklist_section(formatter, "Step checklist", &step.checklist)?;

    let next_steps = instance.effective_next_for(&step.name);
    if !next_steps.is_empty() {
        writeln!(formatter)?;
        writeln!(formatter, "Next steps:")?;
        for next in next_steps {
            match next.summary.as_deref() {
                Some(summary) => writeln!(formatter, "  - {}: {summary}", next.name)?,
                None => writeln!(formatter, "  - {}", next.name)?,
            }
        }
    } else if step.name == current.name && instance.is_final(&step.name) {
        writeln!(formatter)?;
        writeln!(formatter, "Next steps:")?;
        writeln!(
            formatter,
            "  This is the final step. Run `flow complete {}` to complete the flow.",
            instance.name()
        )?;
    }
    Ok(())
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
        Autonomy::Guided => "Confirm step boundaries and each emerging design choice.",
        Autonomy::Steered => "Proceed with routine work; ask on consequential choices.",
        Autonomy::Autonomous => "Work independently; escalate fundamental blockers or reserved decisions.",
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

fn write_checklist_section(
    formatter: &mut fmt::Formatter<'_>,
    label: &str,
    checklist: &[String],
) -> fmt::Result {
    if checklist.is_empty() {
        return Ok(());
    }
    writeln!(formatter)?;
    writeln!(formatter, "{label}:")?;
    for item in checklist {
        writeln!(formatter, "- {item}")?;
    }
    Ok(())
}
