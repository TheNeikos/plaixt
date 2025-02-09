use std::collections::BTreeMap;
use std::sync::Arc;

use kdl::KdlValue;
use paperless_rs::PaperlessClient;
use trustfall::provider::field_property;
use trustfall::provider::resolve_property_with;
use trustfall::provider::AsVertex;
use trustfall::provider::ContextIterator;
use trustfall::provider::ContextOutcomeIterator;
use trustfall::provider::ResolveInfo;
use trustfall::FieldValue;

use super::vertex::Vertex;

pub(super) fn resolve_path_property<'a, V: AsVertex<Vertex> + 'a>(
    contexts: ContextIterator<'a, V>,
    property_name: &str,
    _resolve_info: &ResolveInfo,
) -> ContextOutcomeIterator<'a, V, FieldValue> {
    match property_name {
        "exists" => resolve_property_with(contexts, move |v: &Vertex| {
            let path = v.as_path().expect("vertex was not a Path");

            path.exists().into()
        }),
        "basename" => resolve_property_with(contexts, move |v: &Vertex| {
            let path = v.as_path().expect("vertex was not a Path");

            path.file_name().into()
        }),
        "path" => resolve_property_with(contexts, move |v: &Vertex| {
            let path = v.as_path().expect("vertex was not a Path");

            path.to_string().into()
        }),
        _ => {
            unreachable!("attempted to read unexpected property '{property_name}' on type 'Path'")
        }
    }
}

pub(super) fn resolve_directory_property<'a, V: AsVertex<Vertex> + 'a>(
    contexts: ContextIterator<'a, V>,
    property_name: &str,
    _resolve_info: &ResolveInfo,
) -> ContextOutcomeIterator<'a, V, FieldValue> {
    match property_name {
        "exists" => resolve_property_with(contexts, move |v: &Vertex| {
            let directory = v.as_directory().expect("vertex was not a Directory");

            directory.exists().into()
        }),
        "basename" => resolve_property_with(contexts, move |v: &Vertex| {
            let directory = v.as_directory().expect("vertex was not a Directory");

            directory.file_name().into()
        }),
        "path" => resolve_property_with(contexts, move |v: &Vertex| {
            let directory = v.as_directory().expect("vertex was not a Directory");

            directory.to_string().into()
        }),
        _ => {
            unreachable!("attempted to read unexpected property '{property_name}' on type 'File'")
        }
    }
}

pub(super) fn resolve_file_property<'a, V: AsVertex<Vertex> + 'a>(
    contexts: ContextIterator<'a, V>,
    property_name: &str,
    _resolve_info: &ResolveInfo,
) -> ContextOutcomeIterator<'a, V, FieldValue> {
    match property_name {
        "exists" => resolve_property_with(contexts, move |v: &Vertex| {
            let file = v.as_file().expect("vertex was not a File");

            file.exists().into()
        }),
        "basename" => resolve_property_with(contexts, move |v: &Vertex| {
            let file = v.as_file().expect("vertex was not a File");

            file.file_name().into()
        }),
        "path" => resolve_property_with(contexts, move |v: &Vertex| {
            let file = v.as_file().expect("vertex was not a File");

            file.to_string().into()
        }),
        "extension" => resolve_property_with(contexts, move |v: &Vertex| {
            let file = v.as_file().expect("vertex was not a File");

            file.extension().into()
        }),
        _ => {
            unreachable!("attempted to read unexpected property '{property_name}' on type 'File'")
        }
    }
}

pub(super) fn resolve_paperless_document_property<'a, V: AsVertex<Vertex> + 'a>(
    contexts: ContextIterator<'a, V>,
    property_name: &str,
    _resolve_info: &ResolveInfo,
) -> ContextOutcomeIterator<'a, V, FieldValue> {
    match property_name {
        "added" => resolve_property_with(contexts, field_property!(as_paperless_document, added)),
        "archive_serial_number" => resolve_property_with(
            contexts,
            field_property!(as_paperless_document, archive_serial_number),
        ),
        "content" => {
            resolve_property_with(contexts, field_property!(as_paperless_document, content))
        }
        "created" => {
            resolve_property_with(contexts, field_property!(as_paperless_document, created))
        }
        "id" => resolve_property_with(contexts, field_property!(as_paperless_document, id)),
        "title" => resolve_property_with(contexts, field_property!(as_paperless_document, title)),
        _ => {
            unreachable!(
                "attempted to read unexpected property '{property_name}' on type 'PaperlessDocument'"
            )
        }
    }
}

pub(super) fn resolve_record_property<'a, V: AsVertex<Vertex> + 'a>(
    contexts: ContextIterator<'a, V>,
    property_name: &Arc<str>,
    _resolve_info: &ResolveInfo,
) -> ContextOutcomeIterator<'a, V, FieldValue> {
    let property_name = property_name.clone();
    match property_name.as_ref() {
        "_at" => resolve_property_with(
            contexts,
            field_property!(as_record, at, { at.to_string().into() }),
        ),
        "_kind" => resolve_property_with(contexts, field_property!(as_record, kind)),
        _ => resolve_property_with(contexts, move |v: &Vertex| {
            let rec = v
                .as_record()
                .expect("Called record property without it being a record");

            kdl_to_trustfall_value(rec.fields[property_name.as_ref()].clone())
        }),
    }
}

fn kdl_to_trustfall_value(val: KdlValue) -> FieldValue {
    match val {
        KdlValue::Bool(b) => FieldValue::Boolean(b),
        KdlValue::Float(f) => FieldValue::Float64(f),
        KdlValue::Null => FieldValue::Null,
        KdlValue::Integer(i) => FieldValue::Int64(i.try_into().unwrap()),
        KdlValue::String(s) => FieldValue::String(s.into()),
    }
}
