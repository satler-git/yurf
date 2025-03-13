use ltrait::action::ClosureAction;
use ltrait::color_eyre::Result;
use ltrait::filter::ClosureFilter;
use ltrait::generator::ClosureGenerator;
use ltrait::{Launcher, Level};
use ltrait_gen_calc::{Calc, CalcConfig};
use ltrait_scorer_nucleo::{CaseMatching, Context};
use ltrait_source_desktop::DesktopEntry;
// use ltrait_ui_tui::style::Style;
// use ltrait_ui_tui::{TuiConfig, TuiEntry};

enum Item {
    Desktop(DesktopEntry),
    Num(u32),
    Calc(String),
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
        batcher.input(&mut buf, "= 1 + 2".into());
        while more {
            more = batcher.merge(&mut buf).await?;
            eprintln!("{}", buf.len());
        }
        // while let Some(s) = buf.next() {
        //     eprintln!("{}", s.0);
        // }
        batcher.compute_cusion(0)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    ltrait::setup(Level::INFO)?;

    let launcher = Launcher::default()
        // .add_source(ltrait_source_desktop::new()?, Item::Desktop) // TODO: 遅い
        .add_source(Box::pin(ltrait::tokio_stream::iter(1..=5000)), Item::Num)
        .add_generator(
            ClosureGenerator::new(|input| vec![input.to_string()]),
            Item::Calc,
        )
        // .add_generator(
        //     Calc::new(CalcConfig::new(
        //         (Some('k'), None),
        //         None,
        //         Some(52), // 精度
        //         None,
        //     )),
        //     Item::Calc,
        // )
        .add_sorter(
            ltrait_scorer_nucleo::NucleoMatcher::new(
                // TODO: reverse
                false,
                CaseMatching::Smart,
                ltrait_scorer_nucleo::Normalization::Smart,
            )
            .into_sorter(),
            |c| Context {
                match_string: match c {
                    Item::Desktop(e) => e
                        .entry
                        .name(&["ja", "en"])
                        .or_else(|| Some(e.entry.id().into()))
                        .unwrap()
                        .into(),
                    Item::Num(e) => format!("{e}"),
                    Item::Calc(s) => s.clone(),
                },
            },
        )
        // .add_filter(ClosureFilter::new(|_, _| false), |_| ())
        // .batch_size(50)
        .batch_size(0)
        .set_ui(DummyUI, |c| match c {
            Item::Desktop(e) => e.entry.appid.clone().into(),
            Item::Num(e) => format!("{e}"),
            Item::Calc(s) => s.clone(),
        })
        .add_action(
            ClosureAction::new(|s| {
                println!("{s}");
                Ok(())
            }),
            |c| match c {
                Item::Desktop(e) => e.entry.appid.clone().into(),
                Item::Num(e) => format!("{e}"),
                Item::Calc(s) => s.clone(),
            },
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
