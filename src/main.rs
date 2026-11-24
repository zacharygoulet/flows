use clap::{CommandFactory, Parser, Subcommand};
use flow::{
    Autonomy, ContextUpdate, Flow, FlowName, Instance, InstanceName, InstanceSavePolicy, Move, Scope,
    StepName, Storage, output,
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
        #[arg(short = 'G', long = "copy-flow", conflicts_with = "global")]
        copy_flow: bool,
        /// Claim the new instance for this agent.
        #[arg(long)]
        owner: Option<String>,
    },

    /// Claim an unowned instance, or confirm the claim; show its status.
    Claim {
        instance_name: InstanceName,
        /// Agent identifier to claim the instance for.
        #[arg(long)]
        owner: String,
    },

    /// Release ownership of an instance. Consider adding context, giving a meaningful name for the instance, or any other handoff first.
    Release { instance_name: InstanceName },

    /// Rename an instance.
    Rename {
        instance_name: InstanceName,
        new_name: InstanceName,
    },

    /// Delete an instance.
    Delete { instance_name: InstanceName },

    /// Complete and delete an instance in a terminal step.
    Complete { instance_name: InstanceName },

    /// Move to a listed next step and show its status.
    Next {
        /// Instance to move.
        instance_name: InstanceName,
        /// Listed next step; omit only when exactly one destination is available.
        target: Option<String>,
    },

    /// Configure initial, final, and skipped steps for an instance.
    Route {
        #[command(subcommand)]
        action: RouteAction,
    },

    /// Jump to any unskipped defined step and show its status.
    Jump {
        /// Instance to move.
        instance_name: InstanceName,
        /// Destination step.
        target: String,
    },

    /// Show current or named step guidance without moving.
    Show {
        instance_name: InstanceName,
        /// Step to inspect without changing the instance.
        #[arg(long)]
        step: Option<String>,
    },

    /// Move to the configured initial step and show its status.
    Reset { instance_name: InstanceName },

    /// List flows, instances, or steps in a flow.
    List {
        #[command(subcommand)]
        target: Option<ListTarget>,
    },

    /// Set or remove persistent instance context.
    Context {
        #[command(subcommand)]
        action: ContextAction,
    },

    /// Set autonomy for a step or a range along next paths.
    Autonomy {
        #[command(subcommand)]
        action: AutonomyAction,
    },

    /// Create a flow from the bundled template.
    CreateFlow {
        /// Name for the new flow.
        flow_name: FlowName,
        #[arg(short, long)]
        global: bool,
    },
}

#[derive(Subcommand)]
enum RouteAction {
    /// Configure the initial step without moving.
    Initial {
        #[command(subcommand)]
        action: InitialAction,
    },
    /// Manage additional final steps for an instance.
    Final {
        #[command(subcommand)]
        action: FinalAction,
    },
    /// Bypass a step on normal next transitions.
    Skip {
        instance_name: InstanceName,
        step: String,
    },
    /// Restore a skipped step to normal next transitions.
    Unskip {
        instance_name: InstanceName,
        step: String,
    },
}

#[derive(Subcommand)]
enum InitialAction {
    /// Set the configured initial step without moving.
    Set {
        instance_name: InstanceName,
        step: String,
    },
}

#[derive(Subcommand)]
enum FinalAction {
    /// Add a custom final step.
    Add {
        instance_name: InstanceName,
        step: String,
    },
    /// Remove a custom final step (flow terminal steps remain terminal).
    Remove {
        instance_name: InstanceName,
        step: String,
    },
}

