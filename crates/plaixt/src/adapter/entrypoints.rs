use trustfall::provider::ResolveInfo;
use trustfall::provider::VertexIterator;

use super::vertex::Vertex;
use crate::parsing::Record;

pub(super) fn records<'a>(
    _resolve_info: &ResolveInfo,
    records: &'_ [Record],
) -> VertexIterator<'a, Vertex> {
    #[expect(
        clippy::unnecessary_to_owned,
        reason = "We have to go through a vec to satisfy the lifetimes"
    )]
    Box::new(records.to_vec().into_iter().map(Vertex::Record))
}
