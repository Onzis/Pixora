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
pub fn recognize_text_from_image(
    image: &RgbaImage,
    preferred_lang: Option<&str>,
) -> Result<OcrOutput, String> {
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

    // Query all OCR languages installed on this Windows PC
    let available = OcrEngine::AvailableRecognizerLanguages()
        .map_err(|e| format!("Failed to query AvailableRecognizerLanguages: {}", e))?;
    let count = available.Size().unwrap_or(0);
    let mut ru_lang = None;
    let mut en_lang = None;
    let mut other_lang = None;

    for i in 0..count {
        if let Ok(lang) = available.GetAt(i) {
            if let Ok(tag) = lang.LanguageTag() {
                let tag_str = tag.to_string();
                if tag_str.to_lowercase().starts_with("ru") && ru_lang.is_none() {
                    ru_lang = Some(lang.clone());
                } else if tag_str.to_lowercase().starts_with("en") && en_lang.is_none() {
                    en_lang = Some(lang.clone());
                } else if other_lang.is_none() {
                    other_lang = Some(lang.clone());
                }
            }
        }
    }

    let pref = preferred_lang.unwrap_or("auto").to_lowercase();

    // Mode 1: Explicit English preference
    if pref == "en" || pref == "english" {
        let engine = if let Some(en) = &en_lang {
            OcrEngine::TryCreateFromLanguage(en)
                .map_err(|e| format!("Failed to create English OcrEngine: {}", e))?
        } else {
            OcrEngine::TryCreateFromUserProfileLanguages()
                .map_err(|e| format!("Failed to create OcrEngine: {}", e))?
        };
        let res = engine
            .RecognizeAsync(&software_bitmap)
            .map_err(|e| format!("OCR RecognizeAsync failed: {}", e))?
            .get()
            .map_err(|e| format!("OCR execution failed: {}", e))?;
        return parse_single_ocr_result(&res);
    }

    // Mode 2: Russian available (default in "auto" when Russian pack is installed, or explicit "ru")
    if let Some(ru) = &ru_lang {
        let ru_engine = OcrEngine::TryCreateFromLanguage(ru)
            .map_err(|e| format!("Failed to create Russian OcrEngine: {}", e))?;
        let ru_res = ru_engine
            .RecognizeAsync(&software_bitmap)
            .map_err(|e| format!("Russian OCR RecognizeAsync failed: {}", e))?
            .get()
            .map_err(|e| format!("Russian OCR execution failed: {}", e))?;

        // If English engine is also installed, run dual-engine hybrid refinement:
        // Russian engine provides authentic Cyrillic words, English engine refines Latin tech terms (GPU, Rust, etc.)
        if let Some(en) = &en_lang {
            if let Ok(en_engine) = OcrEngine::TryCreateFromLanguage(en) {
                if let Ok(async_op) = en_engine.RecognizeAsync(&software_bitmap) {
                    if let Ok(en_res) = async_op.get() {
                        return Ok(merge_dual_engine_ocr(&ru_res, &en_res));
                    }
                }
            }
        }

        return parse_single_ocr_result(&ru_res);
    }

    // Mode 3: Fallback if Russian OCR is not installed on this PC
    let engine = if let Some(en) = &en_lang {
        OcrEngine::TryCreateFromLanguage(en)
            .map_err(|e| format!("Failed to create OcrEngine: {}", e))?
    } else if let Some(other) = &other_lang {
        OcrEngine::TryCreateFromLanguage(other)
            .map_err(|e| format!("Failed to create OcrEngine: {}", e))?
    } else {
        OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|e| format!("Failed to create OcrEngine from profile: {}", e))?
    };

    let res = engine
        .RecognizeAsync(&software_bitmap)
        .map_err(|e| format!("OCR RecognizeAsync failed: {}", e))?
        .get()
        .map_err(|e| format!("OCR execution failed: {}", e))?;
    parse_single_ocr_result(&res)
}

