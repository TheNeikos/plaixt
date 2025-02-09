use camino::Utf8PathBuf;
use paperless_rs::endpoint::documents::Document as PaperlessDocument;

use crate::parsing::Record;

#[non_exhaustive]
#[derive(Debug, Clone, trustfall::provider::TrustfallEnumVertex)]
pub enum Vertex {
    Path(Utf8PathBuf),
    File(Utf8PathBuf),
    Directory(Utf8PathBuf),

    PaperlessDocument(PaperlessDocument),
    Record(Record),
}
