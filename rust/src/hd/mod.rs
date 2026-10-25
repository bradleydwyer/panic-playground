//! Experimental native-resolution intro. The reference timeline is retained.
//! Modern mode uses procedural 3D geometry and HDR lighting; faithful mode
//! interpolates the original raster contours. Neither recovers original models.
pub mod clip;
pub mod dots;
mod dots_render;
pub mod field;
mod field_render;
mod modern;
pub mod plasma;
mod plasma_render;
pub mod plasma_stone;
pub mod renderer;
pub mod rings;
mod rings_render;
