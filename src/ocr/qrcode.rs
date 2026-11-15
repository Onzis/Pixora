use image::{DynamicImage, Rgba, RgbaImage};
use rqrr::PreparedImage;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrScanResult {
    pub content: String,
    pub format: Option<String>,
}

fn add_white_padding(image: &RgbaImage, padding: u32) -> RgbaImage {
    let w = image.width() + padding * 2;
    let h = image.height() + padding * 2;
    let mut padded = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));
    image::imageops::overlay(&mut padded, image, padding as i64, padding as i64);
    padded
}

fn upscale_2x(image: &RgbaImage) -> RgbaImage {
    let w = (image.width() * 2).max(1);
    let h = (image.height() * 2).max(1);
    image::imageops::resize(image, w, h, image::imageops::FilterType::Nearest)
}

fn invert_image(image: &RgbaImage) -> RgbaImage {
    let mut inverted = image.clone();
    for pixel in inverted.pixels_mut() {
        pixel[0] = 255 - pixel[0];
        pixel[1] = 255 - pixel[1];
        pixel[2] = 255 - pixel[2];
    }
    inverted
}

fn try_rxing(image: &RgbaImage) -> Option<QrScanResult> {
    let dyn_img = DynamicImage::ImageRgba8(image.clone());
    let mut hints = rxing::DecodeHints::default();
    hints.TryHarder = Some(true);
    hints.AlsoInverted = Some(true);

    if let Ok(result) = rxing::helpers::detect_in_image_with_hints(dyn_img, None, &mut hints) {
        let text = result.getText().trim();
        if !text.is_empty() {
            return Some(QrScanResult {
                content: text.to_string(),
                format: Some(format!("{:?}", result.getBarcodeFormat())),
            });
        }
    }
    None
}

fn try_rxing_filtered(image: &RgbaImage) -> Option<QrScanResult> {
    let dyn_img = DynamicImage::ImageRgba8(image.clone());
    let mut hints = rxing::DecodeHints::default();
    hints.TryHarder = Some(true);
    hints.AlsoInverted = Some(true);

    if let Ok(result) = rxing::helpers::detect_in_image_filtered_with_hints(dyn_img, None, &mut hints) {
        let text = result.getText().trim();
        if !text.is_empty() {
            return Some(QrScanResult {
                content: text.to_string(),
                format: Some(format!("{:?}", result.getBarcodeFormat())),
            });
        }
    }
    None
}

fn try_rqrr(image: &RgbaImage) -> Option<QrScanResult> {
    let dyn_img = DynamicImage::ImageRgba8(image.clone());
    let gray_img = dyn_img.to_luma8();
    let mut prepared = PreparedImage::prepare(gray_img);
    let grids = prepared.detect_grids();

    for grid in grids {
        if let Ok((_meta, content)) = grid.decode() {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return Some(QrScanResult {
                    content: trimmed.to_string(),
                    format: Some("QR_CODE".to_string()),
                });
            }
        }
    }
    None
}

pub fn decode_qr_from_image(image: &RgbaImage) -> Result<QrScanResult, String> {
    if image.width() == 0 || image.height() == 0 {
        return Err("Cannot scan empty image".to_string());
    }

    // Pass 1: Direct rxing with TryHarder and AlsoInverted (supports normal & dark mode inverted QR/barcodes)
    if let Some(res) = try_rxing(image) {
        return Ok(res);
    }

    // Pass 2: Quiet zone padding (24 white pixels border).
    // Tightly cropped QR codes fail finder pattern matching without a quiet zone!
    let padded = add_white_padding(image, 24);
    if let Some(res) = try_rxing(&padded) {
        return Ok(res);
    }

    // Pass 3: Small/low-res code upscaling
    if image.width() < 400 || image.height() < 400 {
        let upscaled = upscale_2x(image);
        if let Some(res) = try_rxing(&upscaled) {
            return Ok(res);
        }
        let upscaled_padded = add_white_padding(&upscaled, 32);
        if let Some(res) = try_rxing(&upscaled_padded) {
            return Ok(res);
        }
    }

    // Pass 4: rxing filtered reader with adaptive multi-thresholding
    if let Some(res) = try_rxing_filtered(image) {
        return Ok(res);
    }
    if let Some(res) = try_rxing_filtered(&padded) {
        return Ok(res);
    }

    // Pass 5: rqrr fallback (original, white-padded, inverted, inverted-padded)
    if let Some(res) = try_rqrr(image) {
        return Ok(res);
    }
    if let Some(res) = try_rqrr(&padded) {
        return Ok(res);
    }
    let inverted = invert_image(image);
    if let Some(res) = try_rqrr(&inverted) {
        return Ok(res);
    }
    let inverted_padded = add_white_padding(&inverted, 24);
    if let Some(res) = try_rqrr(&inverted_padded) {
        return Ok(res);
    }

    Err("No QR code detected in selected area".to_string())
}
