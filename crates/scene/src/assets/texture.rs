//! Texture loading from PNG/JPEG, decoded to RGBA8 via the `image` crate.

use std::path::Path;

use crate::SceneError;

/// CPU-side texture data (tightly packed RGBA8, row-major, top-left origin),
/// ready to upload to the GPU as `Rgba8UnormSrgb`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes: R, G, B, A per pixel.
    pub pixels: Vec<u8>,
}

/// Load and decode a texture, picking the decoder from the file extension.
///
/// PNG and JPEG are decoded to RGBA8. Other formats (including KTX/KTX2, which
/// the `image` crate doesn't handle) return [`SceneError::UnsupportedFormat`].
pub fn load_texture(path: &Path) -> Result<TextureData, SceneError> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("png") | Some("jpg") | Some("jpeg") => decode(path),
        other => Err(SceneError::UnsupportedFormat(
            other.unwrap_or("<none>").to_string(),
        )),
    }
}

fn decode(path: &Path) -> Result<TextureData, SceneError> {
    let image = image::open(path)
        .map_err(|e| SceneError::Parse(format!("decode '{}': {e}", path.display())))?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(TextureData {
        width,
        height,
        pixels: rgba.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal 2x2 PNG, encoded in-memory, then decoded back.
    #[test]
    fn decodes_png_to_rgba8() {
        use image::ImageEncoder;

        // Two rows of two RGBA pixels: red, green / blue, white.
        let raw: [u8; 16] = [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&raw, 2, 2, image::ExtendedColorType::Rgba8)
            .expect("encode png");
        let path = std::env::temp_dir().join("elderforge_tex.png");
        std::fs::write(&path, &png).expect("write png");

        let tex = load_texture(&path).expect("load png");
        assert_eq!((tex.width, tex.height), (2, 2));
        assert_eq!(tex.pixels.len(), 2 * 2 * 4);
        // First pixel is opaque red.
        assert_eq!(&tex.pixels[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn unsupported_extension_errors() {
        let path = std::env::temp_dir().join("elderforge_tex.ktx2");
        std::fs::write(&path, b"not really ktx").expect("write file");
        assert!(matches!(
            load_texture(&path),
            Err(SceneError::UnsupportedFormat(_))
        ));
    }
}
