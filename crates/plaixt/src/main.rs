#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::Read;
use std::sync::Arc;

use camino::Utf8PathBuf;
use clap::Parser;
use clap::Subcommand;
use clap::ValueHint;
use filesystem_trustfall_adapter::FileSystemAdapter;
use human_panic::Metadata;
use miette::IntoDiagnostic;
use parsing::Definition;
use parsing::Record;
use tracing::info;
use tracing_subscriber::EnvFilter;
use trustfall::execute_query;
use trustfall::FieldValue;

mod config;
mod parsing;
mod trustfall_plaixt;

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
    Query,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    human_panic::setup_panic!(
        Metadata::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
            .authors(env!("CARGO_PKG_AUTHORS"))
    );

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .pretty()
        .init();

    let args = Args::parse();

    let config = config::parse_config(&args.config).await?;
    let root_folder = args.root_folder.as_ref().unwrap_or(&config.root_folder);

    let definitions = parsing::load_definitions(&root_folder.join("definitions")).await?;

    let records = parsing::load_records(root_folder, &definitions).await?;

    let (schema, adapter) = get_schema_and_adapter(&definitions, records.clone());

    match args.mode {
        ArgMode::Query => {
            let mut query = String::new();
            std::io::stdin()
                .read_to_string(&mut query)
                .into_diagnostic()?;

            let result = execute_query(
                &schema,
                Arc::new(adapter),
                &query,
                BTreeMap::<Arc<str>, FieldValue>::from([]),
            )
            .unwrap()
            .collect::<Vec<_>>();

            info!("Got records: {result:#?}");
        }
        ArgMode::Dump => {
            print_records(&records);
        }
    }

    Ok(())
}

fn get_schema_and_adapter(
    definitions: &BTreeMap<String, Vec<Definition>>,
    records: Vec<Record>,
) -> (trustfall::Schema, trustfall_plaixt::TrustfallMultiAdapter) {
    let schema = trustfall_plaixt::to_schema(definitions);
    let adapter = trustfall_plaixt::TrustfallMultiAdapter {
        plaixt: trustfall_plaixt::PlaixtAdapter {
            records: records.clone(),
        },
        filesystem: FileSystemAdapter::new(),
    };
    (schema, adapter)
}

fn print_records(records: &[Record]) {
    for record in records {
        println!("{kind} @ {at} {{", kind = record.kind, at = record.at);
        for field in &record.fields {
            println!("\t{name} = {value}", name = field.0, value = field.1);
        }
        println!("}}")
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use tracing_subscriber::EnvFilter;
    use trustfall::provider::check_adapter_invariants;

    use crate::get_schema_and_adapter;
    use crate::parsing;

    #[tokio::test]
    async fn ensure_adapter_satisfies_invariants() {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .pretty()
            .with_test_writer()
            .init();

        let root_folder = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");

        println!("{root_folder}");
        let definitions = parsing::load_definitions(&root_folder.join("definitions"))
            .await
            .unwrap();

        let records = parsing::load_records(&root_folder, &definitions)
            .await
            .unwrap();

        let (schema, adapter) = get_schema_and_adapter(&definitions, records.clone());

        check_adapter_invariants(&schema, adapter);
    }
}
