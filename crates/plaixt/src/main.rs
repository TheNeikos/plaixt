#![allow(dead_code)]

use camino::Utf8Path;
use camino::Utf8PathBuf;
use clap::Parser;
use clap::Subcommand;
use clap::ValueHint;
use human_panic::Metadata;
use kdl::KdlDocument;
use miette::LabeledSpan;
use miette::WrapErr;
use tracing::info;

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

#[derive(Debug)]
pub struct Config {
    root_folder: Utf8PathBuf,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    human_panic::setup_panic!(
        Metadata::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
            .authors(env!("CARGO_PKG_AUTHORS"))
    );

    tracing_subscriber::fmt().pretty().init();

    let args = Args::parse();

    let config = parse_config(&args.config).await?;
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

async fn parse_config(path: &Utf8Path) -> miette::Result<Config> {
    let data = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| miette::miette!(e))
        .wrap_err_with(|| miette::miette!("Could not read configuration at \"{path}\""))?;

    let doc: KdlDocument = data
        .parse()
        .map_err(|e| miette::Error::from(e).with_source_code(data.clone()))?;

    Ok(Config {
        root_folder: doc
            .get("root_folder")
            .ok_or_else(|| miette::miette!("\"root_folder\" configuration value not found"))
            .and_then(|val| {
                val.get(0)
                    .and_then(|v| v.as_string().map(Into::into))
                    .ok_or_else(|| {
                        miette::diagnostic!(
                            labels = vec![LabeledSpan::new_primary_with_span(None, val.span())],
                            "root_folder is expected to be a path"
                        )
                        .into()
                    })
                    .map_err(|e: miette::Report| e.with_source_code(data))
            })?,
    })
}
