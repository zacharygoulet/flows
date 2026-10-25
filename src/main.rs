use clap::{Parser, Subcommand};
use flows::{
    Autonomy, ContextUpdate, Flow, FlowName, Instance, InstanceName, InstanceSavePolicy, Move, Scope,
    StateName, Storage, output,
};
use rust_utils::raise::RaiseExt;
use uuid::Uuid;

const FLOW_TEMPLATE: &str = include_str!("assets/flow.toml");
const HELP_OVERVIEW: &str = include_str!("assets/help.txt");
const MAX_CONTEXT_VALUE_CHARS: usize = 150;
const CONTEXT_VALUE_LIMIT_MESSAGE: &str = "context values must be at most 150 characters; context is for compact reminders and pointers, not detailed plans or notes";

#[derive(Parser)]
#[command(about = HELP_OVERVIEW)]
enum Cli {
    /// Start a new instance from a flow and show its status.
    Start {
        /// Flow to start from.
        flow_name: FlowName,
        /// Optional instance name; generated if omitted.
        instance_name: Option<InstanceName>,
        /// Save globally, requiring an existing global flow.
        #[arg(short, long)]
        global: bool,
        /// Save globally, copying the local flow if absent globally.
        #[arg(short = 'G', long = "copy-flow")]
        copy_flow: bool,
        /// Claim the new instance for this agent.
        #[arg(long)]
        owner: Option<String>,
    },

    /// Claim an unowned instance, or confirm the claim; show its status.
    Resume {
        instance_name: InstanceName,
        /// Agent identifier to claim the instance for.
        #[arg(long)]
        owner: String,
    },

    /// Release ownership of an instance. Consider adding context, giving a meaninful name for the instance, or any other handoff first.
    Pause { instance_name: InstanceName },

    /// Rename an instance.
    Rename {
        instance_name: InstanceName,
        new_name: InstanceName,
    },

    /// Delete an instance.
    Stop { instance_name: InstanceName },

    /// Complete and delete an instance in a terminal state.
    Complete { instance_name: InstanceName },

    /// Move to a listed next state and show its status.
    Next {
        /// Instance to move.
        instance_name: InstanceName,
        /// Listed next state.
        target: String,
    },

    /// Configure initial, final, and skipped states for an instance.
    StateOverrides {
        #[command(subcommand)]
        action: StateOverridesAction,
    },

    /// Jump to any unskipped defined state and show its status.
    Jump {
        /// Instance to move.
        instance_name: InstanceName,
        /// Destination state.
        target: String,
    },

    /// Show current or named state guidance without moving, or list all instances.
    Status {
        /// Instance to inspect; omit to list all instances.
        instance_name: Option<InstanceName>,
        /// State to inspect without changing the instance.
        state: Option<String>,
    },

    /// List flows or states in a flow.
    List {
        #[command(subcommand)]
        target: Option<ListTarget>,
    },

    /// Set or remove persistent instance context.
    Context {
        #[command(subcommand)]
        action: ContextAction,
    },

    /// Set autonomy for a state or a range along next paths.
    Autonomy {
        #[command(subcommand)]
        action: AutonomyAction,
    },

    /// Create a flow from the bundled template.
    NewFlowFromTemplate {
        /// Name for the new flow.
        flow_name: FlowName,
        #[arg(short, long)]
        global: bool,
    },
}

#[derive(Subcommand)]
enum StateOverridesAction {
    /// Set an instance's initial state, reset its current state, and show status.
    Initial {
        instance_name: InstanceName,
        state: String,
    },
    /// Manage additional final states for an instance.
    Final {
        #[command(subcommand)]
        action: FinalAction,
    },
    /// Bypass a state on normal next transitions.
    Skip {
        instance_name: InstanceName,
        state: String,
    },
    /// Restore a skipped state to normal next transitions.
    Unskip {
        instance_name: InstanceName,
        state: String,
    },
}

#[derive(Subcommand)]
enum FinalAction {
    /// Add a custom final state.
    Add {
        instance_name: InstanceName,
        state: String,
    },
    /// Remove a custom final state (flow terminal states remain terminal).
    Remove {
        instance_name: InstanceName,
        state: String,
    },
}

#[derive(Subcommand)]
enum ListTarget {
    /// List states in flow order.
    States { flow_name: FlowName },
}

#[derive(Subcommand)]
enum ContextAction {
    /// Store or replace a context value.
    Set {
        instance_name: InstanceName,
        key: String,
        value: String,
    },
    /// Remove a context value.
    Remove {
        instance_name: InstanceName,
        key: String,
    },
}

#[derive(Subcommand)]
enum AutonomyAction {
    /// Set a state's level or every state on next paths through an end state.
    Set {
        /// Instance to configure.
        instance_name: InstanceName,
        /// guided, steered, or autonomous.
        level: Autonomy,
        /// First state to configure.
        state: String,
        /// Inclusive end state; omit to change only the first state.
        end_state: Option<String>,
    },
}

#[rust_utils::raise_handler]
fn main() {
    run(Cli::parse());
}

