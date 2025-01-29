#![allow(dead_code)]
use std::collections::BTreeMap;
use std::collections::HashMap;

use camino::Utf8PathBuf;
use clap::Parser;
use futures::StreamExt;
use futures::TryStreamExt;
use kdl::KdlDocument;
use kdl::KdlValue;
use miette::IntoDiagnostic;
use miette::LabeledSpan;
use miette::NamedSource;
use owo_colors::OwoColorize;
use time::OffsetDateTime;
use tokio_stream::wrappers::ReadDirStream;
use tracing::info;

#[derive(Debug, Parser)]
struct Args {
    #[arg(short, long)]
    path: Utf8PathBuf,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt().pretty().init();

    let args = Args::parse();

    let definitions = load_definitions(args.path.join("definitions")).await?;

    info!(?definitions, "Got definitions!");

    Ok(())
}

#[derive(Debug)]
pub enum DefinitionKind {
    String,
    OneOf(Vec<String>),
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
    since: OffsetDateTime,
    fields: HashMap<String, DefinitionKind>,
}

fn parse_definition(bytes: &str) -> miette::Result<Vec<Definition>> {
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

                let since = match OffsetDateTime::parse(
                    since,
                    &time::format_description::well_known::Rfc3339,
                ) {
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

    Ok(defs)
}

async fn load_definitions(path: Utf8PathBuf) -> miette::Result<BTreeMap<String, Vec<Definition>>> {
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
