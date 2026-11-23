//! Trasea core: raster illustration / logo -> clean SVG / PDF / EPS / DXF. No UI, no network.
//!
//! Pipeline (see `pipeline::vectorize`): load -> optional non-destructive preprocess (crop/rotate)
//! -> thin-line guard (from the ORIGINAL image) -> AI clean (Real-ESRGAN sidecar, median fallback)
//! -> palette lock (or Shading 12-32 Lab bands) -> layer tracing (visioncortex, the VTracer engine)
//! -> thin lines stacked on top -> tidy + export (SVG, PDF, EPS, DXF, ASE; RGB or CMYK).
pub mod batch;
pub mod brush;
pub mod centerline;
pub mod clean;
pub mod color;
pub mod dxf;
pub mod export;
pub mod fit;
pub mod heatmap;
pub mod license;
pub mod lines;
pub mod load;
pub mod palette;
pub mod pipeline;
pub mod preprocess;
pub mod primitives;
pub mod production;
pub mod project;
pub mod stats;
pub mod symmetry;
pub mod text_detect;
pub mod trace;
pub mod watch;

pub use brush::{BrushKind, BrushStroke};
pub use color::{rgb_to_cmyk, ColorSpace};
pub use dxf::{to_dxf, DxfUnit};
pub use export::to_pdf;
pub use heatmap::NodeHeatmap;
pub use license::{LicenseError, LicenseInfo};
pub use pipeline::{prepare, vectorize, vectorize_prepared, Output, Params, PreparedImage, Progress};
pub use preprocess::{apply_preprocess, apply_preprocess_rgb, PreprocessOptions};
pub use production::{ProductionMode, ProductionOptions};
pub use project::{LoadedProject, ProjectFile};
pub use symmetry::SymmetryMode;
pub use text_detect::TextRegion;
pub use watch::{watch_folder, WatchConfig, WatchEvent};

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