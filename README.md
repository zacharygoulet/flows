# flows

The `flows` tool helps AI agents follow configured, deterministic flows. It
tracks progress; the agent does the work.

A flow describes states, their guidance, and normal next transitions. Start an
instance from a flow, use status to read its current guidance, move with next,
or use jump for exceptional moves.

Instances can be assigned autonomy level per state, per-instance context, and an
optional informational owner. Ownership does not block operations.

Context is a persistent set of compact reminders or pointers for later states,
such as an issue ID or plan URL. Values are limited to 150 characters; keep
detailed plans, requirements, and notes in their proper documents instead.

Each state starts with guided autonomy: pause for approval at state boundaries
and significant decisions. Steered autonomy proceeds by default, asking on
consequential choices and reporting progress. Autonomous autonomy works
independently, stopping only for fundamental blockers. Set each level with
autonomy set.

Use `flows status` to see available instances.
Use `flows start` to start a new instance.
Use `flows resume` to claim an unowned instance.
Use `flows rename` to give an instance a descriptive name, including before pausing it for later continuation.
Use `flows pause` to release ownership. Consider adding handoff context first and renaming the instance.
Use `flows rename <current-name> <new-name>` to rename an instance.
Use `flows stop` to delete an instance.
Use `flows complete <name>` to delete an instance after it reaches a state with no available next transitions.
Use `flows help <command>` for a command's arguments and options.

An instance can change its initial state, add final states, or skip states without
changing its flow. See `flows state-overrides --help`.
