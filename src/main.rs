use ltrait::color_eyre::Result;
use ltrait::{Launcher, Level};
use ltrait_source_desktop::DesktopEntry;
// use ltrait_ui_tui::style::Style;
// use ltrait_ui_tui::{TuiConfig, TuiEntry};

enum Item {
    Desktop(DesktopEntry),
}

struct DummyUI;

use ltrait::ui::{Buffer, UI};

impl<'a> UI<'a> for DummyUI {
    type Context = ();

    async fn run<Cusion: 'a + Send>(
        &self,
        mut batcher: ltrait::launcher::batcher::Batcher<'a, Cusion, Self::Context>,
    ) -> Result<Cusion> {
        let mut buf = Buffer::default();

        let mut more = true;
        while more {
            more = batcher.merge(&mut buf).await?;
            eprintln!("{}", buf.len());
        }
        batcher.compute_cusion(0)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    ltrait::setup(Level::INFO)?;

    let launcher = Launcher::default()
        .add_source(ltrait_source_desktop::new()?, |e| {
            eprintln!("{}", e.entry.appid);
            Item::Desktop(e)
        })
        // .batch_size(50)
        .batch_size(0)
        .set_ui(DummyUI, |_| ());
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
