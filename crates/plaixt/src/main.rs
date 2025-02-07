#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;

use camino::Utf8PathBuf;
use clap::Parser;
use clap::Subcommand;
use clap::ValueHint;
use human_panic::Metadata;
use parsing::Definition;
use parsing::Record;
use tracing::info;
use trustfall::execute_query;
use trustfall::provider::field_property;
use trustfall::provider::resolve_property_with;
use trustfall::provider::Adapter;
use trustfall::FieldValue;
use trustfall::Schema;

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

    let definitions = parsing::load_definitions(&root_folder.join("definitions")).await?;

    let records = parsing::load_records(root_folder, &definitions).await?;

    let schema = to_schema(&definitions);

    let result = execute_query(
        &schema,
        Arc::new(PlaixtAdapter {
            records: records.clone(),
        }),
        "{
            RecordsAll {
                at @output
                kind @output @filter(op: \"=\", value: [\"$foobar\"])
            }
        }",
        [(
            Arc::from("foobar"),
            FieldValue::String(Arc::from("changelog")),
        )]
        .into(),
    )
    .unwrap()
    .collect::<Vec<_>>();

    match args.mode {
        ArgMode::Dump => {
            info!("Got records: {result:#?}");
        }
    }

    Ok(())
}

fn to_schema(_definitions: &BTreeMap<String, Vec<Definition>>) -> Schema {
    Schema::parse(format!(
        r#"schema {{
    query: RootSchemaQuery
}}
{}


type RootSchemaQuery {{
    RecordsAll: [Record!]!
}}
interface Record {{
    at: String!,
    kind: String!,
}}
"#,
        Schema::ALL_DIRECTIVE_DEFINITIONS
    ))
    .unwrap()
}

struct PlaixtAdapter {
    records: Vec<Record>,
}

impl<'a> Adapter<'a> for PlaixtAdapter {
    type Vertex = Record;

    fn resolve_starting_vertices(
        &self,
        edge_name: &Arc<str>,
        _parameters: &trustfall::provider::EdgeParameters,
        _resolve_info: &trustfall::provider::ResolveInfo,
    ) -> trustfall::provider::VertexIterator<'a, Self::Vertex> {
        match edge_name.as_ref() {
            "RecordsAll" => Box::new(self.records.clone().into_iter()),
            _ => unreachable!(),
        }
    }

    fn resolve_property<V: trustfall::provider::AsVertex<Self::Vertex> + 'a>(
        &self,
        contexts: trustfall::provider::ContextIterator<'a, V>,
        type_name: &Arc<str>,
        property_name: &Arc<str>,
        _resolve_info: &trustfall::provider::ResolveInfo,
    ) -> trustfall::provider::ContextOutcomeIterator<'a, V, trustfall::FieldValue> {
        match (type_name.as_ref(), property_name.as_ref()) {
            (_, "__typename") => Box::new(contexts.map(|ctx| {
                let value = match ctx.active_vertex() {
                    Some(_record) => "Record".into(),
                    None => FieldValue::Null,
                };

                (ctx, value)
            })),
            ("Record", "at") => {
                resolve_property_with(contexts, field_property!(at, { at.to_string().into() }))
            }
            ("Record", "kind") => resolve_property_with(contexts, field_property!(kind)),
            _ => unreachable!(),
        }
    }

    fn resolve_neighbors<V: trustfall::provider::AsVertex<Self::Vertex> + 'a>(
        &self,
        _contexts: trustfall::provider::ContextIterator<'a, V>,
        _type_name: &Arc<str>,
        _edge_name: &Arc<str>,
        _parameters: &trustfall::provider::EdgeParameters,
        _resolve_info: &trustfall::provider::ResolveEdgeInfo,
    ) -> trustfall::provider::ContextOutcomeIterator<
        'a,
        V,
        trustfall::provider::VertexIterator<'a, Self::Vertex>,
    > {
        unreachable!()
    }

    fn resolve_coercion<V: trustfall::provider::AsVertex<Self::Vertex> + 'a>(
        &self,
        _contexts: trustfall::provider::ContextIterator<'a, V>,
        _type_name: &Arc<str>,
        _coerce_to_type: &Arc<str>,
        _resolve_info: &trustfall::provider::ResolveInfo,
    ) -> trustfall::provider::ContextOutcomeIterator<'a, V, bool> {
        unreachable!()
    }
}
