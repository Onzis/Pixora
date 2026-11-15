use image::{DynamicImage, RgbaImage};
use rqrr::PreparedImage;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrScanResult {
    pub content: String,
}

pub fn decode_qr_from_image(image: &RgbaImage) -> Result<QrScanResult, String> {
    if image.width() == 0 || image.height() == 0 {
        return Err("Cannot scan empty image".to_string());
    }

    let dyn_img = DynamicImage::ImageRgba8(image.clone());
    let gray_img = dyn_img.to_luma8();
    let mut prepared = PreparedImage::prepare(gray_img);
    let grids = prepared.detect_grids();

    if grids.is_empty() {
        return Err("No QR code detected in selected area".to_string());
    }

    let mut decoded_contents = Vec::new();
    for grid in grids {
        match grid.decode() {
            Ok((_meta, content)) => {
                if !content.trim().is_empty() {
                    decoded_contents.push(content);
                }
            }
            Err(e) => {
                eprintln!("QR decode grid error: {:?}", e);
            }
        }
    }

    if decoded_contents.is_empty() {
        return Err("Failed to decode QR code data".to_string());
    }

    Ok(QrScanResult {
        content: decoded_contents.join("\n"),
    })
}
