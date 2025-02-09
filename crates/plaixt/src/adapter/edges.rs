use std::collections::BTreeMap;
use std::sync::Arc;

use trustfall::provider::resolve_neighbors_with;
use trustfall::provider::AsVertex;
use trustfall::provider::ContextIterator;
use trustfall::provider::ContextOutcomeIterator;
use trustfall::provider::EdgeParameters;
use trustfall::provider::ResolveEdgeInfo;
use trustfall::provider::VertexIterator;

use super::Vertex;
use crate::parsing::DefinitionKind;

pub(super) fn resolve_directory_edge<'a, V: AsVertex<Vertex> + 'a>(
    contexts: ContextIterator<'a, V>,
    edge_name: &str,
    _parameters: &EdgeParameters,
    resolve_info: &ResolveEdgeInfo,
) -> ContextOutcomeIterator<'a, V, VertexIterator<'a, Vertex>> {
    match edge_name {
        "Children" => directory::children(contexts, resolve_info),
        _ => unreachable!("attempted to resolve unexpected edge '{edge_name}' on type 'Directory'"),
    }
}

mod directory {
    use camino::Utf8Path;
    use trustfall::provider::resolve_neighbors_with;
    use trustfall::provider::AsVertex;
    use trustfall::provider::ContextIterator;
    use trustfall::provider::ContextOutcomeIterator;
    use trustfall::provider::ResolveEdgeInfo;
    use trustfall::provider::VertexIterator;

    use crate::adapter::Vertex;

    pub(super) fn children<'a, V: AsVertex<Vertex> + 'a>(
        contexts: ContextIterator<'a, V>,
        _resolve_info: &ResolveEdgeInfo,
    ) -> ContextOutcomeIterator<'a, V, VertexIterator<'a, Vertex>> {
        resolve_neighbors_with(contexts, move |vertex| {
            let vertex = vertex
                .as_directory()
                .expect("conversion failed, vertex was not a Directory");

            fn read_children(path: &Utf8Path) -> Option<impl Iterator<Item = Vertex>> {
                Some(
                    path.read_dir_utf8()
                        .ok()?
                        .flat_map(|item| Some(Vertex::Path(item.ok()?.path().to_path_buf()))),
                )
            }

            read_children(vertex)
                .map(|i| {
                    let it: Box<dyn Iterator<Item = Vertex>> = Box::new(i);
                    it
                })
                .unwrap_or_else(|| Box::new(std::iter::empty()))
        })
    }
}

pub(super) fn resolve_record_edge<'a, V: AsVertex<Vertex> + 'a>(
    contexts: ContextIterator<'a, V>,
    edge_name: &Arc<str>,
    _parameters: &EdgeParameters,
    _resolve_info: &ResolveEdgeInfo,
    definitions: &Arc<BTreeMap<String, BTreeMap<String, DefinitionKind>>>,
) -> ContextOutcomeIterator<'a, V, VertexIterator<'a, Vertex>> {
    let edge_name = edge_name.clone();
    let definitions = definitions.clone();
    resolve_neighbors_with(contexts, move |v| {
        let rec = v.as_record().expect("Expected a record");
        let def = &definitions[&rec.kind][edge_name.as_ref()];

        match def {
            DefinitionKind::Path => Box::new(std::iter::once(Vertex::Path(
                rec.fields[edge_name.as_ref()].as_string().unwrap().into(),
            ))),
            _ => unreachable!("Only `Path` can appear as edge for now"),
        }
    })
}