#[cfg(windows)]
fn parse_single_ocr_result(res: &windows::Media::Ocr::OcrResult) -> Result<OcrOutput, String> {
    let mut lines = Vec::new();
    let mut line_texts = Vec::new();

    if let Ok(line_list) = res.Lines() {
        let count = line_list.Size().unwrap_or(0);
        for i in 0..count {
            if let Ok(line) = line_list.GetAt(i) {
                let text = line.Text().map(|s| s.to_string()).unwrap_or_default();
                if text.trim().is_empty() {
                    continue;
                }

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

                line_texts.push(text.clone());
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

    let full_text = line_texts.join("\n");
    Ok(OcrOutput { full_text, lines })
}

#[cfg(windows)]
fn merge_dual_engine_ocr(
    ru_res: &windows::Media::Ocr::OcrResult,
    en_res: &windows::Media::Ocr::OcrResult,
) -> OcrOutput {
    let is_cyrillic = |c: char| -> bool {
        matches!(c, '\u{0400}'..='\u{04FF}' | '\u{0500}'..='\u{052F}')
    };

    let cyrillic_count = ru_res
        .Text()
        .map(|t| t.to_string().chars().filter(|c| is_cyrillic(*c)).count())
        .unwrap_or(0);

    // If no Cyrillic text is detected, the screenshot is pure English/code/Latin!
    // Directly use the English engine output.
    if cyrillic_count == 0 {
        if let Ok(output) = parse_single_ocr_result(en_res) {
            return output;
        }
    }

    // Collect all words from English engine with their spatial bounding boxes
    struct EnWord {
        text: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    }

    let mut en_words = Vec::new();
    if let Ok(lines) = en_res.Lines() {
        for i in 0..lines.Size().unwrap_or(0) {
            if let Ok(l) = lines.GetAt(i) {
                if let Ok(words) = l.Words() {
                    for j in 0..words.Size().unwrap_or(0) {
                        if let Ok(w) = words.GetAt(j) {
                            if let Ok(r) = w.BoundingRect() {
                                let text = w.Text().map(|t| t.to_string()).unwrap_or_default();
                                en_words.push(EnWord {
                                    text,
                                    x: r.X,
                                    y: r.Y,
                                    w: r.Width,
                                    h: r.Height,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    let mut lines_out = Vec::new();
    let mut recognized_lines = Vec::new();

    if let Ok(ru_lines) = ru_res.Lines() {
        for i in 0..ru_lines.Size().unwrap_or(0) {
            if let Ok(line) = ru_lines.GetAt(i) {
                let mut tokens = Vec::new();
                let mut line_x = 0.0f32;
                let mut line_y = 0.0f32;
                let mut line_w = 0.0f32;
                let mut line_h = 0.0f32;
                let mut last_right = 0.0f32;

                if let Ok(words) = line.Words() {
                    let word_count = words.Size().unwrap_or(0);
                    for j in 0..word_count {
                        if let Ok(w) = words.GetAt(j) {
                            let mut text = w.Text().map(|t| t.to_string()).unwrap_or_default();
                            if let Ok(r) = w.BoundingRect() {
                                if j == 0 {
                                    line_x = r.X;
                                    line_y = r.Y;
                                    line_h = r.Height;
                                }
                                last_right = last_right.max(r.X + r.Width);
                                line_w = last_right - line_x;
                                if r.Height > line_h {
                                    line_h = r.Height;
                                }

                                let r_cx = r.X + r.Width / 2.0;
                                let r_cy = r.Y + r.Height / 2.0;

                                // Find closest matching EN word by center distance
                                let en_match = en_words
                                    .iter()
                                    .filter(|ew| {
                                        let ew_cy = ew.y + ew.h / 2.0;
                                        let ew_cx = ew.x + ew.w / 2.0;
                                        (ew_cy - r_cy).abs() < 14.0
                                            && (ew_cx - r_cx).abs() < (r.Width.max(ew.w) * 0.8 + 14.0)
                                    })
                                    .min_by(|a, b| {
                                        let da = ((a.x + a.w / 2.0) - r_cx).abs();
                                        let db = ((b.x + b.w / 2.0) - r_cx).abs();
                                        da.partial_cmp(&db).unwrap()
                                    });

                                if let Some(en_w) = en_match {
                                    // 1. Prefix acronyms: "бРИ-ускоренная" -> "GPU-ускоренная"
                                    if text.starts_with("бРИ-") && en_w.text.contains("GPU") {
                                        text = format!("GPU-{}", text.trim_start_matches("бРИ-"));
                                    }
                                    // 2. Cyrillic lookalike of GPU: "бРИ" -> "GPU"
                                    else if (text == "бРИ" || text == "бРИ-") && en_w.text.contains("GPU") {
                                        text = "GPU".to_string();
                                    }
                                    // 3. Junk characters in RU for Latin acronyms: e.g. "Епд!Ёђ" -> "GPUI"
                                    else if (text.contains('!') || text.contains('ђ') || text.contains('Ё') || text.contains('ј'))
                                        && en_w.text.chars().any(|c| c.is_ascii_alphabetic())
                                    {
                                        text = en_w.text.clone();
                                    }
                                    // 4. Pure Latin words in RU: prefer EN dictionary casing (e.g. CleanShot vs CIeanShot)
                                    else if !text.chars().any(is_cyrillic)
                                        && en_w.text.chars().all(|c| c.is_ascii())
                                    {
                                        text = en_w.text.clone();
                                    }
                                }
                            }
                            tokens.push(text);
                        }
                    }

                    // Check if EN engine had trailing words on this line that RU dropped (e.g. "GPUI.")
                    for ew in &en_words {
                        let ew_cy = ew.y + ew.h / 2.0;
                        let line_cy = line_y + line_h / 2.0;
                        if (ew_cy - line_cy).abs() < 12.0
                            && ew.x >= last_right - 5.0
                            && ew.x < last_right + 120.0
                        {
                            if ew.text.chars().all(|c| c.is_ascii()) && !tokens.contains(&ew.text) {
                                tokens.push(ew.text.clone());
                                last_right = ew.x + ew.w;
                                line_w = last_right - line_x;
                            }
                        }
                    }
                }

                if !tokens.is_empty() {
                    let line_str = tokens.join(" ");
                    lines_out.push(line_str.clone());
                    recognized_lines.push(OcrRecognizedLine {
                        text: line_str,
                        x: line_x,
                        y: line_y,
                        width: line_w,
                        height: line_h,
                    });
                }
            }
        }
    }

    let full_text = lines_out.join("\n");
    OcrOutput {
        full_text,
        lines: recognized_lines,
    }
}

#[cfg(not(windows))]
pub fn recognize_text_from_image(
    _image: &RgbaImage,
    _preferred_lang: Option<&str>,
) -> Result<OcrOutput, String> {
    Err("OCR only supported on Windows".to_string())
}
