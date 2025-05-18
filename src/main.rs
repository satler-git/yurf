use ltrait::{
    Launcher, Level,
    action::ClosureAction,
    color_eyre::{Result, eyre::WrapErr},
    filter::ClosureFilter,
    sorter::ClosureSorter,
};

use ltrait_extra::{scorer::ScorerExt as _, sorter::SorterExt as _};
use ltrait_gen_calc::{Calc, CalcConfig};
use ltrait_scorer_nucleo::{CaseMatching, Context};
use ltrait_sorter_frecency::FrecencyConfig;
use ltrait_source_desktop::DesktopEntry;
use ltrait_ui_tui::{Tui, TuiConfig, TuiEntry, style::Style};

use std::{cmp::Ordering, io, time::Duration};

use tracing::info;

use tikv_jemallocator::Jemalloc;
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct Task {
    name: String,
    command: String,
    need_confirm: bool,
}

#[derive(Deserialize, Debug)]
struct Config {
    task: Vec<Task>,
}

#[derive(strum::Display, strum::EnumIs, Clone)]
enum Item {
    Desktop(DesktopEntry),
    Calc(String),
    Stdin(String),
    Task(Task),
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
                '>',
                ' ',
                ltrait_ui_tui::sample_keyconfig,
            )),
            |c| TuiEntry {
                text: (c.into(), Style::new()),
            },
        );

    if !args.command.is_stdin() {
        launcher = launcher
            .add_raw_sorter(
                ltrait_sorter_frecency::Frecency::new(FrecencyConfig {
                    // Duration::from_secs(days * MINS_PER_HOUR * SECS_PER_MINUTE * HOURS_PER_DAY)
                    half_life: Duration::from_secs(30 * 60 * 60 * 24),
                    type_ident: args.command.type_ident(),
                })?
                .to_if(
                    |c| !Item::is_calc(c),
                    |c: &Item| ltrait_sorter_frecency::Context {
                        ident: format!("{}-{}", c, Into::<String>::into(c)),
                        bonus: 15.,
                    },
                ),
            )
            .add_action(
                ltrait_sorter_frecency::Frecency::new(FrecencyConfig {
                    half_life: Duration::from_secs(30 * 60 * 60 * 24),
                    type_ident: args.command.type_ident(),
                })?,
                |c| ltrait_sorter_frecency::Context {
                    ident: format!("{}-{}", c, Into::<String>::into(c)),
                    bonus: 15.,
                },
            );
    }

    match args.command {
        Commands::Stdin => {
            pub fn new<'a>() -> ltrait::source::Source<'a, String> {
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
            // TODO: 将来的にはWaylougoutみたいなやつのreplace
            // TODO: confirmは落ちる。popupみたいな感じにするかいい感じにしないとだめ。
            let cfg = dirs::config_dir()
                .expect("Could not find config directory")
                .join("yurf")
                .join("config.toml");

            let toml_str = std::fs::read_to_string(cfg).unwrap_or_default();

            let config: Config = toml::from_str(&toml_str)?;

            launcher = launcher
                .add_source(
                    Box::pin(ltrait::tokio_stream::iter(config.task)),
                    Item::Task,
                )
                .add_raw_action(ClosureAction::new(|c| {
                    if let Item::Task(t) = c {
                        use std::io::Write;
                        use std::os::unix::process::CommandExt;
                        use std::process::{Command, Stdio};

                        if t.need_confirm {
                            let exe_path = std::env::current_exe()?;

                            let mut child = Command::new(&exe_path)
                                .arg("stdin")
                                .stdin(Stdio::piped())
                                .stdout(Stdio::piped())
                                .spawn()?;

                            if let Some(mut stdin) = child.stdin.take() {
                                stdin.write_all(b"yes\nno")?;
                            }
                            let output = child.wait_with_output()?;
                            let stdout_str =
                                String::from_utf8(output.stdout).expect("Invalid UTF-8 in stdout");
                            if &stdout_str != "yes\n" {
                                return Ok(());
                            }
                        }

                        let cmd = t.command.clone();
                        let cmd = cmd.split_whitespace().collect::<Vec<&str>>();

                        Command::new(cmd[0])
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
                .add_raw_sorter(ClosureSorter::new(|lhs, rhs, _| match (lhs, rhs) {
                    (Item::Calc(_), Item::Calc(_)) => Ordering::Equal,
                    (Item::Calc(_), _) => Ordering::Greater,
                    (_, Item::Calc(_)) => Ordering::Less,
                    _ => Ordering::Equal,
                }))
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
