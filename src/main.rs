use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use zfg::core::{
    config::Settings,
    store::{AddFocusResult, List, Store},
};

mod ui;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Launcher,
    Focus,
    Later,
    Settings,
    Status {
        #[arg(long)]
        json: bool,
    },
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
}
#[derive(Subcommand)]
enum TaskCommand {
    Add {
        #[command(flatten)]
        target: Target,
        text: String,
        #[arg(long)]
        replace: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    Complete {
        #[command(flatten)]
        target: Target,
        index: usize,
    },
    Move {
        #[arg(long, value_enum)]
        from: Destination,
        #[arg(long, value_enum)]
        to: Destination,
        #[arg(long)]
        index: usize,
    },
}
#[derive(Args)]
#[group(required = true, multiple = false)]
struct Target {
    #[arg(long)]
    focus: bool,
    #[arg(long)]
    later: bool,
}
impl Target {
    fn list(&self) -> List {
        if self.focus { List::Focus } else { List::Later }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Destination {
    Focus,
    Later,
}
fn index(value: usize) -> Result<usize> {
    value.checked_sub(1).context("task indices start at 1")
}
fn added(result: AddFocusResult, limit: usize) -> Result<()> {
    if let AddFocusResult::Full { current } = result {
        bail!(
            "{}",
            serde_json::json!({"error":"focus_full", "focus_count":current.len(), "focus_limit":limit})
        );
    }
    Ok(())
}
fn run() -> Result<()> {
    let cli = Cli::parse();
    let settings = Settings::load()?;
    let root = dirs::data_dir()
        .context("Could not determine XDG data directory")?
        .join("zfg");
    let store = Store::new(root, settings.focus_limit)?;
    match cli.command {
        Command::Status { json } => {
            let status = store.status()?;
            if json {
                let mut output = serde_json::to_value(&status)?;
                output["panel"] = serde_json::to_value(&settings.panel)?;
                output["focus_empty_label"] =
                    zfg::core::i18n::text(&settings.language, "focus.empty").into();
                output["panel_labels"] = serde_json::json!({
                    "focus": zfg::core::i18n::text(&settings.language, "pie.focus"),
                    "later": zfg::core::i18n::text(&settings.language, "pie.later"),
                    "open_focus": zfg::core::i18n::text(&settings.language, "task.open_focus"),
                    "open_later": zfg::core::i18n::text(&settings.language, "task.open_later"),
                    "capture": zfg::core::i18n::text(&settings.language, "task.capture"),
                });
                println!("{}", serde_json::to_string(&output)?);
            } else {
                println!(
                    "Focus {}/{}: {}\nLater {}",
                    status.focus_count, status.focus_limit, status.focus_first, status.later_count
                );
            }
        }
        Command::Task { command } => match command {
            TaskCommand::Add {
                target,
                text,
                replace,
                json,
            } => {
                let result = if let Some(position) = replace {
                    if !target.focus {
                        bail!("--replace requires --focus");
                    }
                    store.replace_focus(index(position)?, &text, &store.snapshot()?)?;
                    AddFocusResult::Added
                } else if target.focus {
                    store.add_focus(&text)?
                } else {
                    store.add_later(&text)?;
                    AddFocusResult::Added
                };
                if result == AddFocusResult::Added && json {
                    println!("{{\"result\":\"added\"}}");
                } else {
                    added(result, settings.focus_limit)?;
                }
            }
            TaskCommand::Complete {
                target,
                index: position,
            } => store.complete(target.list(), index(position)?, &store.snapshot()?)?,
            TaskCommand::Move {
                from,
                to,
                index: position,
            } => {
                if from == to {
                    bail!("source and destination must differ");
                }
                let list = if from == Destination::Focus {
                    List::Focus
                } else {
                    List::Later
                };
                added(
                    store.move_task(list, index(position)?, &store.snapshot()?)?,
                    settings.focus_limit,
                )?;
            }
        },
        Command::Launcher => ui::run(store, settings, ui::UiMode::Launcher)?,
        Command::Focus => ui::run(store, settings, ui::UiMode::Focus)?,
        Command::Later => ui::run(store, settings, ui::UiMode::Later)?,
        Command::Settings => ui::run(store, settings, ui::UiMode::Settings)?,
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
