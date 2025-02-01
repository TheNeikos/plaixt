#![allow(dead_code)]

use camino::Utf8PathBuf;
use clap::Parser;
use clap::Subcommand;
use clap::ValueHint;
use human_panic::Metadata;

mod parsing;

#[derive(Debug, Parser)]
struct Args {
    #[arg(short, long, value_hint(ValueHint::DirPath))]
    path: Utf8PathBuf,

    #[command(subcommand)]
    mode: ArgMode,
}

#[derive(Debug, Subcommand)]
enum ArgMode {
    Dump,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    human_panic::setup_panic!(
        Metadata::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
            .authors(env!("CARGO_PKG_AUTHORS"))
    );

    tracing_subscriber::fmt().pretty().init();

    let args = Args::parse();
    let load_records = async {
        let definitions = parsing::load_definitions(args.path.join("definitions")).await?;
        parsing::load_records(args.path, &definitions).await
    };

    match args.mode {
        ArgMode::Dump => {
            let records = load_records.await?;
        }
    }

    Ok(())
}
