//! Trasea core: raster illustration / logo -> clean SVG / PDF. No UI, no network.
//!
//! Pipeline (see `pipeline::vectorize`): load -> thin-line guard (from the ORIGINAL image) -> AI clean
//! (Real-ESRGAN sidecar, median fallback) -> palette lock -> layer tracing (visioncortex, the VTracer engine)
//! -> thin lines stacked on top -> tidy + export.
pub mod centerline;
pub mod clean;
pub mod export;
pub mod fit;
pub mod lines;
pub mod load;
pub mod palette;
pub mod pipeline;
pub mod primitives;
pub mod stats;
pub mod symmetry;
pub mod text_detect;
pub mod trace;

pub use pipeline::{vectorize, Output, Params, Progress};
pub use symmetry::SymmetryMode;
pub use text_detect::TextRegion;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read image: {0}")]
    Load(String),
    #[error("{0}")]
    Other(String),
}
pub type Result<T> = std::result::Result<T, Error>;

pub type Rgb = [u8; 3];

pub fn hex(c: Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}
