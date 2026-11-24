# flow

The `flow` tool helps AI agents follow configured, deterministic flows. It
tracks progress; the agent does the work.

A flow describes steps, their guidance, and normal next transitions. Start an
instance from a flow, use show to read its current guidance, move with next,
or use jump for exceptional moves.

Use `flow next <instance-name> [target]` to move to a listed next step.
Omit the target only when exactly one effective destination is available,
expanding skipped steps in order. The instance name is always required.

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
if the flow defaults change. Override saved levels with
`flow autonomy set <instance-name> <level> --step <step>` or
`flow autonomy set <instance-name> <level> --from <first> --through <last>`.

Use `flow list flows` to see available flows.
Use `flow list instances` to see available instances. Bare `flow list` shows help.
Use `flow create-flow <flow-name>` to create a flow from the bundled template.
Use `flow show <instance-name>` to read current guidance, or add `--step <step>` to preview another step without moving.
Use `flow start` to start a new instance.
Use `flow claim` to claim an unowned instance.
Use `flow rename` to give an instance a descriptive name, including before releasing it for later continuation.
Use `flow release` to release ownership. Consider adding handoff context first and renaming the instance.
Use `flow rename <current-name> <new-name>` to rename an instance.
Use `flow delete` to delete an instance.
Use `flow complete <name>` to delete an instance after it reaches a natural terminal or custom final step.
Use `flow help <command>` for a command's arguments and options.

An instance can change its initial step, add final steps, or skip steps without
changing its flow. See `flow route --help`. `flow route initial set <instance-name> <step>`
configures the initial step without moving; `flow reset <instance-name>` moves there explicitly.

Use `flow list steps <flow-name>` to list a flow's steps. Guidance at global,
flow, and step levels uses `checklist = ["instruction", ...]`. Checklist items
are instructions, not individually tracked work.

## Migrating existing configuration

These changes are breaking: old schema keys and CLI commands are not
accepted, and no automatic migration or aliases are provided. Update existing
flow and instance TOML explicitly before using them:

- Flow: `initial_state` → `initial_step`, `[[states]]` → `[[steps]]`.
- Guidance at flow and step levels: `steps = [...]` → `checklist = [...]`.
- Instance: `state` → `step`, `[state_overrides]` → `[step_overrides]`.
- CLI: `list states` → `list steps`, `state-overrides`/`step-overrides` → `route`.
- Executable: `flows` → `flow`.
- Storage: `.flows` → `.flow`, `$XDG_STATE_HOME/flows` → `$XDG_STATE_HOME/flow`
  (defaulting to `~/.local/state/flow`). Old storage paths are not read or migrated.

Step names, `next` destinations, autonomy-map keys, and flow names remain unchanged.
Rename keys rather than changing their values; update both local and global files
where applicable.
