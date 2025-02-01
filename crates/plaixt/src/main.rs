#![allow(dead_code)]

use camino::Utf8PathBuf;
use clap::Parser;
use clap::Subcommand;
use clap::ValueHint;
use human_panic::Metadata;
use tracing::info;

mod config;
mod parsing;

#[derive(Debug, Parser)]
struct Args {
    #[arg(short, long, value_hint(ValueHint::DirPath))]
    root_folder: Option<Utf8PathBuf>,

    #[arg(
        short,
        long,
        value_hint(ValueHint::FilePath),
        default_value_t = Utf8PathBuf::from("plaixt.kdl")
    )]
    config: Utf8PathBuf,

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

    let config = config::parse_config(&args.config).await?;
    let root_folder = args.root_folder.as_ref().unwrap_or(&config.root_folder);

    let load_records = async {
        let definitions = parsing::load_definitions(&root_folder.join("definitions")).await?;
        parsing::load_records(root_folder, &definitions).await
    };

    match args.mode {
        ArgMode::Dump => {
            let records = load_records.await?;

            info!("Got records: {records:#?}");
        }
    }

    Ok(())
}
