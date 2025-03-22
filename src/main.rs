use ltrait::action::ClosureAction;
use ltrait::color_eyre::Result;
use ltrait::{Launcher, Level};
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

use tracing::{debug, info};

#[derive(strum::Display, Clone)]
enum Item {
    Desktop(DesktopEntry),
    Num(u32),
    Calc(String),
}

impl Into<String> for &Item {
    fn into(self) -> String {
        match self {
            Item::Desktop(e) => e
                .entry
                .name(&["ja", "en"])
                .or_else(|| Some(e.entry.id().into()))
                .unwrap()
                .into(),
            Item::Num(e) => format!("{e}"),
            Item::Calc(s) => s.into(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = ltrait::setup(Level::INFO)?;
    info!("Tracing has been installed");

    let launcher = Launcher::default()
        .add_source(ltrait_source_desktop::new()?, Item::Desktop)
        .add_source(ltrait::source::from_iter(1..=5000), Item::Num)
        .add_generator(
            Calc::new(CalcConfig::new(
                (Some('k'), None),
                None,
                None, // 精度
                None,
            )),
            Item::Calc,
        )
        .add_raw_sorter(ReversedSorter::new(SorterIf::new(
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
        )))
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
        .add_raw_filter(FilterIf::new(
            ltrait_scorer_nucleo::NucleoMatcher::new(
                false,
                CaseMatching::Smart,
                ltrait_scorer_nucleo::Normalization::Smart,
            )
            .into_filter(|score| {
                debug!("{score}");
                score >= 100
            }), // TODO: どのくらいの数字がいいのかあんまりよくわかってない
            |c: &Item| match c {
                Item::Desktop(_) => true,
                _ => false,
            },
            |c: &Item| Context {
                match_string: c.into(),
            },
        ))
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
        .add_action(
            ClosureAction::new(|s: &String| {
                println!("{s}");
                Ok(())
            }),
            |c| c.into(),
        );

    launcher.run().await?;

    Ok(())
}
