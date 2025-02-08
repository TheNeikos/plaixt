use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt::Write;
use std::ops::Not;
use std::sync::Arc;

use kdl::KdlValue;
use tracing::debug;
use tracing::trace;
use trustfall::provider::field_property;
use trustfall::provider::resolve_coercion_with;
use trustfall::provider::resolve_neighbors_with;
use trustfall::provider::resolve_property_with;
use trustfall::provider::Adapter;
use trustfall::provider::AsVertex;
use trustfall::FieldValue;
use trustfall::Schema;

use crate::parsing::Definition;
use crate::parsing::Record;

const ADAPTER_SEP: &str = "__";

#[derive(Debug, Default)]
pub struct StartingVertex {
    adapter_name: String,
    start_vertex_name: String,
    vertex_type: String,
}

impl StartingVertex {
    pub fn new(adapter_name: String, start_vertex_name: String, start_vertex_type: String) -> Self {
        Self {
            adapter_name,
            start_vertex_name,
            vertex_type: start_vertex_type,
        }
    }

    pub fn schema_name(&self) -> String {
        format!(
            "{}{ADAPTER_SEP}{}",
            self.adapter_name, self.start_vertex_name
        )
    }

    pub fn vertex_type(&self) -> &str {
        &self.vertex_type
    }
}

#[derive(Debug, Default)]
pub struct VertexType {
    adapter_name: String,
    vertex_name: String,
    vertex_fields: HashMap<String, String>,
    implements: Vec<String>,
}

impl VertexType {
    pub fn new(
        adapter_name: String,
        vertex_name: String,
        vertex_fields: HashMap<String, String>,
        implements: Vec<String>,
    ) -> Self {
        Self {
            adapter_name,
            vertex_name,
            vertex_fields,
            implements,
        }
    }

    pub fn schema_name(&self) -> String {
        format!("{}{ADAPTER_SEP}{}", self.adapter_name, self.vertex_name)
    }

    pub fn schema_type(&self) -> String {
        format!(
            r#"type {name} {impls} {{ {fields} }}"#,
            name = self.schema_name(),
            impls = self
                .implements
                .is_empty()
                .not()
                .then(|| format!("implements {}", self.implements.join(" & ")))
                .unwrap_or_else(String::new),
            fields = self.vertex_fields.iter().fold(String::new(), |mut out, f| {
                write!(out, "{}: {}, ", f.0, f.1).unwrap();
                out
            }),
        )
    }
}

#[derive(Debug, Default)]
pub struct DynamicSchema {
    roots: Vec<StartingVertex>,
    types: Vec<VertexType>,
}

impl DynamicSchema {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_root(&mut self, root: StartingVertex) {
        self.roots.push(root);
    }

    pub fn add_type(&mut self, kind: VertexType) {
        self.types.push(kind);
    }
}

pub(crate) fn to_schema(definitions: &BTreeMap<String, Vec<Definition>>) -> Schema {
    let mut schema = DynamicSchema::new();

    schema.add_root(StartingVertex::new(
        "Plaixt".to_string(),
        "RecordsAll".to_string(),
        "[Record!]!".to_string(),
    ));

    for definition in definitions.values().flat_map(|d| d.first()) {
        let fields = VertexType::new(
            "Plaixt".to_string(),
            format!("{}Fields", definition.name),
            definition
                .fields
                .iter()
                .map(|(name, val)| (name.clone(), format!("{}!", val.trustfall_kind())))
                .collect(),
            vec![],
        );
        schema.add_type(VertexType::new(
            "Plaixt".to_string(),
            definition.name.clone(),
            [
                (String::from("at"), String::from("String!")),
                (String::from("kind"), String::from("String!")),
                (String::from("fields"), format!("{}!", fields.schema_name())),
            ]
            .into(),
            vec![String::from("Record")],
        ));
        schema.add_type(fields);
    }

    let schema = format!(
        r#"schema {{
            query: RootSchemaQuery
        }}
        {}
        type RootSchemaQuery {{
            {roots}
        }}
        interface Record {{
            at: String!,
            kind: String!,
        }}

        {types}
        "#,
        Schema::ALL_DIRECTIVE_DEFINITIONS,
        roots = schema.roots.iter().fold(String::new(), |mut out, r| {
            write!(out, "{}: {}, ", r.schema_name(), r.vertex_type()).unwrap();
            out
        }),
        types = schema.types.iter().fold(String::new(), |mut out, t| {
            writeln!(out, "{}", t.schema_type()).unwrap();
            out
        }),
    );
    trace!(%schema, "Using schema");
    Schema::parse(schema).unwrap()
}

pub struct TrustfallMultiAdapter {
    pub plaixt: PlaixtAdapter,
}

#[derive(Debug, Clone)]
pub enum TrustfallMultiVertex {
    Plaixt(PlaixtVertex),
}

impl AsVertex<PlaixtVertex> for TrustfallMultiVertex {
    fn as_vertex(&self) -> Option<&PlaixtVertex> {
        self.as_plaixt()
    }

    fn into_vertex(self) -> Option<PlaixtVertex> {
        self.as_plaixt().cloned()
    }
}