fn run(cli: Cli) {
    match cli {
        Cli::Start { flow_name, instance_name, global, copy_flow, owner } => {
            start(flow_name, instance_name, global, copy_flow, owner)
        }
        Cli::Resume { instance_name, owner } => {
            Instance::load_from_name(instance_name.as_str()).claim(owner);
            println!("instance {instance_name} claimed");
            println!();
            print!("{}", Instance::load_from_name(instance_name.as_str()).status());
        }
        Cli::Pause { instance_name } => {
            Instance::load_from_name(instance_name.as_str()).release();
            println!("instance {instance_name} is unowned");
        }
        Cli::Rename { instance_name, new_name } => {
            Instance::load_from_name(instance_name.as_str()).rename(new_name.clone());
            println!("renamed instance {instance_name} to {new_name}");
        }
        Cli::Stop { instance_name } => {
            Instance::stop_from_name(instance_name.as_str());
            println!("stopped instance {instance_name}");
        }
        Cli::Complete { instance_name } => {
            Instance::complete_from_name(instance_name.as_str());
            println!("completed instance {instance_name}");
        }
        Cli::NewFlowFromTemplate { flow_name, global } => create_flow_from_template(flow_name, global),
        Cli::Status { instance_name: Some(instance_name), state } => {
            let instance = Instance::load_from_name(instance_name.as_str());
            print!(
                "{}",
                match state {
                    Some(state) => instance.status_for_state(&state),
                    None => instance.status(),
                }
            );
        }
        Cli::Status { instance_name: None, state: None } => status_all(),
        Cli::Status { instance_name: None, state: Some(_) } => {
            unreachable!("clap requires the instance name")
        }
        Cli::List { target } => list(target),
        Cli::Context { action } => change_context(action),
        Cli::Autonomy { action } => change_autonomy(action),
        Cli::StateOverrides { action } => change_state_overrides(action),
        Cli::Next { instance_name, target } => move_instance(instance_name, Move::Next(StateName(target))),
        Cli::Jump { instance_name, target } => move_instance(instance_name, Move::JumpTo(StateName(target))),
    }
}

fn start(
    flow_name: FlowName,
    instance_name: Option<InstanceName>,
    global: bool,
    copy_flow: bool,
    owner: Option<String>,
) {
    let name = instance_name
        .unwrap_or_else(|| InstanceName::parse(format!("{}-{}", flow_name, Uuid::new_v4().simple())).raise());

    let flow = Flow::load_from_name(flow_name.as_str());
    let mut instance = flow.into_new_instance(name);
    if let Some(owner) = owner {
        instance = instance.with_owner(owner);
    }
    let path = instance.save_new(InstanceSavePolicy::from_cli_args(global, copy_flow));
    println!("{}", output::created_instance(&instance, &path));
    println!();
    print!("{}", instance.status());
}

fn create_flow_from_template(flow_name: FlowName, global: bool) {
    let scope = if global { Scope::Global } else { Scope::Local };
    let path = Flow::create_from_template(flow_name.clone(), FLOW_TEMPLATE, scope);
    println!("{}", output::created_flow(&flow_name, &path));
}

fn status_all() {
    let instances = Instance::load_all();
    print!("{}", output::InstancesTable(&instances));
}

fn list(target: Option<ListTarget>) {
    match target {
        Some(ListTarget::States { flow_name }) => {
            let flow = Flow::load_from_name(flow_name.as_str());
            print!("{}", output::StatesTable(&flow));
        }
        None => {
            let names = Storage::current().flow_names();
            let flows: Vec<_> = names
                .iter()
                .map(|name| Flow::load_from_name(name.as_str()))
                .collect();
            print!("{}", output::FlowsTable(&flows));
        }
    }
}

fn change_state_overrides(action: StateOverridesAction) {
    match action {
        StateOverridesAction::Initial { instance_name, state } => {
            let instance = Instance::load_from_name(instance_name.as_str()).set_initial(state);
            print!("{}", instance.status());
        }
        StateOverridesAction::Final { action } => match action {
            FinalAction::Add { instance_name, state } => {
                Instance::load_from_name(instance_name.as_str()).add_final(state);
            }
            FinalAction::Remove { instance_name, state } => {
                Instance::load_from_name(instance_name.as_str()).remove_final(&state);
            }
        },
        StateOverridesAction::Skip { instance_name, state } => {
            Instance::load_from_name(instance_name.as_str()).skip(state);
        }
        StateOverridesAction::Unskip { instance_name, state } => {
            Instance::load_from_name(instance_name.as_str()).unskip(&state);
        }
    }
}

fn change_context(action: ContextAction) {
    let (name, change) = match action {
        ContextAction::Set { instance_name, key, value } => {
            if value.chars().count() > MAX_CONTEXT_VALUE_CHARS {
                rust_utils::raise::raise(CONTEXT_VALUE_LIMIT_MESSAGE.to_owned());
            }
            (instance_name, ContextUpdate::Set { key, value })
        }
        ContextAction::Remove { instance_name, key } => (instance_name, ContextUpdate::Remove { key }),
    };
    Instance::load_from_name(name.as_str()).update_context(change);
}

fn change_autonomy(action: AutonomyAction) {
    match action {
        AutonomyAction::Set { instance_name, level, state, end_state } => {
            let changed = Instance::load_from_name(instance_name.as_str()).set_autonomy(
                level,
                &state,
                end_state.as_deref(),
            );
            print!("{}", output::autonomy_changed(level, &changed));
        }
    }
}

fn move_instance(instance_name: InstanceName, movement: Move) {
    let instance = Instance::load_from_name(instance_name.as_str()).move_to(movement);
    println!("{}", output::moved_instance(&instance));
    println!();
    print!("{}", instance.status());
}
