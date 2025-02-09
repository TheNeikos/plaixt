use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::OnceLock;

use paperless_rs::PaperlessClient;
use trustfall::provider::resolve_coercion_using_schema;
use trustfall::provider::resolve_property_with;
use trustfall::provider::AsVertex;
use trustfall::provider::ContextIterator;
use trustfall::provider::ContextOutcomeIterator;
use trustfall::provider::EdgeParameters;
use trustfall::provider::ResolveEdgeInfo;
use trustfall::provider::ResolveInfo;
use trustfall::provider::Typename;
use trustfall::provider::VertexIterator;
use trustfall::FieldValue;
use trustfall::Schema;

use super::vertex::Vertex;
use crate::parsing::DefinitionKind;
use crate::parsing::Record;

static SCHEMA: OnceLock<Schema> = OnceLock::new();

#[non_exhaustive]
pub struct Adapter {
    schema: Arc<Schema>,
    records: Vec<Record>,
    definitions: Arc<BTreeMap<String, BTreeMap<String, DefinitionKind>>>,
    paperless_client: Option<PaperlessClient>,
    runtime_handle: tokio::runtime::Handle,
}

impl std::fmt::Debug for Adapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Adapter").finish_non_exhaustive()
    }
}

impl Adapter {
    pub fn new(
        schema: Schema,
        records: Vec<Record>,
        definitions: BTreeMap<String, BTreeMap<String, DefinitionKind>>,
        paperless_client: Option<PaperlessClient>,
        runtime: tokio::runtime::Handle,
    ) -> Self {
        Self {
            schema: Arc::new(schema),
            records,
            definitions: Arc::new(definitions),
            paperless_client,
            runtime_handle: runtime,
        }
    }

    pub const SCHEMA_TEXT: &'static str = include_str!("./schema.graphql");

    pub fn schema() -> &'static Schema {
        SCHEMA.get_or_init(|| Schema::parse(Self::SCHEMA_TEXT).expect("not a valid schema"))
    }
}

impl<'a> trustfall::provider::Adapter<'a> for Adapter {
    type Vertex = Vertex;

    fn resolve_starting_vertices(
        &self,
        edge_name: &Arc<str>,
        _parameters: &EdgeParameters,
        resolve_info: &ResolveInfo,
    ) -> VertexIterator<'a, Self::Vertex> {
        match edge_name.as_ref() {
            "Records" => super::entrypoints::records(resolve_info, &self.records),
            _ => {
                unreachable!(
                    "attempted to resolve starting vertices for unexpected edge name: {edge_name}"
                )
            }
        }
    }

    fn resolve_property<V: AsVertex<Self::Vertex> + 'a>(
        &self,
        contexts: ContextIterator<'a, V>,
        type_name: &Arc<str>,
        property_name: &Arc<str>,
        resolve_info: &ResolveInfo,
    ) -> ContextOutcomeIterator<'a, V, FieldValue> {
        if property_name.as_ref() == "__typename" {
            return resolve_property_with(contexts, |vertex| vertex.typename().into());
        }
        match type_name.as_ref() {
            "PaperlessDocument" => super::properties::resolve_paperless_document_property(
                contexts,
                property_name.as_ref(),
                resolve_info,
            ),
            "Path" => super::properties::resolve_path_property(
                contexts,
                property_name.as_ref(),
                resolve_info,
            ),
            "File" => super::properties::resolve_file_property(
                contexts,
                property_name.as_ref(),
                resolve_info,
            ),
            "Directory" => super::properties::resolve_directory_property(
                contexts,
                property_name.as_ref(),
                resolve_info,
            ),
            "Record" => {
                super::properties::resolve_record_property(contexts, property_name, resolve_info)
            }
            kind if kind.starts_with("p_") => {
                super::properties::resolve_record_property(contexts, property_name, resolve_info)
            }
            _ => {
                unreachable!(
                    "attempted to read property '{property_name}' on unexpected type: {type_name}"
                )
            }
        }
    }

    fn resolve_neighbors<V: AsVertex<Self::Vertex> + 'a>(
        &self,
        contexts: ContextIterator<'a, V>,
        type_name: &Arc<str>,
        edge_name: &Arc<str>,
        parameters: &EdgeParameters,
        resolve_info: &ResolveEdgeInfo,
    ) -> ContextOutcomeIterator<'a, V, VertexIterator<'a, Self::Vertex>> {
        match type_name.as_ref() {
            "Directory" => super::edges::resolve_directory_edge(
                contexts,
                edge_name.as_ref(),
                parameters,
                resolve_info,
            ),
            kind if kind.starts_with("p_") => super::edges::resolve_record_edge(
                contexts,
                edge_name,
                parameters,
                resolve_info,
                &self.definitions,
            ),
            _ => {
                unreachable!(
                    "attempted to resolve edge '{edge_name}' on unexpected type: {type_name}"
                )
            }
        }
    }

    fn resolve_coercion<V: AsVertex<Self::Vertex> + 'a>(
        &self,
        contexts: ContextIterator<'a, V>,
        _type_name: &Arc<str>,
        coerce_to_type: &Arc<str>,
        _resolve_info: &ResolveInfo,
    ) -> ContextOutcomeIterator<'a, V, bool> {
        let schema = self.schema.clone();
        let coerce_to_type = coerce_to_type.clone();

        Box::new(contexts.map(move |ctx| {
            let subtypes: BTreeSet<_> = schema
                .subtypes(coerce_to_type.as_ref())
                .unwrap_or_else(|| panic!("type {coerce_to_type} is not part of this schema"))
                .collect();

            match ctx.active_vertex::<Vertex>() {
                None => (ctx, false),
                Some(vertex) => {
                    let typename = vertex.typename();
                    let can_coerce = subtypes.contains(typename);
                    (ctx, can_coerce)
                }
            }
        }))
    }
}
