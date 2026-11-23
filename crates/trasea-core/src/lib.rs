//! Trasea core: raster illustration / logo -> clean SVG / PDF / EPS / DXF. No UI, no network.
//!
//! Pipeline (see `pipeline::vectorize`): load -> optional non-destructive preprocess (rotate/crop/remove-bg)
//! -> thin-line guard (from the ORIGINAL image) -> AI clean (Real-ESRGAN sidecar, median fallback)
//! -> palette lock (Shading 12-32 Lab bands, or pure Centerline strokes) -> layer tracing (visioncortex)
//! -> thin lines stacked on top -> non-destructive path edits -> optional Die-Cut Contour
//! -> tidy + export (SVG, PDF w/ Named Spot Colors, EPS w/ CustomColors, DXF R12, ASE Spot; RGB or CMYK).
pub mod batch;
pub mod brush;
pub mod centerline;
pub mod clean;
pub mod color;
pub mod contour;
pub mod dxf;
pub mod export;
pub mod fit;
pub mod heatmap;
pub mod license;
pub mod lines;
pub mod load;
pub mod node_edit;
pub mod palette;
pub mod path_edit;
pub mod pipeline;
pub mod preprocess;
pub mod primitives;
pub mod production;
pub mod project;
pub mod stats;
pub mod svg_import;
pub mod symmetry;
pub mod text_detect;
pub mod trace;
pub mod watch;

pub use brush::{BrushKind, BrushStroke};
pub use color::{
    is_out_of_cmyk_gamut, rgb_to_cmyk, rgb_to_soft_proof_rgb, sanitize_spot_name, ColorSpace,
    SpotColorMapping,
};
pub use contour::{build_die_cut_contours, die_cut_bounds_svg};
pub use dxf::{to_dxf, DxfUnit};
pub use export::{to_ase_with_spots, to_pdf};
pub use heatmap::NodeHeatmap;
pub use license::{LicenseError, LicenseInfo};
pub use node_edit::{apply_node_edits, inspect_path_nodes, HandleSide, NodeEdit, PathNodeGeometry};
pub use path_edit::{apply_path_edits, PathEdit};
pub use pipeline::{prepare, vectorize, vectorize_prepared, vectorize_svg, Output, Params, PreparedImage, Progress};
pub use preprocess::{
    apply_preprocess, apply_preprocess_ex, apply_preprocess_rgb, apply_preprocess_rgb_with_mask,
    remove_background_rgba, PreprocessOptions, RemoveBgReport,
};
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
