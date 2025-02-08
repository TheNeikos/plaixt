use std::collections::BTreeMap;
use std::collections::HashMap;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use futures::StreamExt;
use futures::TryStreamExt;
use jiff::fmt::temporal::DateTimeParser;
use jiff::Timestamp;
use kdl::KdlDocument;
use kdl::KdlValue;
use miette::IntoDiagnostic;
use miette::LabeledSpan;
use miette::NamedSource;
use owo_colors::OwoColorize;
use tokio_stream::wrappers::ReadDirStream;

#[derive(Debug, Clone)]
pub struct Record {
    pub(crate) kind: String,
    pub(crate) at: Timestamp,
    pub(crate) fields: BTreeMap<String, KdlValue>,
}

pub(crate) fn parse_timestamp(value: &str) -> miette::Result<Timestamp> {
    let parser = DateTimeParser::new();

    parser
        .parse_timestamp(value)
        .or_else(|_| {
            parser
                .parse_datetime(value)
                .and_then(|date| date.in_tz("UTC").map(|z| z.timestamp()))
        })
        .or_else(|_| {
            parser
                .parse_date(value)
                .and_then(|date| date.in_tz("UTC").map(|z| z.timestamp()))
        })
        .into_diagnostic()
}

pub(crate) fn parse_record(
    bytes: &str,
    definitions: &BTreeMap<String, Vec<Definition>>,
) -> miette::Result<Vec<Record>> {
    let doc: KdlDocument = bytes.parse()?;

    let mut recs = vec![];

    for node in doc.nodes() {
        let Some(def) = definitions.get(node.name().value()) else {
            return Err(miette::diagnostic!(
                labels = vec![LabeledSpan::new_primary_with_span(None, node.name().span())],
                "Unknown record kind"
            ))?;
        };

        let Some(at_entry) = node.entry(0) else {
            return Err(miette::diagnostic!(
                labels = vec![LabeledSpan::new_primary_with_span(None, node.name().span())],
                "Every record has to have a first argument with a datetime formatted as RFC3339."
            ))?;
        };

        let KdlValue::String(at) = at_entry.value() else {
            return Err(miette::diagnostic!(
                labels = vec![LabeledSpan::new_primary_with_span(None, at_entry.span())],
                "This datetime should be a string formatted as RFC3339."
            ))?;
        };

        let Ok(at) = parse_timestamp(at) else {
            return Err(miette::diagnostic!(
                labels = vec![LabeledSpan::new_primary_with_span(None, at_entry.span())],
                "This datetime should be a string formatted as RFC3339."
            ))?;
        };

        let fields = node
            .iter_children()
            .map(|field| {
                let Some(get) = field.get(0) else {
                    return Err(miette::diagnostic!(
                        labels = vec![LabeledSpan::new_primary_with_span(None, at_entry.span())],
                        "This datetime should be a string formatted as RFC3339."
                    ))?;
                };
                Ok::<_, miette::Report>((field.name().clone(), get.clone()))
            })
            .map(|val| match val {
                Ok((name, val)) => {
                    let matching_def =
                        &def[def.partition_point(|v| v.since > at).saturating_sub(1)];

                    let kind = &matching_def.fields[name.value()];

                    if let Err(e) = kind.validate(&val) {
                        Err(miette::diagnostic!(
                            labels = vec![LabeledSpan::new_primary_with_span(
                                Some(String::from("here")),
                                name.span()
                            )],
                            help = e,
                            "This field has the wrong kind."
                        ))?;
                    }

                    Ok((name.to_string(), val))
                }
                Err(err) => Err(err),
            })
            .collect::<Result<_, _>>()?;

        recs.push(Record {
            kind: node.name().to_string(),
            at,
            fields,
        });
    }

    Ok(recs)
}

pub(crate) async fn load_records(
    path: &Utf8Path,
    definitions: &BTreeMap<String, Vec<Definition>>,
) -> miette::Result<Vec<Record>> {
    let defs = ReadDirStream::new(tokio::fs::read_dir(path).await.into_diagnostic()?)
        .map_err(miette::Report::from_err)
        .and_then(|entry| async move {
            if entry.file_type().await.into_diagnostic()?.is_file() {
                Ok(Some((
                    Utf8PathBuf::from_path_buf(entry.path().to_path_buf()).unwrap(),
                    tokio::fs::read_to_string(entry.path())
                        .await
                        .into_diagnostic()?,
                )))
            } else {
                Ok(None)
            }
        })
        .flat_map(|val| futures::stream::iter(val.transpose()))
        .and_then(|(name, bytes)| async move {
            parse_record(&bytes, definitions)
                .map_err(|e| e.with_source_code(NamedSource::new(name, bytes).with_language("kdl")))
        })
        .map(|val| val.map(|recs| futures::stream::iter(recs).map(Ok::<_, miette::Report>)))
        .try_flatten()
        .try_collect()
        .await?;

    Ok(defs)
}

#[derive(Debug)]
pub enum DefinitionKind {
    String,
    OneOf(Vec<String>),
}

impl DefinitionKind {
    pub(crate) fn trustfall_kind(&self) -> String {
        match self {
            DefinitionKind::String => String::from("String"),
            DefinitionKind::OneOf(_vecs) => String::from("String"),
        }
    }

    pub(crate) fn validate(&self, val: &KdlValue) -> Result<(), String> {
        match self {
            DefinitionKind::String => val
                .is_string()
                .then_some(())
                .ok_or("Expected a string here".to_string()),
            DefinitionKind::OneOf(options) => val
                .as_string()
                .is_some_and(|val| options.iter().any(|o| o == val))
                .then_some(())
                .ok_or_else(|| format!("Expected one of: {}", options.join(", "))),
        }
    }
}

