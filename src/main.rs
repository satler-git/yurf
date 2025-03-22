use ltrait::action::ClosureAction;
use ltrait::color_eyre::Result;
use ltrait::color_eyre::eyre::WrapErr;
use ltrait::filter::ClosureFilter;
#[allow(unused_imports)]
use ltrait::sorter::ClosureSorter;
use ltrait::{Launcher, Level};
#[allow(unused_imports)]
use ltrait_extra::{
    filter::FilterIf,
    sorter::{ReversedSorter, SorterIf},
};
use ltrait_gen_calc::{Calc, CalcConfig};
use ltrait_scorer_nucleo::{CaseMatching, Context};
use ltrait_sorter_frecency::FrecencyConfig;
use ltrait_source_desktop::DesktopEntry;

use ltrait_ui_tui::{Tui, TuiConfig, TuiEntry, style::Style};

use std::time::Duration;

use tracing::info;

#[derive(strum::Display, Clone)]
enum Item {
    Desktop(DesktopEntry),
    Calc(String),
}

impl Into<String> for &Item {
    fn into(self) -> String {
        match self {
            Item::Desktop(e) => e
                .entry
                .name(&[/* "ja", */ "en"])
                .or_else(|| Some(e.entry.id().into()))
                .unwrap()
                .into(),
            Item::Calc(s) => s.into(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = ltrait::setup(Level::INFO)?;
    info!("Tracing has been installed");

    let launcher = Launcher::default()
        .add_source(
            ltrait_source_desktop::new(ltrait_source_desktop::default_paths().skip(1))?,
            Item::Desktop,
        )
        // .add_source(ltrait::source::from_iter(1..=5000), Item::Num)
        .add_generator(
            Calc::new(CalcConfig::new(
                (Some('k'), None),
                None,
                None, // 精度
                None,
            )),
            Item::Calc,
        )
        .add_raw_sorter(SorterIf::new(
            ltrait_scorer_nucleo::NucleoMatcher::new(
                false,
                CaseMatching::Smart,
                ltrait_scorer_nucleo::Normalization::Smart,
            )
            .into_sorter(),
            |c: &Item| match c {
                Item::Desktop(_) => true,
                _ => false,
            },
            |c: &Item| Context {
                match_string: c.into(),
            },
        ))
        .add_sorter(
            ltrait_sorter_frecency::Frecency::new(FrecencyConfig {
                // Duration::from_secs(days * MINS_PER_HOUR * SECS_PER_MINUTE * HOURS_PER_DAY)
                half_life: Duration::from_secs(30 * 60 * 60 * 24),
                type_ident: "yurf".into(),
            })?,
            |c| ltrait_sorter_frecency::Context {
                ident: format!("{}-{}", c.to_string(), Into::<String>::into(c)),
                bonus: 15.,
            },
        )
        .add_raw_filter(ClosureFilter::new(|c, _| {
            if let Item::Desktop(d) = c {
                !d.entry.no_display() && d.entry.exec().is_some()
            } else {
                true
            }
        }))
        // .add_raw_filter(FilterIf::new(
        //     ltrait_scorer_nucleo::NucleoMatcher::new(
        //         false,
        //         CaseMatching::Smart,
        //         ltrait_scorer_nucleo::Normalization::Smart,
        //     )
        //     .into_filter(|score| {
        //         debug!("{score}");
        //         score >= 100
        //     }), // TO/DO: どのくらいの数字がいいのかあんまりよくわかってない
        //     |c: &Item| match c {
        //         // Item::Desktop(_) => true,
        //         // _ => false,
        //         _ => true,
        //     },
        //     |c: &Item| Context {
        //         match_string: c.into(),
        //     },
        // ))
        .batch_size(100)
        .set_ui(Tui::new(TuiConfig::new(12, '>', ' ')), |c| TuiEntry {
            text: (c.into(), Style::new()),
        })
        .add_action(
            ltrait_sorter_frecency::Frecency::new(FrecencyConfig {
                half_life: Duration::from_secs(30 * 60 * 60 * 24),
                type_ident: "yurf".into(),
            })?,
            |c| ltrait_sorter_frecency::Context {
                ident: format!("{}-{}", c.to_string(), Into::<String>::into(c)),
                bonus: 15.,
            },
        )
        .add_raw_action(ClosureAction::new(|c| {
            if let Item::Desktop(d) = c {
                use std::process::{Command, Stdio};

                let cmd = d.entry.parse_exec().wrap_err("failed to parse exec")?;

                Command::new(&cmd[0])
                    .args(&cmd[1..])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .wrap_err("failed to start the selected app")?;
            }

            Ok(())
        }));

    launcher.run().await?;

    Ok(())
}
