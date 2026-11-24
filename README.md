# flows

The `flows` tool helps AI agents follow configured, deterministic flows. It
tracks progress; the agent does the work.

A flow describes steps, their guidance, and normal next transitions. Start an
instance from a flow, use status to read its current guidance, move with next,
or use jump for exceptional moves.

Use `flows next <instance-name> [target]` to move to a listed next step.
Omit the target to select the first available transition, expanding skipped
steps in order. The instance name is always required.

Instances can be assigned autonomy level per step, per-instance context, and an
optional informational owner. Ownership does not block operations.

Context is a persistent set of compact reminders or pointers for later steps,
such as an issue ID or plan URL. Values are limited to 150 characters; keep
detailed plans, requirements, and notes in their proper documents instead.

Each `[[steps]]` entry can set `autonomy = "guided"`, `"steered"`, or
`"autonomous"`; omitted values default to guided. Guided autonomy confirms
step boundaries and each emerging design choice. Steered autonomy proceeds with
routine work and asks on consequential choices. Autonomous autonomy works independently
and escalates fundamental blockers or reserved decisions.
New instances save these defaults per step. Loading older instances fills only
missing autonomy entries from the flow defaults; saved values are preserved even
if the flow defaults change. Override saved levels with `flows autonomy set`.

Use `flows status` to see available instances.
Use `flows start` to start a new instance.
Use `flows resume` to claim an unowned instance.
Use `flows rename` to give an instance a descriptive name, including before pausing it for later continuation.
Use `flows pause` to release ownership. Consider adding handoff context first and renaming the instance.
Use `flows rename <current-name> <new-name>` to rename an instance.
Use `flows stop` to delete an instance.
Use `flows complete <name>` to delete an instance after it reaches a step with no available next transitions.
Use `flows help <command>` for a command's arguments and options.

An instance can change its initial step, add final steps, or skip steps without
changing its flow. See `flows step-overrides --help`.

Use `flows list steps <flow-name>` to list a flow's steps. Guidance at global,
flow, and step levels uses `checklist = ["instruction", ...]`. Checklist items
are instructions, not individually tracked work.

## Migrating existing configuration

This terminology change is breaking: old schema keys and CLI commands are not
accepted, and no automatic migration or aliases are provided. Update existing
flow and instance TOML explicitly before using them:

- Flow: `initial_state` → `initial_step`, `[[states]]` → `[[steps]]`.
- Guidance at flow and step levels: `steps = [...]` → `checklist = [...]`.
- Instance: `state` → `step`, `[state_overrides]` → `[step_overrides]`.
- CLI: `list states` → `list steps`, `state-overrides` → `step-overrides`.

Step names, `next` destinations, autonomy-map keys, flow names, and storage
locations remain unchanged. Rename keys rather than changing their values;
update both local and global files where applicable.