impl TrustfallMultiVertex {
    pub fn as_plaixt(&self) -> Option<&PlaixtVertex> {
        if let Self::Plaixt(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

impl<'v> Adapter<'v> for TrustfallMultiAdapter {
    type Vertex = TrustfallMultiVertex;

    fn resolve_starting_vertices(
        &self,
        edge_name: &Arc<str>,
        parameters: &trustfall::provider::EdgeParameters,
        resolve_info: &trustfall::provider::ResolveInfo,
    ) -> trustfall::provider::VertexIterator<'v, Self::Vertex> {
        let (adapter_name, edge_name) = edge_name.split_once(ADAPTER_SEP).unwrap();

        trace!(?adapter_name, ?edge_name, "Got start vertex");

        match adapter_name {
            "Plaixt" => {
                let iter = self.plaixt.resolve_starting_vertices(
                    &Arc::from(edge_name),
                    parameters,
                    resolve_info,
                );

                Box::new(iter.map(TrustfallMultiVertex::Plaixt))
            }
            _ => unreachable!(),
        }
    }

    fn resolve_property<V>(
        &self,
        contexts: trustfall::provider::ContextIterator<'v, V>,
        type_name: &Arc<str>,
        property_name: &Arc<str>,
        resolve_info: &trustfall::provider::ResolveInfo,
    ) -> trustfall::provider::ContextOutcomeIterator<'v, V, FieldValue>
    where
        V: trustfall::provider::AsVertex<Self::Vertex> + 'v,
    {
        let (adapter_name, type_name) = type_name.split_once(ADAPTER_SEP).unwrap();

        match adapter_name {
            "Plaixt" => {
                let contexts = contexts.collect::<Vec<_>>();

                let properties = self.plaixt.resolve_property(
                    Box::new(
                        contexts
                            .clone()
                            .into_iter()
                            .map(|v| v.flat_map(&mut |v: V| v.into_vertex())),
                    ),
                    &Arc::from(type_name),
                    property_name,
                    resolve_info,
                );

                Box::new(
                    properties
                        .into_iter()
                        .zip(contexts)
                        .map(|((_ctx, name), og_ctx)| (og_ctx, name)),
                )
            }
            _ => unreachable!(),
        }
    }

    fn resolve_neighbors<V: trustfall::provider::AsVertex<Self::Vertex> + 'v>(
        &self,
        contexts: trustfall::provider::ContextIterator<'v, V>,
        type_name: &Arc<str>,
        edge_name: &Arc<str>,
        parameters: &trustfall::provider::EdgeParameters,
        resolve_info: &trustfall::provider::ResolveEdgeInfo,
    ) -> trustfall::provider::ContextOutcomeIterator<
        'v,
        V,
        trustfall::provider::VertexIterator<'v, Self::Vertex>,
    > {
        let (adapter_name, type_name) = type_name.split_once(ADAPTER_SEP).unwrap();

        match adapter_name {
            "Plaixt" => {
                let contexts = contexts.collect::<Vec<_>>();

                let properties = self.plaixt.resolve_neighbors(
                    Box::new(
                        contexts
                            .clone()
                            .into_iter()
                            .map(|v| v.flat_map(&mut |v: V| v.into_vertex())),
                    ),
                    &Arc::from(type_name),
                    edge_name,
                    parameters,
                    resolve_info,
                );

                Box::new(
                    properties
                        .into_iter()
                        .zip(contexts)
                        .map(|((_ctx, vals), og_ctx)| {
                            (
                                og_ctx,
                                Box::new(vals.map(TrustfallMultiVertex::Plaixt)) as Box<_>,
                            )
                        }),
                )
            }
            _ => unreachable!(),
        }
    }

    fn resolve_coercion<V: trustfall::provider::AsVertex<Self::Vertex> + 'v>(
        &self,
        contexts: trustfall::provider::ContextIterator<'v, V>,
        type_name: &Arc<str>,
        coerce_to_type: &Arc<str>,
        resolve_info: &trustfall::provider::ResolveInfo,
    ) -> trustfall::provider::ContextOutcomeIterator<'v, V, bool> {
        trace!(?type_name, ?coerce_to_type, "Trying to coerce");
        let (adapter_name, coerce_to_type) = coerce_to_type.split_once(ADAPTER_SEP).unwrap();

        match adapter_name {
            "Plaixt" => {
                let contexts = contexts.collect::<Vec<_>>();

                let properties = self.plaixt.resolve_coercion(
                    Box::new(
                        contexts
                            .clone()
                            .into_iter()
                            .map(|v| v.flat_map(&mut |v: V| v.into_vertex())),
                    ),
                    type_name,
                    &Arc::from(coerce_to_type),
                    resolve_info,
                );

                Box::new(
                    properties
                        .into_iter()
                        .zip(contexts)
                        .map(|((_ctx, val), og_ctx)| (og_ctx, val)),
                )
            }
            _ => unreachable!(),
        }
    }
}

pub(crate) struct PlaixtAdapter {
    pub(crate) records: Vec<Record>,
}

#[derive(Clone, Debug)]
pub(crate) enum PlaixtVertex {
    Record(Record),
    Fields {
        name: String,
        values: BTreeMap<String, KdlValue>,
    },
}

impl PlaixtVertex {
    pub(crate) fn as_fields(&self) -> Option<&BTreeMap<String, KdlValue>> {
        if let Self::Fields { values, .. } = self {
            Some(values)
        } else {
            None
        }
    }

    pub(crate) fn as_record(&self) -> Option<&Record> {
        if let Self::Record(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub(crate) fn typename(&self) -> String {
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
        trace!(?edge_name, "Resolving start vertex");
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
