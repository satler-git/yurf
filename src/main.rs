use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use ltrait::{
    Launcher, Level,
    action::ClosureAction,
    color_eyre::{Result, eyre::WrapErr},
    filter::ClosureFilter,
    sorter::ClosureSorter,
};

use ltrait_extra::{action::ActionExt as _, scorer::ScorerExt as _, sorter::SorterExt};
use ltrait_gen_calc::{Calc, CalcConfig};
use ltrait_scorer_nucleo::{CaseMatching, Context};
use ltrait_sorter_frecency::{Frecency, FrecencyConfig};
use ltrait_source_desktop::DesktopEntry;
use ltrait_source_task::{Task, TaskConfig, TaskItem};
use ltrait_ui_tui::{Tui, TuiConfig, TuiEntry, style::Style};

use std::{cmp::Ordering, io, time::Duration};

use tracing::info;

use tikv_jemallocator::Jemalloc;
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[derive(strum::Display, strum::EnumIs, strum::EnumTryAs, Clone)]
enum Item {
    Desktop(DesktopEntry),
    Calc(String),
    Stdin(String),
    Task(TaskItem),
}

impl From<&Item> for String {
    fn from(val: &Item) -> Self {
        match val {
            Item::Desktop(e) => e
                .entry
                .name(&[/* "ja", */ "en"])
                .or_else(|| Some(e.entry.id().into()))
                .unwrap()
                .into(),
            Item::Calc(s) => s.into(),
            Item::Stdin(s) => s.into(),
            Item::Task(s) => s.name.clone(),
        }
    }
}

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    /// Copy as a string. Currently only wl-copy is available
    #[arg(short, long)]
    copy: bool,

    /// Display on full screen on the terminal when TUI
    #[arg(short, long, conflicts_with = "inline")]
    fullscreen: bool,

    /// How many lines to display when not in Fullscreen
    #[arg(short, long, default_value_t = 12)]
    inline: u16,
}

#[derive(Subcommand, Debug, strum::EnumIs)]
enum Commands {
    Task,
    Launch,
    Stdin,
}

impl Commands {
    fn type_ident(&self) -> String {
        match self {
            Commands::Task => "yurf-task",
            Commands::Launch => "yurf",
            _ => "",
        }
        .into()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // let args = Args::try_parse().wrap_err("failed to parse args")?;
    let args = Args::parse();

    let _guard = ltrait::setup(Level::INFO)?;
    info!("Tracing has been installed");

    let mut launcher = Launcher::default()
        .add_raw_sorter(ClosureSorter::new(|lhs, rhs, _| match (lhs, rhs) {
            // 優先する
            (Item::Calc(_), Item::Calc(_)) => Ordering::Equal,
            (Item::Calc(_), _) => Ordering::Greater,
            (_, Item::Calc(_)) => Ordering::Less,
            _ => Ordering::Equal,
        }))
        .add_raw_sorter(
            ltrait_scorer_nucleo::NucleoMatcher::new(
                false,
                CaseMatching::Smart,
                ltrait_scorer_nucleo::Normalization::Smart,
            )
            .into_sorter()
            .to_if(
                |c| !Item::is_calc(c),
                |c: &Item| Context {
                    match_string: c.into(),
                },
            ),
        )
        .batch_size(1000)
        .set_ui(
            Tui::new(TuiConfig::new(
                if !args.fullscreen {
                    ltrait_ui_tui::Viewport::Inline(12)
                } else {
                    ltrait_ui_tui::Viewport::Fullscreen
                },
                true,
                '>',
                ' ',
                ltrait_ui_tui::sample_keyconfig,
            )),
            |c| TuiEntry {
                text: (c.into(), Style::new()),
            },
        );

    if !args.command.is_stdin() {
        let config = FrecencyConfig {
            // Duration::from_secs(days * MINS_PER_HOUR * SECS_PER_MINUTE * HOURS_PER_DAY)
            half_life: Duration::from_secs(30 * 60 * 60 * 24),
            type_ident: args.command.type_ident(),
        };

        launcher = launcher
            .add_raw_sorter(SorterExt::to_if(
                Frecency::new(config.clone())?,
                |c| !Item::is_calc(c),
                |c: &Item| ltrait_sorter_frecency::Context {
                    ident: format!("{}-{}", c, Into::<String>::into(c)),
                    bonus: 15.,
                },
            ))
            .add_action(Frecency::new(config)?, |c| {
                ltrait_sorter_frecency::Context {
                    ident: format!("{}-{}", c, Into::<String>::into(c)),
                    bonus: 15.,
                }
            });
    }

    if args.copy {
        launcher = launcher.add_raw_action(ClosureAction::new(|c| {
            Command::new("wl-copy")
                .args([
                    "--type=text/plain".into(),
                    format!("{}", <&Item as Into<String>>::into(c)),
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .process_group(0)
                .spawn()
                .wrap_err("failed to copy")?;

            Ok(())
        }))
    }

    match args.command {
        Commands::Stdin => {
            pub fn new() -> ltrait::source::Source<String> {
                let lines: Vec<_> = io::stdin().lines().map_while(Result::ok).collect();

                Box::pin(ltrait::tokio_stream::iter(lines))
            }

            launcher = launcher
                .add_source(new(), Item::Stdin)
                .add_raw_action(ClosureAction::new(|c: &Item| {
                    println!("{}", <&Item as Into<String>>::into(c));
                    Ok(())
                }));
        }
        Commands::Task => {
            let config = TaskConfig {
                path: vec![ltrait_source_task::default_path()?],
            };

            let task = Task::new(config);

            launcher = launcher
                .add_source(task.create_source()?, Item::Task)
                .add_raw_action(task.to_if(Item::is_task, |c| c.clone().try_as_task().unwrap()));
        }
        Commands::Launch => {
            launcher = launcher
                .add_source(
                    ltrait_source_desktop::new(ltrait_source_desktop::default_paths().skip(1))?,
                    Item::Desktop,
                )
                .add_raw_filter(ClosureFilter::new(|c, _| {
                    if let Item::Desktop(d) = c {
                        !d.entry.no_display() && d.entry.exec().is_some()
                    } else {
                        true
                    }
                }))
                .add_generator(
                    Calc::new(CalcConfig::new(
                        (Some('k'), None),
                        None,
                        None, // 精度
                        None,
                    )),
                    Item::Calc,
                )
                .add_raw_action(ClosureAction::new(|c| {
                    if let Item::Desktop(d) = c {
                        use std::os::unix::process::CommandExt;
                        use std::process::{Command, Stdio};

                        let cmd = d.entry.parse_exec().wrap_err("failed to parse exec")?;

                        Command::new(&cmd[0])
                            .args(&cmd[1..])
                            .stdin(Stdio::null())
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .process_group(0)
                            .spawn()
                            .wrap_err("failed to start the selected app")?;
                    }

                    Ok(())
                }));
        }
    }

    launcher.run().await?;

    Ok(())
}