#[derive(Subcommand)]
enum ListTarget {
    /// List available flows.
    Flows,
    /// List available instances.
    Instances,
    /// List steps in flow order.
    Steps { flow_name: FlowName },
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
    /// Set a step's level or every step on next paths through an end step.
    Set {
        /// Instance to configure.
        instance_name: InstanceName,
        /// guided, steered, or autonomous.
        level: Autonomy,
        /// Configure exactly one step.
        #[arg(long, required_unless_present = "from", conflicts_with_all = ["from", "through"])]
        step: Option<String>,
        /// First step of an inclusive structural-graph range.
        #[arg(long, requires = "through", conflicts_with = "step")]
        from: Option<String>,
        /// Last step of the range.
        #[arg(long, requires = "from", conflicts_with = "step")]
        through: Option<String>,
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
        Cli::Claim { instance_name, owner } => {
            Instance::load_from_name(instance_name.as_str()).claim(owner);
            println!("instance {instance_name} claimed");
            println!();
            print!("{}", Instance::load_from_name(instance_name.as_str()).status());
        }
        Cli::Release { instance_name } => {
            Instance::load_from_name(instance_name.as_str()).release();
            println!("instance {instance_name} is unowned");
        }
        Cli::Rename { instance_name, new_name } => {
            Instance::load_from_name(instance_name.as_str()).rename(new_name.clone());
            println!("renamed instance {instance_name} to {new_name}");
        }
        Cli::Delete { instance_name } => {
            Instance::stop_from_name(instance_name.as_str());
            println!("deleted instance {instance_name}");
        }
        Cli::Complete { instance_name } => {
            Instance::complete_from_name(instance_name.as_str());
            println!("completed instance {instance_name}");
        }
        Cli::CreateFlow { flow_name, global } => create_flow_from_template(flow_name, global),
        Cli::Show { instance_name, step } => {
            let instance = Instance::load_from_name(instance_name.as_str());
            print!(
                "{}",
                match step {
                    Some(step) => instance.status_for_step(&step),
                    None => instance.status(),
                }
            );
        }
        Cli::Reset { instance_name } => {
            let instance = Instance::load_from_name(instance_name.as_str()).reset();
            println!("reset instance {instance_name}");
            println!();
            print!("{}", instance.status());
        }
        Cli::List { target: Some(target) } => list(target),
        Cli::List { target: None } => {
            let mut command = Cli::command();
            command.build();
            command
                .find_subcommand_mut("list")
                .expect("list command exists")
                .print_help()
                .raise();
            println!();
        }
        Cli::Context { action } => change_context(action),
        Cli::Autonomy { action } => change_autonomy(action),
        Cli::Route { action } => change_route(action),
        Cli::Next { instance_name, target } => next_instance(instance_name, target),
        Cli::Jump { instance_name, target } => move_instance(
            Instance::load_from_name(instance_name.as_str()),
            Move::JumpTo(StepName(target)),
        ),
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

fn list(target: ListTarget) {
    match target {
        ListTarget::Steps { flow_name } => {
            let flow = Flow::load_from_name(flow_name.as_str());
            print!("{}", output::StepsTable(&flow));
        }
        ListTarget::Instances => status_all(),
        ListTarget::Flows => {
            let names = Storage::current().flow_names();
            let flows: Vec<_> = names
                .iter()
                .map(|name| Flow::load_from_name(name.as_str()))
                .collect();
            print!("{}", output::FlowsTable(&flows));
        }
    }
}

fn change_route(action: RouteAction) {
    match action {
        RouteAction::Initial { action: InitialAction::Set { instance_name, step } } => {
            Instance::load_from_name(instance_name.as_str()).set_initial(step.clone());
            println!("set initial step to {step:?} for instance {instance_name}");
        }
        RouteAction::Final { action } => match action {
            FinalAction::Add { instance_name, step } => {
                Instance::load_from_name(instance_name.as_str()).add_final(step.clone());
                println!("added final step {step:?} for instance {instance_name}");
            }
            FinalAction::Remove { instance_name, step } => {
                Instance::load_from_name(instance_name.as_str()).remove_final(&step);
                println!("removed final step {step:?} for instance {instance_name}");
            }
        },
        RouteAction::Skip { instance_name, step } => {
            Instance::load_from_name(instance_name.as_str()).skip(step.clone());
            println!("skipped step {step:?} for instance {instance_name}");
        }
        RouteAction::Unskip { instance_name, step } => {
            Instance::load_from_name(instance_name.as_str()).unskip(&step);
            println!("unskipped step {step:?} for instance {instance_name}");
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
    println!("updated context for instance {name}");
}

fn change_autonomy(action: AutonomyAction) {
    match action {
        AutonomyAction::Set { instance_name, level, step, from, through } => {
            let changed = Instance::load_from_name(instance_name.as_str()).set_autonomy(
                level,
                step.as_deref()
                    .or(from.as_deref())
                    .expect("clap requires a selection"),
                through.as_deref(),
            );
            print!("{}", output::autonomy_changed(level, &changed));
        }
    }
}

fn next_instance(instance_name: InstanceName, target: Option<String>) {
    let instance = Instance::load_from_name(instance_name.as_str());
    let target = target.unwrap_or_else(|| {
        let choices = instance.effective_next();
        match choices.as_slice() {
            [step] => step.name.clone(),
            [] => rust_utils::raise::raise(format!(
                "instance {instance_name} has no available next transitions"
            )),
            _ => rust_utils::raise::raise(format!(
                "instance {instance_name} has multiple next destinations; choose one: {}",
                choices
                    .iter()
                    .map(|step| format!("{:?}", step.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    });
    move_instance(instance, Move::Next(StepName(target)));
}

fn move_instance(instance: Instance, movement: Move) {
    let instance = instance.move_to(movement);
    println!("{}", output::moved_instance(&instance));
    println!();
    print!("{}", instance.status());
}
