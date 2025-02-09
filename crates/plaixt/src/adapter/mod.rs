mod adapter_impl;
mod edges;
mod entrypoints;
mod properties;
mod vertex;

#[cfg(test)]
mod tests;

pub use adapter_impl::Adapter;
use tracing::trace;
use trustfall::Schema;
pub use vertex::Vertex;

pub struct CustomVertex {
    pub name: String,
    pub definition: String,
}

impl crate::parsing::Definition {
    fn to_custom_vertices(&self) -> Vec<CustomVertex> {
        let name = format!("p_{}", self.name);

        let fields = self
            .fields
            .iter()
            .map(|(fname, ftype)| {
                let kind = ftype.trustfall_kind(&format!("{name}{fname}"));
                format!("{fname}: {kind}")
            })
            .chain([String::from("_at: String!"), String::from("_kind: String!")])
            .collect::<Vec<_>>();

        let definition = format!("type {name} implements Record {{ {} }}", fields.join(","));

        [CustomVertex { name, definition }].into_iter().collect()
    }
}

pub(crate) fn to_schema(
    definitions: &std::collections::BTreeMap<String, Vec<crate::parsing::Definition>>,
) -> trustfall::Schema {
    let base_text = Adapter::SCHEMA_TEXT;

    let generated = definitions
        .values()
        .flat_map(|defs| defs.last().unwrap().to_custom_vertices())
        .map(|v| v.definition)
        .collect::<Vec<_>>()
        .join("\n");

    let input = format!("{base_text}{generated}");
    trace!(%input, "Using schema");
    Schema::parse(input).unwrap()
}
