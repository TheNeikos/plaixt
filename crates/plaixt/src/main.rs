#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::Read;
use std::sync::Arc;

use camino::Utf8PathBuf;
use clap::Parser;
use clap::Subcommand;
use clap::ValueHint;
use human_panic::Metadata;
use kdl::KdlValue;
use miette::IntoDiagnostic;
use parsing::Definition;
use parsing::Record;
use tracing::debug;
use tracing::info;
use tracing::trace;
use tracing_subscriber::EnvFilter;
use trustfall::execute_query;
use trustfall::provider::field_property;
use trustfall::provider::resolve_coercion_with;
use trustfall::provider::resolve_neighbors_with;
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

    let schema = to_schema(&definitions);

    match args.mode {
        ArgMode::Query => {
            let mut query = String::new();
            std::io::stdin()
                .read_to_string(&mut query)
                .into_diagnostic()?;

            let result = execute_query(
                &schema,
                Arc::new(PlaixtAdapter {
                    records: records.clone(),
                }),
                &query,
                BTreeMap::<Arc<str>, FieldValue>::from([("search".into(), "trust".into())]),
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

fn print_records(records: &[Record]) {
    for record in records {
        println!("{kind} @ {at} {{", kind = record.kind, at = record.at);
        for field in &record.fields {
            println!("\t{name} = {value}", name = field.0, value = field.1);
        }
        println!("}}")
    }
}

fn to_schema(definitions: &BTreeMap<String, Vec<Definition>>) -> Schema {
    let custom_schemas = definitions
        .iter()
        .map(|(name, def)| {
            let fields = def
                .last()
                .unwrap()
                .fields
                .iter()
                .map(|(name, def)| format!("{name}: {}!", def.trustfall_kind()))
                .collect::<Vec<_>>()
                .join("\n");

            let field_type = format!("{name}Fields");

            format!(
                r#"

        type {field_type} {{
            {fields}
        }}

        type {name} implements Record {{
            at: String!
            kind: String!
            fields: {field_type}!
        }}
        "#
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let schema = format!(
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

{}
"#,
        Schema::ALL_DIRECTIVE_DEFINITIONS,
        custom_schemas
    );
    trace!(%schema, "Using schema");
    Schema::parse(schema).unwrap()
}

struct PlaixtAdapter {
    records: Vec<Record>,
}

#[derive(Clone, Debug)]
enum PlaixtVertex {
    Record(Record),
    Fields {
        name: String,
        values: BTreeMap<String, KdlValue>,
    },
}

impl PlaixtVertex {
    fn as_fields(&self) -> Option<&BTreeMap<String, KdlValue>> {
        if let Self::Fields { values, .. } = self {
            Some(values)
        } else {
            None
        }
    }

    fn as_record(&self) -> Option<&Record> {
        if let Self::Record(v) = self {
            Some(v)
        } else {
            None
        }
    }

    fn typename(&self) -> String {
        match self {
            PlaixtVertex::Record { .. } => "Record".to_string(),
            PlaixtVertex::Fields { name, .. } => name.clone(),
        }
    }
}

impl<'a> Adapter<'a> for PlaixtAdapter {
    type Vertex = PlaixtVertex;

    fn resolve_starting_vertices(
        &self,
        edge_name: &Arc<str>,
        _parameters: &trustfall::provider::EdgeParameters,
        _resolve_info: &trustfall::provider::ResolveInfo,
    ) -> trustfall::provider::VertexIterator<'a, Self::Vertex> {
        match edge_name.as_ref() {
            "RecordsAll" => Box::new(self.records.clone().into_iter().map(PlaixtVertex::Record)),
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
                    Some(_record) => _record.typename().into(),
                    None => FieldValue::Null,
                };

                (ctx, value)
            })),
            (_, "at") => resolve_property_with(
                contexts,
                field_property!(as_record, at, { at.to_string().into() }),
            ),
            (_, "kind") => resolve_property_with(contexts, field_property!(as_record, kind)),
            (name, field) => {
                debug!(?name, ?field, "Asking for properties");

                let field = field.to_string();
                resolve_property_with(contexts, move |vertex| {
                    trace!(?vertex, ?field, "Getting property");
                    let fields = vertex.as_fields().unwrap();
                    match fields.get(&field).unwrap().clone() {
                        KdlValue::Bool(b) => FieldValue::Boolean(b),
                        KdlValue::Float(f) => FieldValue::Float64(f),
                        KdlValue::Null => FieldValue::Null,
                        KdlValue::Integer(i) => FieldValue::Int64(i.try_into().unwrap()),
                        KdlValue::String(s) => FieldValue::String(s.into()),
                    }
                })
            }
        }
    }

    fn resolve_neighbors<V: trustfall::provider::AsVertex<Self::Vertex> + 'a>(
        &self,
        contexts: trustfall::provider::ContextIterator<'a, V>,
        _type_name: &Arc<str>,
        edge_name: &Arc<str>,
        _parameters: &trustfall::provider::EdgeParameters,
        _resolve_info: &trustfall::provider::ResolveEdgeInfo,
    ) -> trustfall::provider::ContextOutcomeIterator<
        'a,
        V,
        trustfall::provider::VertexIterator<'a, Self::Vertex>,
    > {
        match edge_name.as_ref() {
            "fields" => resolve_neighbors_with(contexts, |c| {
                Box::new(
                    c.as_record()
                        .map(|r| PlaixtVertex::Fields {
                            name: format!("{}Fields", r.kind),
                            values: r.fields.clone(),
                        })
                        .into_iter(),
                )
            }),
            _ => unreachable!(),
        }
    }

    fn resolve_coercion<V: trustfall::provider::AsVertex<Self::Vertex> + 'a>(
        &self,
        contexts: trustfall::provider::ContextIterator<'a, V>,
        type_name: &Arc<str>,
        coerce_to_type: &Arc<str>,
        _resolve_info: &trustfall::provider::ResolveInfo,
    ) -> trustfall::provider::ContextOutcomeIterator<'a, V, bool> {
        debug!("Asking to coerce {type_name} into {coerce_to_type}");
        let coerce_to_type = coerce_to_type.clone();
        resolve_coercion_with(contexts, move |node| {
            node.as_record()
                .map(|r| r.kind == *coerce_to_type)
                .unwrap_or(false)
        })
    }
}
