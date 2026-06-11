//! Texture loading from PNG/JPEG/KTX.

use std::path::Path;

use crate::SceneError;

/// CPU-side texture data (RGBA8), ready to upload to the GPU.
#[derive(Debug, Clone, Default)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Load a texture, picking the decoder from the file extension.
pub fn load_texture(path: &Path) -> Result<TextureData, SceneError> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("png") | Some("jpg") | Some("jpeg") | Some("ktx") | Some("ktx2") => {
            let _bytes = std::fs::read(path)?;
            // TODO: decode the image data into RGBA8.
            Ok(TextureData::default())
        }
        other => Err(SceneError::UnsupportedFormat(
            other.unwrap_or("<none>").to_string(),
        )),
    }
}