impl TryFrom<&str> for DefinitionKind {
    type Error = miette::Report;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_ascii_lowercase().as_str() {
            "string" => Ok(DefinitionKind::String),
            other => miette::bail!("Did not recognize valid field kind: \"{other}\""),
        }
    }
}

#[derive(Debug)]
pub struct Definition {
    pub(crate) since: Timestamp,
    pub(crate) fields: HashMap<String, DefinitionKind>,
}

pub(crate) fn parse_definition(bytes: &str) -> miette::Result<Vec<Definition>> {
    let doc: KdlDocument = bytes.parse()?;

    let mut defs = vec![];

    for node in doc.nodes() {
        match node.name().value() {
            "define" => {
                let Some(since_entry) = node.entry("since") else {
                    return Err(miette::diagnostic!(
                        labels = vec![LabeledSpan::new_primary_with_span(
                            Some(String::from("this define")),
                            node.name().span()
                        )],
                        "Missing `since` property. Every `define` block requires one."
                    ))?;
                };

                let KdlValue::String(since) = since_entry.value() else {
                    return Err(miette::diagnostic!(
                        labels = vec![LabeledSpan::new_primary_with_span(
                            Some(String::from("in this define")),
                            since_entry.span()
                        )],
                        "The `since` property needs to be a string in RFC3339 format."
                    ))?;
                };

                let since = match parse_timestamp(since) {
                    Ok(since) => since,
                    Err(_err) => {
                        return Err(miette::diagnostic!(
                            labels = vec![LabeledSpan::new_primary_with_span(
                                Some(String::from("in this define")),
                                since_entry.span()
                            )],
                            "Could not parse the `since` property as a valid RFC3339 time"
                        ))?;
                    }
                };

                let Some(fields) = node
                    .iter_children()
                    .find(|field| field.name().value() == "fields")
                else {
                    return Err(miette::diagnostic!(
                        labels = vec![LabeledSpan::new_primary_with_span(
                            Some(String::from("in this define")),
                            node.span()
                        )],
                        "Could not find `fields` child, which is a required child node."
                    ))?;
                };

                let fields = fields
                    .iter_children()
                    .map(|field| {
                        let kind = if let Some(kind) = field.get("is") {
                            kind.as_string()
                                .ok_or_else(|| {
                                    miette::Report::from(miette::diagnostic!(
                                        labels = vec![LabeledSpan::new_primary_with_span(
                                            Some(String::from("in this define")),
                                            field.span()
                                        )],
                                        "The `is` field needs to be a string."
                                    ))
                                })
                                .and_then(DefinitionKind::try_from)?
                        } else {
                            let Some(children) = field.children() else {
                                return Err(miette::diagnostic!(
                                    labels = vec![LabeledSpan::new_primary_with_span(
                                        Some(String::from("in this define")),
                                        field.span()
                                    )],
                                    "Either set a `is` property, or a child with the given definition"
                                ))?;
                            };

                            if let Some(one_of) = children.get("oneOf") {
                                DefinitionKind::OneOf(
                                    one_of.iter().map(|opt| opt.value().to_string()).collect(),
                                )
                            } else {
                                return Err(miette::diagnostic!(
                                    labels = vec![LabeledSpan::new_primary_with_span(
                                        Some(String::from("in this define")),
                                        field.span()
                                    )],
                                    "Unrecognizable field definition"
                                ))?;
                            }
                        };

                        match field.name().value() {
                            "at" | "kind" => return Err(miette::diagnostic!(
                                    labels = vec![LabeledSpan::new_primary_with_span(
                                        Some(String::from("this name")),
                                        field.name().span()
                                    )],
                                    help = "Both `at` and `kind` are reserved field names.",
                                    "Reserved field name."
                                    ))?,
                            _ => {}
                        }

                        Ok((field.name().to_string(), kind))
                    })
                    .collect::<miette::Result<_>>()?;

                defs.push(Definition { since, fields });
            }
            unknown => {
                return Err(miette::diagnostic!(
                    labels = vec![LabeledSpan::new_primary_with_span(
                        Some(String::from("here")),
                        node.name().span()
                    )],
                    help = "Allowed nodes are: \"define\"",
                    "Unknown node \"{}\".",
                    unknown.red(),
                ))?
            }
        }
    }

    defs.sort_by_key(|d| d.since);

    Ok(defs)
}

pub(crate) async fn load_definitions(
    path: &Utf8Path,
) -> miette::Result<BTreeMap<String, Vec<Definition>>> {
    let defs = ReadDirStream::new(tokio::fs::read_dir(path).await.into_diagnostic()?)
        .map_err(miette::Report::from_err)
        .and_then(|entry| async move {
            if entry.file_type().await.into_diagnostic()?.is_file() {
                Ok(Some((
                    Utf8PathBuf::from_path_buf(entry.path().to_path_buf()).unwrap(),
                    tokio::fs::read_to_string(entry.path())
                        .await
                        .into_diagnostic()?,
                )))
            } else {
                Ok(None)
            }
        })
        .flat_map(|val| futures::stream::iter(val.transpose()))
        .and_then(|(name, bytes)| async move {
            Ok((
                name.file_stem().unwrap().to_string(),
                parse_definition(&bytes).map_err(|e| {
                    e.with_source_code(NamedSource::new(name, bytes).with_language("kdl"))
                })?,
            ))
        })
        .try_collect()
        .await?;

    Ok(defs)
}
