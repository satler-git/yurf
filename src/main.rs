use ltrait::action::ClosureAction;
use ltrait::color_eyre::Result;
use ltrait::{Launcher, Level};
use ltrait_gen_calc::{Calc, CalcConfig};
use ltrait_scorer_nucleo::{CaseMatching, Context};
use ltrait_sorter_frecency::FrecencyConfig;
use ltrait_source_desktop::DesktopEntry;
// use ltrait_ui_tui::style::Style;
// use ltrait_ui_tui::{TuiConfig, TuiEntry};
use ltrait_extra::sorter::{ReversedSorter, SorterIf};

use std::time::Duration;

#[derive(strum::Display)]
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

struct DummyUI;

use ltrait::ui::{Buffer, UI};

impl<'a> UI<'a> for DummyUI {
    type Context = String;

    async fn run<Cusion: 'a + Send>(
        &self,
        mut batcher: ltrait::launcher::batcher::Batcher<'a, Cusion, Self::Context>,
    ) -> Result<Cusion> {
        let mut buf = Buffer::default();

        let mut more = true;
        batcher.input(&mut buf, "firefox".into());
        while more {
            more = batcher.merge(&mut buf).await?;
            eprintln!("{}", buf.len());
        }

        // デバッグのあと(0.5.0の次のバージョンで修正されたバグ)
        // more = true;
        // batcher.input(&mut buf, "firefox".into());
        //
        // while more {
        //     more = batcher.merge(&mut buf).await?;
        //     eprintln!("{}", buf.len());
        // }

        while let Some(s) = buf.next() {
            eprintln!("{}", s.0);
        }
        batcher.compute_cusion(0)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    ltrait::setup(Level::INFO)?;

    let launcher = Launcher::default()
        // .add_source(ltrait_source_desktop::new()?, Item::Desktop) // TODO: 遅い
        .add_source(Box::pin(ltrait::tokio_stream::iter(1..=5000)), Item::Num)
        // .add_generator(
        //     ClosureGenerator::new(|input| vec![input.to_string()]),
        //     Item::Calc,
        // )
        .add_generator(
            Calc::new(CalcConfig::new(
                (Some('k'), None),
                None,
                None, // 精度
                None,
            )),
            Item::Calc,
        )
        .add_sorter(
            ReversedSorter::new(SorterIf::new(
                ltrait_scorer_nucleo::NucleoMatcher::new(
                    false,
                    CaseMatching::Smart,
                    ltrait_scorer_nucleo::Normalization::Smart,
                )
                .into_sorter(),
                |c: &Context| &c.match_string != "",
            )),
            |c| match c {
                Item::Desktop(_) => Context {
                    match_string: c.into(),
                },
                _ => Context {
                    match_string: "".into(),
                },
            },
        )
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
        // .add_filter(ClosureFilter::new(|_, _| false), |_| ())
        // .batch_size(50)
        .batch_size(0)
        .set_ui(DummyUI, |c| c.into())
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
    // .batch_size(50)
    // .set_ui(
    //     ltrait_ui_tui::Tui::new(TuiConfig::new(8, '>', ' ')),
    //     |cusion| match cusion {
    //         Item::Desktop(entry) => {
    //             let entry = &entry.entry;
    //             TuiEntry {
    //                 text: (
    //                     entry
    //                         .name(&["ja", "en"])
    //                         .or_else(|| Some(entry.id().into()))
    //                         .unwrap()
    //                         .to_string(),
    //                     Style::default(),
    //                 ),
    //             }
    //         }
    //     },
    // );

    launcher.run().await?;

    Ok(())
}
