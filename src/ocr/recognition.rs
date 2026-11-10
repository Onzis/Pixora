use image::RgbaImage;

#[derive(Clone, Debug)]
pub struct OcrRecognizedLine {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug)]
pub struct OcrOutput {
    pub full_text: String,
    pub lines: Vec<OcrRecognizedLine>,
}

#[cfg(windows)]
pub fn recognize_text_from_image(image: &RgbaImage) -> Result<OcrOutput, String> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;

    let width = image.width();
    let height = image.height();

    if width == 0 || height == 0 {
        return Err("Cannot run OCR on empty image".to_string());
    }

    let writer = DataWriter::new().map_err(|e| format!("Failed to create DataWriter: {}", e))?;
    writer
        .WriteBytes(image.as_raw())
        .map_err(|e| format!("Failed to write bytes to DataWriter: {}", e))?;
    let buffer = writer
        .DetachBuffer()
        .map_err(|e| format!("Failed to detach buffer: {}", e))?;

    let software_bitmap = SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        BitmapPixelFormat::Rgba8,
        width as i32,
        height as i32,
    )
    .map_err(|e| format!("Failed to create SoftwareBitmap: {}", e))?;

    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|e| format!("Failed to create OcrEngine: {}", e))?;

    let async_op = engine
        .RecognizeAsync(&software_bitmap)
        .map_err(|e| format!("OCR RecognizeAsync failed: {}", e))?;

    let ocr_result = async_op
        .get()
        .map_err(|e| format!("OCR execution failed: {}", e))?;

    let full_text = ocr_result
        .Text()
        .map(|s| s.to_string())
        .unwrap_or_default();

    let mut lines = Vec::new();
    if let Ok(line_list) = ocr_result.Lines() {
        let count = line_list.Size().unwrap_or(0);
        for i in 0..count {
            if let Ok(line) = line_list.GetAt(i) {
                let text = line.Text().map(|s| s.to_string()).unwrap_or_default();
                // Extract bounding rect from first word if available
                let mut x = 0.0;
                let mut y = 0.0;
                let mut w = 0.0;
                let mut h = 0.0;

                if let Ok(words) = line.Words() {
                    let word_count = words.Size().unwrap_or(0);
                    if word_count > 0 {
                        if let Ok(first_word) = words.GetAt(0) {
                            if let Ok(rect) = first_word.BoundingRect() {
                                x = rect.X;
                                y = rect.Y;
                                h = rect.Height;
                            }
                        }
                        if let Ok(last_word) = words.GetAt(word_count - 1) {
                            if let Ok(rect) = last_word.BoundingRect() {
                                w = (rect.X + rect.Width) - x;
                                if rect.Height > h {
                                    h = rect.Height;
                                }
                            }
                        }
                    }
                }

                lines.push(OcrRecognizedLine {
                    text,
                    x,
                    y,
                    width: w,
                    height: h,
                });
            }
        }
    }

    Ok(OcrOutput { full_text, lines })
}

#[cfg(not(windows))]
pub fn recognize_text_from_image(_image: &RgbaImage) -> Result<OcrOutput, String> {
    Err("OCR only supported on Windows".to_string())
}
