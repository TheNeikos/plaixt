use camino::Utf8Path;
use camino::Utf8PathBuf;
use kdl::KdlDocument;
use miette::Context;
use miette::LabeledSpan;

#[derive(Debug)]
pub struct Config {
    pub(crate) root_folder: Utf8PathBuf,
}

pub(crate) async fn parse_config(path: &Utf8Path) -> miette::Result<Config> {
    let data = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| miette::miette!(e))
        .wrap_err_with(|| miette::miette!("Could not read configuration at \"{path}\""))?;

    let doc: KdlDocument = data
        .parse()
        .map_err(|e| miette::Error::from(e).with_source_code(data.clone()))?;

    Ok(Config {
        root_folder: doc
            .get("root_folder")
            .ok_or_else(|| miette::miette!("\"root_folder\" configuration value not found"))
            .and_then(|val| {
                val.get(0)
                    .and_then(|v| v.as_string().map(Into::into))
                    .ok_or_else(|| {
                        miette::diagnostic!(
                            labels = vec![LabeledSpan::new_primary_with_span(None, val.span())],
                            "root_folder is expected to be a path"
                        )
                        .into()
                    })
                    .map_err(|e: miette::Report| e.with_source_code(data))
            })?,
    })
}
