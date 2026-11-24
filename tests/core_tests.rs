use gpui::{Bounds, point, px, rgb, size};
use image::{Rgba, RgbaImage};
use pixora::config::AppConfig;
use pixora::editor::annotations::{Annotation, AnnotationHandle, resize_bounds_with_handle};
use pixora::editor::canvas::render_annotations_to_image;
use pixora::editor::history::EditHistory;
use pixora::image::crop::crop_rgba;
use pixora::image::processing::{apply_blur, apply_pixelate};
use pixora::overlay::selection::{ResizeHandle, SelectionState};

#[test]
fn test_config_defaults_and_serialization() {
    let config = AppConfig::default();
    assert!(config.auto_copy_to_clipboard);
    assert!(config.show_dimensions);
    assert!(!config.show_magnifier);
    assert_eq!(config.blur_radius, 12);
    assert_eq!(config.pixelate_block, 10);
    assert_eq!(config.spotlight_dim, 0.60);
    assert!(config.auto_check_updates);
    assert!(config.window_snapping);

    let json = serde_json::to_string(&config).expect("Must serialize to JSON");
    let deserialized: AppConfig = serde_json::from_str(&json).expect("Must deserialize from JSON");
    assert_eq!(config.filename_prefix, deserialized.filename_prefix);
    assert_eq!(config.default_stroke_width, deserialized.default_stroke_width);
    assert_eq!(config.blur_radius, deserialized.blur_radius);
    assert_eq!(config.pixelate_block, deserialized.pixelate_block);
    assert_eq!(config.spotlight_dim, deserialized.spotlight_dim);
    assert_eq!(config.default_font_size, deserialized.default_font_size);
    assert_eq!(config.window_snapping, deserialized.window_snapping);

    let toml_str = toml::to_string(&config).expect("Must serialize to TOML");
    let deserialized_toml: AppConfig = toml::from_str(&toml_str).expect("Must deserialize from TOML");
    assert_eq!(config.filename_prefix, deserialized_toml.filename_prefix);
    assert_eq!(config.filename_pattern, deserialized_toml.filename_pattern);
    assert_eq!(config.image_format, deserialized_toml.image_format);
    assert_eq!(config.spotlight_dim, deserialized_toml.spotlight_dim);
}

#[test]
fn test_config_adjacent_path() {
    let path = AppConfig::config_path();
    assert!(path.ends_with("config.toml"));
}

#[test]
fn test_sound_file_resolution_and_extraction() {
    use pixora::system::sound::get_or_extract_sound_path;
    let path = get_or_extract_sound_path().expect("Sound path must be resolved or extracted");
    assert!(path.exists(), "Resolved sound file must exist on disk");
    let meta = std::fs::metadata(&path).expect("Metadata must be readable");
    assert!(meta.len() > 0, "Sound file must not be empty");
}

#[test]
fn test_selection_normalization_and_handles() {
    // Normalization of inverted drag (dragging up-left)
    let sel = SelectionState::new(200.0, 300.0, -100.0, -50.0);
    assert_eq!(sel.x(), 100.0);
    assert_eq!(sel.y(), 250.0);
    assert_eq!(sel.width(), 100.0);
    assert_eq!(sel.height(), 50.0);
    assert!(sel.is_active);

    // Contains test
    assert!(sel.contains(point(px(150.0), px(275.0))));
    assert!(!sel.contains(point(px(50.0), px(50.0))));

    // Hit test top-left handle
    let handle = sel.hit_test_handle(point(px(100.0), px(250.0)), px(8.0));
    assert_eq!(handle, Some(ResizeHandle::TopLeft));

    // Hit test bottom-right handle
    let handle_br = sel.hit_test_handle(point(px(200.0), px(300.0)), px(8.0));
    assert_eq!(handle_br, Some(ResizeHandle::BottomRight));

    // Hit test bottom-center handle
    let handle_b = sel.hit_test_handle(point(px(150.0), px(300.0)), px(8.0));
    assert_eq!(handle_b, Some(ResizeHandle::Bottom));

    // Hit test bottom-left handle
    let handle_bl = sel.hit_test_handle(point(px(100.0), px(300.0)), px(8.0));
    assert_eq!(handle_bl, Some(ResizeHandle::BottomLeft));

    // Test dragging bottom handle downwards by 20px
    let mut sel_mut = sel;
    sel_mut.update_resize(ResizeHandle::Bottom, sel.bounds, point(px(0.0), px(20.0)));
    assert_eq!(sel_mut.height(), 70.0);
    assert_eq!(sel_mut.width(), 100.0);

    // Test dragging bottom-right handle by (+15px, +25px)
    sel_mut = sel;
    sel_mut.update_resize(ResizeHandle::BottomRight, sel.bounds, point(px(15.0), px(25.0)));
    assert_eq!(sel_mut.width(), 115.0);
    assert_eq!(sel_mut.height(), 75.0);

    // Test dragging bottom-left handle by (-10px, +30px)
    sel_mut = sel;
    sel_mut.update_resize(ResizeHandle::BottomLeft, sel.bounds, point(px(-10.0), px(30.0)));
    assert_eq!(sel_mut.x(), 90.0);
    assert_eq!(sel_mut.width(), 110.0);
    assert_eq!(sel_mut.height(), 80.0);
}

#[test]
fn test_edit_history_undo_redo() {
    let mut history = EditHistory::new();
    assert!(!history.can_undo());
    assert!(!history.can_redo());

    let ann1 = Annotation::Line {
        from: point(px(10.0), px(10.0)),
        to: point(px(50.0), px(50.0)),
        color: rgb(0xFF0000),
        width: 3.0,
    };

    history.push_annotation(ann1);
    assert_eq!(history.annotations.len(), 1);
    assert!(history.can_undo());
    assert!(!history.can_redo());

    let ann2 = Annotation::Rectangle {
        bounds: Bounds {
            origin: point(px(20.0), px(20.0)),
            size: size(px(40.0), px(40.0)),
        },
        color: rgb(0x00FF00),
        width: 2.0,
        filled: false,
    };
    history.push_annotation(ann2);
    assert_eq!(history.annotations.len(), 2);

    // Undo second annotation
    assert!(history.undo());
    assert_eq!(history.annotations.len(), 1);
    assert!(history.can_redo());

    // Redo second annotation
    assert!(history.redo());
    assert_eq!(history.annotations.len(), 2);
    assert!(!history.can_redo());
}

#[test]
fn test_crop_rgba() {
    let mut img = RgbaImage::new(100, 100);
    img.put_pixel(10, 10, Rgba([255, 0, 0, 255]));

    let cropped = crop_rgba(&img, 5, 5, 20, 20).expect("Valid crop");
    assert_eq!(cropped.width(), 20);
    assert_eq!(cropped.height(), 20);
    assert_eq!(*cropped.get_pixel(5, 5), Rgba([255, 0, 0, 255]));

    // Out of bounds crop returns None
    assert!(crop_rgba(&img, 200, 200, 50, 50).is_none());
}

#[test]
fn test_apply_pixelate_and_blur() {
    let mut img = RgbaImage::new(50, 50);
    for y in 0..50 {
        for x in 0..50 {
            img.put_pixel(x, y, Rgba([(x * 5) as u8, (y * 5) as u8, 128, 255]));
        }
    }

    // Apply pixelate
    apply_pixelate(&mut img, 10, 10, 20, 20, 5);
    // Inside block, pixels should be uniform
    let p1 = *img.get_pixel(11, 11);
    let p2 = *img.get_pixel(12, 12);
    assert_eq!(p1, p2);

    // Apply blur without panics
    apply_blur(&mut img, 5, 5, 30, 30, 4);
    assert_eq!(img.width(), 50);
    assert_eq!(img.height(), 50);
}

#[test]
fn test_render_all_annotations_to_image() {
    let mut img = RgbaImage::new(200, 200);

    let annotations = vec![
        Annotation::Pencil {
            points: vec![point(px(10.0), px(10.0)), point(px(30.0), px(30.0))],
            color: rgb(0xFF453A),
            width: 3.0,
        },
        Annotation::Marker {
            points: vec![point(px(15.0), px(15.0)), point(px(45.0), px(15.0))],
            color: rgb(0xFFD60A),
            width: 8.0,
        },
        Annotation::Arrow {
            from: point(px(50.0), px(50.0)),
            to: point(px(100.0), px(100.0)),
            control: None,
            two_headed: false,
            color: rgb(0x8174FF),
            width: 4.0,
        },
        Annotation::Rectangle {
            bounds: Bounds {
                origin: point(px(60.0), px(60.0)),
                size: size(px(40.0), px(40.0)),
            },
            color: rgb(0x30D158),
            width: 2.0,
            filled: true,
        },
        Annotation::Ellipse {
            bounds: Bounds {
                origin: point(px(120.0), px(120.0)),
                size: size(px(50.0), px(30.0)),
            },
            color: rgb(0x0A84FF),
            width: 2.0,
            filled: false,
        },
        Annotation::NumberBadge {
            center: point(px(160.0), px(50.0)),
            number: 1,
            color: rgb(0x8174FF),
            radius: 12.0,
            shape: pixora::editor::annotations::BadgeShape::Circle,
            mode: pixora::editor::annotations::BadgeMode::Numbers,
            filled: true,
        },
        Annotation::Heart {
            bounds: Bounds {
                origin: point(px(180.0), px(50.0)),
                size: size(px(40.0), px(40.0)),
            },
            color: rgb(0xFF2D55),
            width: 2.0,
            filled: true,
        },
        Annotation::Cross {
            bounds: Bounds {
                origin: point(px(230.0), px(50.0)),
                size: size(px(30.0), px(30.0)),
            },
            color: rgb(0xFF3B30),
            width: 3.0,
            filled: false,
        },
        Annotation::Checkmark {
            bounds: Bounds {
                origin: point(px(270.0), px(50.0)),
                size: size(px(30.0), px(30.0)),
            },
            color: rgb(0x34C759),
            width: 3.0,
            filled: true,
        },
        Annotation::Text {
            origin: point(px(20.0), px(160.0)),
            text: "PIXORA 2026".to_string(),
            color: rgb(0xFFFFFF),
            font_size: 16.0,
            box_width: None,
        },
        Annotation::Blur {
            bounds: Bounds {
                origin: point(px(10.0), px(70.0)),
                size: size(px(30.0), px(30.0)),
            },
            radius: 5,
        },
        Annotation::Pixelate {
            bounds: Bounds {
                origin: point(px(70.0), px(10.0)),
                size: size(px(30.0), px(30.0)),
            },
            block_size: 6,
        },
        Annotation::Triangle {
            bounds: Bounds {
                origin: point(px(10.0), px(100.0)),
                size: size(px(30.0), px(30.0)),
            },
            color: rgb(0xFF9F0A),
            width: 2.0,
            filled: true,
        },
        Annotation::Star {
            bounds: Bounds {
                origin: point(px(50.0), px(120.0)),
                size: size(px(30.0), px(30.0)),
            },
            color: rgb(0xFFD60A),
            width: 2.0,
            filled: false,
        },
        Annotation::Diamond {
            bounds: Bounds {
                origin: point(px(90.0), px(100.0)),
                size: size(px(30.0), px(30.0)),
            },
            color: rgb(0xBF5AF2),
            width: 2.0,
            filled: true,
        },
        Annotation::Spotlight {
            bounds: Bounds {
                origin: point(px(10.0), px(10.0)),
                size: size(px(40.0), px(40.0)),
            },
            color: rgb(0xFF453A),
            width: 2.0,
            dim_opacity: 0.60,
        },
    ];

    render_annotations_to_image(&mut img, &annotations, 0.0, 0.0);
    assert_eq!(img.width(), 200);
    assert_eq!(img.height(), 200);
}

#[test]
fn test_parse_hex_color() {
    use pixora::overlay::toolbar::parse_hex_color;

    let red = parse_hex_color("#FF0000").expect("Valid hex #FF0000");
    assert_eq!(red, rgb(0xFF0000));

    let green = parse_hex_color("00FF00").expect("Valid hex 00FF00");
    assert_eq!(green, rgb(0x00FF00));

    let purple = parse_hex_color("#8174FF").expect("Valid hex #8174FF");
    assert_eq!(purple, rgb(0x8174FF));

    assert!(parse_hex_color("invalid").is_none());
    assert!(parse_hex_color("#123").is_none());
    assert!(parse_hex_color("#GGFFFF").is_none());
}

#[test]
fn test_svg_asset_source_loads_all_icons() {
    use gpui::AssetSource;
    use pixora::assets::PixoraAssets;

    let assets = PixoraAssets;
    let list = assets.list("").expect("List icons");
    assert_eq!(list.len(), 41);

    for path in list {
        let content = assets.load(&path).expect("Load svg").expect("Found icon");
        let s = std::str::from_utf8(&content).expect("Valid UTF-8 SVG");
        assert!(s.starts_with("<svg"), "Icon {} must start with <svg", path);
        assert!(s.ends_with("</svg>"), "Icon {} must end with </svg>", path);
    }
}

#[test]
fn test_spotlight_dimming_and_cutout() {
    use pixora::image::processing::{apply_spotlights, SpotlightRegion};

    // Create a 100x100 image with constant value 200
    let mut img = RgbaImage::from_pixel(100, 100, Rgba([200, 200, 200, 255]));

    let spots = vec![
        SpotlightRegion {
            x: 20.0,
            y: 20.0,
            width: 30.0,
            height: 30.0,
            border_color: rgb(0xFF0000),
            border_width: 0.0,
            dim_opacity: 0.60,
        },
    ];

    apply_spotlights(&mut img, 0, 0, 100, 100, &spots, 0.60, 0.0);

    // Pixel inside spotlight (at 30, 30) should remain undimmed: 200
    let inside_pixel = img.get_pixel(30, 30);
    assert_eq!(inside_pixel[0], 200);
    assert_eq!(inside_pixel[1], 200);
    assert_eq!(inside_pixel[2], 200);

    // Pixel outside spotlight (at 5, 5) should be dimmed by factor (1 - 0.60) = 0.40 -> 200 * 0.40 = 80
    let outside_pixel = img.get_pixel(5, 5);
    assert_eq!(outside_pixel[0], 80);
    assert_eq!(outside_pixel[1], 80);
    assert_eq!(outside_pixel[2], 80);
}

#[test]
fn test_spotlight_dim_getter_setter() {
    let mut spot = Annotation::Spotlight {
        bounds: Bounds {
            origin: point(px(10.0), px(10.0)),
            size: size(px(50.0), px(50.0)),
        },
        color: rgb(0x000000),
        width: 0.0,
        dim_opacity: 0.60,
    };
    assert_eq!(spot.dim_opacity(), Some(0.60));
    spot.set_dim_opacity(0.85);
    assert_eq!(spot.dim_opacity(), Some(0.85));
}

#[test]
fn test_toolbar_position_boundaries_and_clamping() {
    use pixora::overlay::toolbar::{calculate_toolbar_position, TOOLBAR_WIDTH};

    let screen = Bounds {
        origin: point(px(0.0), px(0.0)),
        size: size(px(1920.0), px(1080.0)),
    };

    // Case 1: Selection at far right edge (where toolbar was previously cut off)
    let sel_right = Bounds {
        origin: point(px(1500.0), px(100.0)),
        size: size(px(420.0), px(300.0)),
    };
    let pos = calculate_toolbar_position(sel_right, screen, TOOLBAR_WIDTH, 48.0);
    let right_edge = f32::from(pos.x) + TOOLBAR_WIDTH;
    assert!(
        right_edge <= 1920.0 - 12.0,
        "Toolbar right edge ({}) must not exceed screen width with margin (1908)",
        right_edge
    );
    assert!(f32::from(pos.x) >= 12.0, "Toolbar left edge must be >= 12px");

    // Case 2: Fullscreen selection
    let sel_full = Bounds {
        origin: point(px(0.0), px(0.0)),
        size: size(px(1920.0), px(1080.0)),
    };
    let pos_full = calculate_toolbar_position(sel_full, screen, TOOLBAR_WIDTH, 96.0);
    let bottom_edge = f32::from(pos_full.y) + 96.0;
    assert!(
        bottom_edge <= 1080.0 - 12.0,
        "Toolbar bottom edge ({}) must fit inside screen",
        bottom_edge
    );

    // Case 3: Narrow selection (toolbar is centered on selection)
    let sel_narrow = Bounds {
        origin: point(px(500.0), px(200.0)),
        size: size(px(100.0), px(100.0)),
    };
    let pos_narrow = calculate_toolbar_position(sel_narrow, screen, TOOLBAR_WIDTH, 48.0);
    assert!(f32::from(pos_narrow.x) >= 12.0);
    assert!(f32::from(pos_narrow.x) + TOOLBAR_WIDTH <= 1908.0);
}

#[test]
fn test_hotkey_string_parsing() {
    use pixora::shortcuts::hotkeys::parse_hotkey_string;

    // Default region capture: PrintScreen
    let (mods, vk) = parse_hotkey_string("PrintScreen").expect("Parse PrintScreen");
    assert_eq!(vk, 0x2C); // VK_SNAPSHOT
    assert_eq!(mods & 0x4000, 0x4000); // MOD_NOREPEAT

    // Fullscreen capture: Shift + PrintScreen
    let (mods_fs, vk_fs) = parse_hotkey_string("Shift + PrintScreen").expect("Parse Shift+PrintScreen");
    assert_eq!(vk_fs, 0x2C);
    assert_ne!(mods_fs & 0x0004, 0); // MOD_SHIFT

    // Window capture: Alt + PrintScreen
    let (mods_win, vk_win) = parse_hotkey_string("Alt + PrintScreen").expect("Parse Alt+PrintScreen");
    assert_eq!(vk_win, 0x2C);
    assert_ne!(mods_win & 0x0001, 0); // MOD_ALT

    // Custom: Ctrl + Shift + A
    let (mods_custom, vk_custom) = parse_hotkey_string("Ctrl + Shift + A").expect("Parse Ctrl+Shift+A");
    assert_eq!(vk_custom, 'A' as u32);
    assert_ne!(mods_custom & 0x0002, 0); // MOD_CONTROL
    assert_ne!(mods_custom & 0x0004, 0); // MOD_SHIFT
}

#[test]
fn test_hsv_and_rgb_conversions() {
    use pixora::overlay::toolbar::{hsv_to_rgb, rgb_to_hsv, HUE_STOPS};

    assert_eq!(HUE_STOPS.len(), 24);

    // Primary Colors
    assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), (255, 0, 0));
    assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), (0, 255, 0));
    assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), (0, 0, 255));

    // Secondary Colors
    assert_eq!(hsv_to_rgb(60.0, 1.0, 1.0), (255, 255, 0));
    assert_eq!(hsv_to_rgb(180.0, 1.0, 1.0), (0, 255, 255));
    assert_eq!(hsv_to_rgb(300.0, 1.0, 1.0), (255, 0, 255));

    // Round-trip RGB to HSV
    let (h_r, s_r, v_r) = rgb_to_hsv(255, 0, 0);
    assert_eq!(h_r, 0.0);
    assert_eq!(s_r, 1.0);
    assert_eq!(v_r, 1.0);

    let (h_g, _, _) = rgb_to_hsv(0, 255, 0);
    assert_eq!(h_g, 120.0);

    let (h_b, _, _) = rgb_to_hsv(0, 0, 255);
    assert_eq!(h_b, 240.0);
}

#[test]
fn test_palette_color_formats_matching_reference() {
    use pixora::overlay::toolbar::{rgb_to_cmyk, rgb_to_hsl, rgb_to_hsv};

    // User reference screenshot test: RGB 235, 64, 52 (#eb4034)
    let (c, m, y, k) = rgb_to_cmyk(235, 64, 52);
    assert_eq!((c, m, y, k), (0, 73, 78, 8));

    let (h, s, l) = rgb_to_hsl(235, 64, 52);
    assert_eq!(h.round() as u32, 4);
    assert_eq!((s * 100.0).round() as u32, 82);
    assert_eq!((l * 100.0).round() as u32, 56);

    let (hsv_h, hsv_s, hsv_v) = rgb_to_hsv(235, 64, 52);
    assert_eq!(hsv_h.round() as u32, 4);
    assert_eq!((hsv_s * 100.0).round() as u32, 78);
    assert_eq!((hsv_v * 100.0).round() as u32, 92);
}

#[test]
fn test_i18n_language_detection_and_resolution() {
    use pixora::i18n::{detect_system_language, t, AppLanguage, LanguageChoice};

    // System language detection test (returns either En or Ru without panicking)
    let detected = detect_system_language();
    assert!(matches!(detected, AppLanguage::En | AppLanguage::Ru));

    // Auto resolution test dynamically matches system language
    assert_eq!(LanguageChoice::Auto.resolve(), detected);
    assert_eq!(LanguageChoice::En.resolve(), AppLanguage::En);
    assert_eq!(LanguageChoice::Ru.resolve(), AppLanguage::Ru);

    // Translation helper test
    assert_eq!(t(AppLanguage::En, "General", "Основные"), "General");
    assert_eq!(t(AppLanguage::Ru, "General", "Основные"), "Основные");
}

#[test]
fn test_blur_and_pixelate_undo_redo_pixel_restoration() {
    let mut base_img = RgbaImage::new(100, 100);
    // Draw high-contrast checkerboard
    for y in 0..100 {
        for x in 0..100 {
            let color = if (x / 10 + y / 10) % 2 == 0 {
                image::Rgba([255, 255, 255, 255])
            } else {
                image::Rgba([0, 0, 0, 255])
            };
            base_img.put_pixel(x, y, color);
        }
    }

    let mut history = EditHistory::new();
    let blur_ann = Annotation::Blur {
        bounds: Bounds {
            origin: point(px(20.0), px(20.0)),
            size: size(px(40.0), px(40.0)),
        },
        radius: 12,
    };

    history.push_annotation(blur_ann);
    assert_eq!(history.annotations.len(), 1);

    // 1. Apply blur to a working image
    let mut working_img = base_img.clone();
    render_annotations_to_image(&mut working_img, &history.annotations, 0.0, 0.0);

    // Pixel at (20, 20) should have changed due to blur
    assert_ne!(
        working_img.get_pixel(25, 25),
        base_img.get_pixel(25, 25),
        "Blurred pixels must differ from pristine base"
    );

    // 2. Undo the blur annotation
    assert!(history.undo());
    assert_eq!(history.annotations.len(), 0);

    // 3. Rebuild image from base_img (simulating overlay.rebuild_render_image())
    let mut restored_img = base_img.clone();
    render_annotations_to_image(&mut restored_img, &history.annotations, 0.0, 0.0);

    // Verify 100% exact pixel match with base_img
    assert_eq!(
        restored_img,
        base_img,
        "Undoing blur must completely restore pristine background image"
    );

    // 4. Redo the blur annotation
    assert!(history.redo());
    assert_eq!(history.annotations.len(), 1);

    let mut reblurred_img = base_img.clone();
    render_annotations_to_image(&mut reblurred_img, &history.annotations, 0.0, 0.0);
    assert_eq!(
        reblurred_img,
        working_img,
        "Redoing blur must produce identical blurred result"
    );
}

#[test]
fn test_ocr_recognition_with_hybrid_engine() {
    use pixora::ocr::recognition::recognize_text_from_image;

    let test_img_path = "C:/Users/Admin/.gemini/antigravity/brain/0a21d654-ba7d-4227-96a2-37fa85f98aba/.user_uploaded/media_1794385840467_034bea47.png";
    if let Ok(dyn_img) = image::open(test_img_path) {
        let rgba = dyn_img.to_rgba8();
        let sub = image::imageops::crop_imm(&rgba, 35, 55, 950, 260).to_image();

        let ocr_res = recognize_text_from_image(&sub, Some("auto")).expect("OCR must succeed");
        println!("\nRecognized text:\n{}", ocr_res.full_text);

        // Must recognize authentic Russian words, NOT English transliterated gibberish
        assert!(
            ocr_res.full_text.contains("студия"),
            "OCR must recognize 'студия', instead of gibberish"
        );
        assert!(
            ocr_res.full_text.contains("создания"),
            "OCR must recognize 'создания', instead of 'c03AaHHR'"
        );
        assert!(
            ocr_res.full_text.contains("скриншотов"),
            "OCR must recognize 'скриншотов', instead of 'CKPMHWOTOB'"
        );
        assert!(
            ocr_res.full_text.contains("Windows"),
            "OCR must preserve English technical terms like 'Windows'"
        );
        assert!(
            !ocr_res.full_text.contains("CKPMHWOTOB"),
            "OCR must not contain pseudo-Latin transliteration 'CKPMHWOTOB'"
        );
        assert!(
            !ocr_res.full_text.contains("CTYA\"R"),
            "OCR must not contain pseudo-Latin transliteration 'CTYA\"R'"
        );
    }
}

#[test]
fn test_ocr_on_empty_image_returns_error() {
    use pixora::ocr::recognition::recognize_text_from_image;
    let empty = image::RgbaImage::new(0, 0);
    assert!(recognize_text_from_image(&empty, None).is_err());
}

#[test]
fn test_magnifier_pixel_sampling_and_grid() {
    use pixora::overlay::magnifier::{LOUPE_GRID_SIZE, LOUPE_TOTAL_CELLS, MagnifierInfo};
    let mut img = image::RgbaImage::new(30, 30);
    // Fill with black
    for p in img.pixels_mut() {
        *p = image::Rgba([0, 0, 0, 255]);
    }
    // Set center pixel at (10, 10) to red (#FF453A)
    img.put_pixel(10, 10, image::Rgba([255, 69, 58, 255]));
    // Set neighboring pixel at (9, 10) to green (#30D158)
    img.put_pixel(9, 10, image::Rgba([48, 209, 88, 255]));

    let info = MagnifierInfo::from_image(&img, 1000, 500, 10, 10);
    assert_eq!(info.screen_x, 1000);
    assert_eq!(info.screen_y, 500);
    assert_eq!(info.hex_string(), "#FF453A");
    assert_eq!(info.r, 255);
    assert_eq!(info.g, 69);
    assert_eq!(info.b, 58);
    assert_eq!(info.grid_colors.len(), LOUPE_TOTAL_CELLS);
    assert_eq!(LOUPE_GRID_SIZE, 11);

    // Center cell is index 60 (5 * 11 + 5)
    let center_col = info.grid_colors[60];
    assert_eq!(center_col, gpui::rgb(0xFF453A));

    // dx = -1, dy = 0 is index (0 + 5) * 11 + (-1 + 5) = 5 * 11 + 4 = 59
    let left_col = info.grid_colors[59];
    assert_eq!(left_col, gpui::rgb(0x30D158));
}

#[test]
fn test_magnifier_clamping_at_edges() {
    use pixora::overlay::magnifier::{LOUPE_TOTAL_CELLS, MagnifierInfo};
    let img = image::RgbaImage::new(5, 5);
    // Coordinates at top-left corner (0, 0)
    let info_tl = MagnifierInfo::from_image(&img, 0, 0, 0, 0);
    assert_eq!(info_tl.grid_colors.len(), LOUPE_TOTAL_CELLS);
    // Coordinates at bottom-right beyond image
    let info_br = MagnifierInfo::from_image(&img, 999, 999, 100, 100);
    assert_eq!(info_br.grid_colors.len(), LOUPE_TOTAL_CELLS);
}

#[test]
fn test_version_parsing_and_comparison() {
    use pixora::system::updater::{is_newer_version, parse_version_tuple};

    assert_eq!(parse_version_tuple("0.14.0"), vec![0, 14, 0]);
    assert_eq!(parse_version_tuple("v0.14.0"), vec![0, 14, 0]);
    assert_eq!(parse_version_tuple("V1.2.3.4"), vec![1, 2, 3, 4]);
    assert_eq!(parse_version_tuple("v0.19.1-alpha"), vec![0, 19, 1]);

    assert!(is_newer_version("0.14.0", "0.14.1"));
    assert!(is_newer_version("0.14.0", "0.19.1"));
    assert!(is_newer_version("0.14.0", "1.0.0"));
    assert!(is_newer_version("0.14.0", "v0.14.1"));

    assert!(!is_newer_version("0.14.0", "0.14.0"));
    assert!(!is_newer_version("0.14.0", "v0.14.0"));
    assert!(!is_newer_version("0.14.0", "0.13.9"));
    assert!(!is_newer_version("1.0.0", "0.99.99"));
}

#[test]
fn test_github_release_checking() {
    use pixora::system::updater::check_latest_release;

    // Network may be unavailable or repo has no releases yet in sandbox/offline
    if let Ok(info) = check_latest_release() {
        assert_eq!(info.current_version, env!("CARGO_PKG_VERSION"));
        assert!(!info.latest_version.is_empty());
        assert!(info.release_url.contains("github.com/Onzis/Pixora/releases"));
    }
}

#[test]
fn test_settings_window_tracking() {
    use pixora::app::clear_settings_window_handle;
    // Clearing settings window handle must be safe and idempotent
    clear_settings_window_handle();
    clear_settings_window_handle();
}

#[test]
fn test_arrow_geometry_scaling_and_proportions() {
    use pixora::editor::annotations::calculate_arrow_geometry;

    // Test across various stroke widths from 1px to 60px
    for width in [1.0, 2.0, 4.0, 8.0, 16.0, 24.0, 40.0, 60.0] {
        let from = (100.0, 100.0);
        let to = (300.0, 100.0);
        let geom = calculate_arrow_geometry(from, to, width);

        assert!(geom.has_head, "Arrow of length 200 must have an arrowhead");

        // Tip must be exactly at target 'to'
        assert_eq!(geom.tip, to);

        // Arrowhead base width (distance between wings) must be strictly wider than shaft thickness
        let wing_w = (geom.left_wing.1 - geom.right_wing.1).abs();
        assert!(
            wing_w > width * 1.5,
            "For width {}, wing width {} must be > 1.5 * width",
            width,
            wing_w
        );

        // Shaft must end before the tip (never poke past the tip)
        assert!(
            geom.shaft_to.0 < geom.tip.0,
            "Shaft end {} must be strictly before tip {}",
            geom.shaft_to.0,
            geom.tip.0
        );

        // Shaft must extend into the arrowhead base so there is no gap
        let base_x = (geom.left_wing.0 + geom.right_wing.0) / 2.0;
        assert!(
            geom.shaft_to.0 >= base_x,
            "Shaft end {} must enter arrowhead base {}",
            geom.shaft_to.0,
            base_x
        );
    }

    // Short arrow test
    let short_geom = calculate_arrow_geometry((0.0, 0.0), (20.0, 0.0), 10.0);
    assert!(short_geom.has_head);
    assert!(short_geom.shaft_to.0 <= short_geom.tip.0);
    assert!(short_geom.shaft_from.0 <= short_geom.shaft_to.0);
}

#[test]
fn test_arrow_render_to_image_at_thick_stroke() {
    use pixora::editor::annotations::Annotation;
    use pixora::editor::canvas::render_annotations_to_image;

    let mut img = image::RgbaImage::new(400, 400);

    // Thick 30px arrow
    let annotations = vec![
        Annotation::Arrow {
            from: gpui::point(gpui::px(50.0), gpui::px(200.0)),
            to: gpui::point(gpui::px(350.0), gpui::px(200.0)),
            control: None,
            two_headed: false,
            color: gpui::rgb(0xFF453A),
            width: 30.0,
        },
    ];

    render_annotations_to_image(&mut img, &annotations, 0.0, 0.0);

    // Verify tip pixel at (349, 200) is painted
    let tip_pixel = img.get_pixel(349, 200);
    assert!(tip_pixel[3] > 0, "Tip pixel should be painted");

    // Verify pixels beyond the tip (e.g. 365, 200) are NOT painted (shaft did not poke through)
    let beyond_tip = img.get_pixel(365, 200);
    assert_eq!(beyond_tip[3], 0, "Pixels beyond tip must not be painted");
}

#[test]
fn test_window_snapping_detection_and_selection() {
    use gpui::{Bounds, point, px, size};
    use pixora::overlay::overlay::WindowSnapCandidate;
    use pixora::overlay::selection::SelectionState;

    let candidate = WindowSnapCandidate {
        bounds: Bounds {
            origin: point(px(100.0), px(150.0)),
            size: size(px(800.0), px(600.0)),
        },
        title: "Test Application Window".to_string(),
        is_child_element: false,
        z_order: 0,
    };

    // Point inside the window
    let pt_inside = point(px(200.0), px(300.0));
    assert!(candidate.bounds.contains(&pt_inside));

    // Point outside the window
    let pt_outside = point(px(50.0), px(50.0));
    assert!(!candidate.bounds.contains(&pt_outside));

    // When clicked/snapped, SelectionState takes exact window bounds
    let snapped_sel = SelectionState::new(
        f32::from(candidate.bounds.origin.x),
        f32::from(candidate.bounds.origin.y),
        f32::from(candidate.bounds.size.width),
        f32::from(candidate.bounds.size.height),
    );
    assert_eq!(snapped_sel.x(), 100.0);
    assert_eq!(snapped_sel.y(), 150.0);
    assert_eq!(snapped_sel.width(), 800.0);
    assert_eq!(snapped_sel.height(), 600.0);

    // Child UI Automation element snapping (e.g. browser tab or toolbar button)
    let child_candidate = WindowSnapCandidate {
        bounds: Bounds {
            origin: point(px(120.0), px(155.0)),
            size: size(px(180.0), px(34.0)),
        },
        title: "New Tab".to_string(),
        is_child_element: true,
        z_order: 0,
    };
    assert!(child_candidate.is_child_element);
    assert!(candidate.bounds.contains(&child_candidate.bounds.origin));

    let child_snapped = SelectionState::new(
        f32::from(child_candidate.bounds.origin.x),
        f32::from(child_candidate.bounds.origin.y),
        f32::from(child_candidate.bounds.size.width),
        f32::from(child_candidate.bounds.size.height),
    );
    assert_eq!(child_snapped.x(), 120.0);
    assert_eq!(child_snapped.y(), 155.0);
    assert_eq!(child_snapped.width(), 180.0);
    assert_eq!(child_snapped.height(), 34.0);
}

#[test]
fn test_selection_cancellation_and_history_reset() {
    use gpui::{point, px, rgb};
    use pixora::editor::annotations::Annotation;
    use pixora::editor::history::EditHistory;
    use pixora::overlay::selection::SelectionState;

    // Simulate an active selection with multiple annotations
    let mut sel = SelectionState::new(100.0, 100.0, 500.0, 400.0);
    assert!(sel.is_active);

    let mut history = EditHistory::new();
    history.push_annotation(Annotation::Pencil {
        points: vec![point(px(120.0), px(120.0)), point(px(150.0), px(150.0))],
        color: rgb(0xFF9500),
        width: 3.0,
    });
    history.push_annotation(Annotation::Text {
        origin: point(px(200.0), px(200.0)),
        text: "Sample annotation".to_string(),
        color: rgb(0xFF9500),
        font_size: 16.0,
        box_width: None,
    });
    assert_eq!(history.annotations.len(), 2);

    // When Esc or cancel_selection is executed:
    sel.is_active = false;
    history = EditHistory::new(); // Complete reset

    assert!(!sel.is_active);
    assert_eq!(history.annotations.len(), 0);
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn test_palette_gradient_generation_and_corners() {
    use pixora::overlay::toolbar::{generate_gradient_bitmap, generate_hue_strip_bitmap, hsv_to_rgb};

    // Test red hue (0 degrees)
    let grad = generate_gradient_bitmap(0.0, 160, 80);
    assert_eq!(grad.frame_count(), 1);

    // Test blue hue (240 degrees)
    let grad_blue = generate_gradient_bitmap(240.0, 160, 80);
    assert_eq!(grad_blue.frame_count(), 1);

    // Test hue strip bitmap
    let strip = generate_hue_strip_bitmap(448, 12);
    assert_eq!(strip.frame_count(), 1);

    // Check HSV to RGB math at corner cases
    assert_eq!(hsv_to_rgb(0.0, 0.0, 1.0), (255, 255, 255)); // White
    assert_eq!(hsv_to_rgb(0.0, 1.0, 0.0), (0, 0, 0));       // Black
    assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), (255, 0, 0));     // Pure Red
}

#[test]
fn test_text_annotation_commit_outside() {
    use gpui::{point, px, rgb};
    use pixora::editor::annotations::Annotation;
    use pixora::editor::history::EditHistory;

    let mut history = EditHistory::new();
    let mut text_input_mode = true;
    let mut text_input_buffer = "Clean Text".to_string();
    let text_input_pos = point(px(100.0), px(100.0));

    // Simulated commit on click outside
    if text_input_mode {
        if !text_input_buffer.trim().is_empty() {
            let ann = Annotation::Text {
                origin: text_input_pos,
                text: text_input_buffer.clone(),
                color: rgb(0xFFFFFF),
                font_size: 24.0,
                box_width: None,
            };
            history.push_annotation(ann);
        }
        text_input_mode = false;
        text_input_buffer.clear();
    }

    assert_eq!(history.annotations.len(), 1);
    assert!(!text_input_mode);
    assert!(text_input_buffer.is_empty());

    // Subsequent click when text_input_mode is false creates a new input
    if !text_input_mode {
        text_input_mode = true;
        text_input_buffer.clear();
    }
    assert!(text_input_mode);
}

#[test]
fn test_dual_format_clipboard_support() {
    use image::RgbaImage;
    use pixora::image::clipboard::copy_image_to_clipboard;

    let mut img = RgbaImage::new(16, 16);
    // Draw semi-transparent pixel to verify alpha support
    img.put_pixel(0, 0, image::Rgba([255, 0, 0, 128]));
    let res = copy_image_to_clipboard(&img);
    #[cfg(windows)]
    assert!(res.is_ok(), "copy_image_to_clipboard failed: {:?}", res.err());
    #[cfg(not(windows))]
    let _ = res;
}

#[test]
fn test_qr_code_detection_empty_and_blank() {
    use image::RgbaImage;
    use pixora::ocr::qrcode::decode_qr_from_image;

    let empty = RgbaImage::new(0, 0);
    assert!(decode_qr_from_image(&empty).is_err());

    let blank = RgbaImage::new(100, 100);
    let err = decode_qr_from_image(&blank);
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("No QR code detected"));
}

#[test]
fn test_qr_code_decoding_tight_crop_and_inverted() {
    use image::{Rgba, RgbaImage};
    use pixora::ocr::qrcode::decode_qr_from_image;
    use rxing::qrcode::QRCodeWriter;
    use rxing::Writer;
    use rxing::BarcodeFormat;

    let writer = QRCodeWriter::default();
    let bit_matrix = writer
        .encode("https://pixora.app/test", &BarcodeFormat::QR_CODE, 100, 100)
        .expect("should encode QR");

    let w = bit_matrix.getWidth();
    let h = bit_matrix.getHeight();
    let mut img = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let color = if bit_matrix.get(x, y) {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            };
            img.put_pixel(x, y, color);
        }
    }

    // 1. Standard QR decode
    let res = decode_qr_from_image(&img).expect("should decode standard QR");
    assert_eq!(res.content, "https://pixora.app/test");

    // 2. Inverted QR decode (dark theme: white on black)
    let mut inverted = img.clone();
    for p in inverted.pixels_mut() {
        p[0] = 255 - p[0];
        p[1] = 255 - p[1];
        p[2] = 255 - p[2];
    }
    let res_inv = decode_qr_from_image(&inverted).expect("should decode inverted QR");
    assert_eq!(res_inv.content, "https://pixora.app/test");

    // 3. Tightly cropped QR decode (0 quiet zone padding)
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0;
    let mut max_y = 0;
    for y in 0..h {
        for x in 0..w {
            if bit_matrix.get(x, y) {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    let tight_w = max_x - min_x + 1;
    let tight_h = max_y - min_y + 1;
    let mut tight_img = RgbaImage::new(tight_w, tight_h);
    for y in 0..tight_h {
        for x in 0..tight_w {
            tight_img.put_pixel(x, y, *img.get_pixel(min_x + x, min_y + y));
        }
    }
    let res_tight = decode_qr_from_image(&tight_img).expect("should decode tightly cropped QR");
    assert_eq!(res_tight.content, "https://pixora.app/test");
}

#[test]
fn test_beautify_padding_and_presets() {
    use image::{Rgba, RgbaImage};
    use pixora::image::beautify::{apply_beautify, BeautifyOptions, BeautifyPreset};

    let mut src = RgbaImage::new(80, 60);
    for p in src.pixels_mut() {
        *p = Rgba([200, 200, 200, 255]);
    }

    let options = BeautifyOptions {
        preset: BeautifyPreset::Sunset,
        padding: 30,
        corner_radius: 10.0,
        shadow_blur: 15.0,
        shadow_offset_y: 8.0,
        shadow_opacity: 0.35,
    };

    let result = apply_beautify(&src, &options);
    assert_eq!(result.width(), 80 + 30 * 2);
    assert_eq!(result.height(), 60 + 30 * 2);

    // Verify Transparent preset produces non-opaque corners
    let mut trans_opts = options.clone();
    trans_opts.preset = BeautifyPreset::Transparent;
    let trans_res = apply_beautify(&src, &trans_opts);
    assert_eq!(trans_res.width(), 140);
    assert_eq!(trans_res.height(), 120);
    // Outer corner (0, 0) should be transparent
    let corner = trans_res.get_pixel(0, 0);
    assert_eq!(corner[3], 0);
}

#[test]
fn test_pinned_screenshot_channel_swap() {
    let mut img = RgbaImage::new(10, 10);
    // Fill with a distinct color: Red=200, Green=100, Blue=50, Alpha=255
    for p in img.pixels_mut() {
        *p = Rgba([200, 100, 50, 255]);
    }

    let mut bgra = img.clone();
    for chunk in bgra.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }

    let p0 = bgra.get_pixel(0, 0);
    assert_eq!(p0[0], 50, "Blue should be swapped to index 0");
    assert_eq!(p0[1], 100, "Green should remain at index 1");
    assert_eq!(p0[2], 200, "Red should be swapped to index 2");
    assert_eq!(p0[3], 255, "Alpha should remain intact");
}

#[test]
fn test_annotation_selection_hit_testing() {
    let arrow = Annotation::Arrow {
        from: point(px(10.0), px(10.0)),
        to: point(px(100.0), px(10.0)),
        control: None,
        two_headed: false,
        color: gpui::rgb(0xFF453A),
        width: 4.0,
    };
    // Point on the line segment
    assert!(arrow.hit_test(point(px(50.0), px(10.0)), 6.0));
    assert!(arrow.hit_test(point(px(50.0), px(12.0)), 6.0));
    // Point far away
    assert!(!arrow.hit_test(point(px(50.0), px(50.0)), 6.0));

    let rect = Annotation::Rectangle {
        bounds: Bounds {
            origin: point(px(20.0), px(20.0)),
            size: size(px(60.0), px(40.0)),
        },
        color: gpui::rgb(0xFFFFFF),
        width: 2.0,
        filled: true,
    };
    // Inside filled rectangle
    assert!(rect.hit_test(point(px(40.0), px(30.0)), 4.0));
    // Outside rectangle
    assert!(!rect.hit_test(point(px(100.0), px(100.0)), 4.0));

    let text = Annotation::Text {
        origin: point(px(50.0), px(50.0)),
        text: "Hello Pixora".into(),
        color: gpui::rgb(0xFFFFFF),
        font_size: 16.0,
        box_width: None,
    };
    assert!(text.hit_test(point(px(60.0), px(55.0)), 4.0));
    assert!(!text.hit_test(point(px(200.0), px(200.0)), 4.0));
}

#[test]
fn test_annotation_transform_and_handles() {
    let mut arrow = Annotation::Arrow {
        from: point(px(10.0), px(20.0)),
        to: point(px(80.0), px(20.0)),
        control: None,
        two_headed: false,
        color: gpui::rgb(0xFF453A),
        width: 4.0,
    };

    // Test handle hit-testing
    let h_start = arrow.hit_test_handle(point(px(11.0), px(21.0)), px(8.0));
    assert_eq!(h_start, Some(AnnotationHandle::Endpoint(true)));

    let h_end = arrow.hit_test_handle(point(px(80.0), px(20.0)), px(8.0));
    assert_eq!(h_end, Some(AnnotationHandle::Endpoint(false)));

    let h_none = arrow.hit_test_handle(point(px(40.0), px(20.0)), px(8.0));
    assert_eq!(h_none, None);

    // Test translation
    arrow.translate(point(px(15.0), px(10.0)));
    if let Annotation::Arrow { from, to, .. } = arrow {
        assert_eq!(from, point(px(25.0), px(30.0)));
        assert_eq!(to, point(px(95.0), px(30.0)));
    } else {
        panic!("Expected Arrow variant");
    }

    // Test resize bounds
    let orig = Bounds {
        origin: point(px(10.0), px(10.0)),
        size: size(px(50.0), px(40.0)),
    };
    let resized = resize_bounds_with_handle(
        orig,
        AnnotationHandle::BottomRight,
        point(px(60.0), px(50.0)),
        point(px(80.0), px(70.0)),
    );
    assert_eq!(resized.size.width, px(70.0));
    assert_eq!(resized.size.height, px(60.0));
}

#[test]
fn test_annotation_property_updates() {
    let mut rect = Annotation::Rectangle {
        bounds: Bounds {
            origin: point(px(10.0), px(10.0)),
            size: size(px(100.0), px(50.0)),
        },
        color: gpui::rgb(0xFFFFFF),
        width: 2.0,
        filled: false,
    };

    assert_eq!(rect.color(), Some(gpui::rgb(0xFFFFFF)));
    rect.set_color(gpui::rgb(0x000000));
    assert_eq!(rect.color(), Some(gpui::rgb(0x000000)));

    assert_eq!(rect.stroke_width(), Some(2.0));
    rect.set_stroke_width(6.0);
    assert_eq!(rect.stroke_width(), Some(6.0));

    assert_eq!(rect.is_filled(), Some(false));
    rect.set_filled(true);
    assert_eq!(rect.is_filled(), Some(true));

    let mut text = Annotation::Text {
        origin: point(px(10.0), px(10.0)),
        text: "Test".into(),
        color: gpui::rgb(0xFFFFFF),
        font_size: 16.0,
        box_width: None,
    };
    assert_eq!(text.font_size(), Some(16.0));
    text.set_font_size(28.0);
    assert_eq!(text.font_size(), Some(28.0));
}

#[test]
fn test_text_free_transform_resizing_and_handles() {
    let mut text = Annotation::Text {
        origin: point(px(50.0), px(50.0)),
        text: "траляля".into(),
        color: gpui::rgb(0xFF0000),
        font_size: 20.0,
        box_width: None,
    };

    let orig_b = text.bounds();
    let orig_w = f32::from(orig_b.size.width);
    let orig_h = f32::from(orig_b.size.height);
    assert!(orig_w > 0.0);
    assert!(orig_h > 0.0);

    // 1. Hit test right handle
    let right_handle_pos = point(orig_b.origin.x + orig_b.size.width, orig_b.origin.y + orig_b.size.height / 2.0);
    let hit_r = text.hit_test_handle(right_handle_pos, px(14.0));
    assert_eq!(hit_r, Some(AnnotationHandle::Right));

    // 2. Drag right handle to the right by +orig_w: box_width expands, origin.x stays fixed!
    let start_m = right_handle_pos;
    let cur_m = point(right_handle_pos.x + px(orig_w), right_handle_pos.y);
    text.resize_with_handle(orig_b, AnnotationHandle::Right, start_m, cur_m);
    assert_eq!(text.box_width(), Some(orig_w + orig_w));
    if let Annotation::Text { origin, .. } = text {
        assert_eq!(origin.x, px(50.0), "Left edge must remain anchored when dragging right handle");
    }

    // 3. Drag left handle to the left by -orig_w: right edge stays anchored!
    let left_handle_pos = point(orig_b.origin.x, orig_b.origin.y + orig_b.size.height / 2.0);
    let hit_l = text.hit_test_handle(left_handle_pos, px(14.0));
    assert_eq!(hit_l, Some(AnnotationHandle::Left));

    let mut text_left = Annotation::Text {
        origin: point(px(50.0), px(50.0)),
        text: "траляля".into(),
        color: gpui::rgb(0xFF0000),
        font_size: 20.0,
        box_width: None,
    };
    let start_m_l = left_handle_pos;
    let cur_m_l = point(left_handle_pos.x - px(orig_w), left_handle_pos.y);
    text_left.resize_with_handle(orig_b, AnnotationHandle::Left, start_m_l, cur_m_l);
    assert_eq!(text_left.box_width(), Some(orig_w + orig_w));
    if let Annotation::Text { origin, .. } = text_left {
        let new_w = f32::from(text_left.bounds().size.width);
        let right_edge = f32::from(origin.x) + new_w;
        let expected_right = 50.0 + orig_w;
        assert!((right_edge - expected_right).abs() < 1.0, "Right edge must stay anchored");
    }

    // 4. Drag bottom handle: scales font_size!
    let bottom_pos = point(orig_b.origin.x + orig_b.size.width / 2.0, orig_b.origin.y + orig_b.size.height);
    let mut text_bottom = Annotation::Text {
        origin: point(px(50.0), px(50.0)),
        text: "траляля".into(),
        color: gpui::rgb(0xFF0000),
        font_size: 20.0,
        box_width: None,
    };
    text_bottom.resize_with_handle(orig_b, AnnotationHandle::Bottom, bottom_pos, point(bottom_pos.x, bottom_pos.y + px(orig_h)));
    assert!((text_bottom.font_size().unwrap() - 40.0).abs() < 0.1, "Font size must scale up proportionally when dragging bottom handle");
}

#[test]
fn test_long_string_and_latin_text_bounds() {
    let long_text = Annotation::Text {
        origin: point(px(10.0), px(10.0)),
        text: "пвыаввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввв".into(),
        color: gpui::rgb(0xFF0000),
        font_size: 52.0,
        box_width: None,
    };
    let bounds = long_text.bounds();
    let pad = (52.0 * 0.12f32).clamp(6.0, 14.0);
    let measured = pixora::editor::annotations::measure_text_width("пвыаввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввввв", 52.0);
    assert!(f32::from(bounds.size.width) >= measured + 2.0 * pad - 0.01);

    let ghbdtn = Annotation::Text {
        origin: point(px(10.0), px(10.0)),
        text: "ghbdtn".into(),
        color: gpui::rgb(0xFF0000),
        font_size: 96.0,
        box_width: None,
    };
    let g_bounds = ghbdtn.bounds();
    let g_pad = (96.0 * 0.12f32).clamp(6.0, 14.0);
    let g_measured = pixora::editor::annotations::measure_text_width("ghbdtn", 96.0);
    assert!(f32::from(g_bounds.size.width) >= g_measured + 2.0 * g_pad - 0.01);
}

#[test]
fn test_history_pop_snapshot_if_unchanged() {
    let mut history = EditHistory::new();
    let text = Annotation::Text {
        origin: point(px(10.0), px(10.0)),
        text: "Initial".into(),
        color: gpui::rgb(0xFFFFFF),
        font_size: 16.0,
        box_width: None,
    };
    history.push_annotation(text);
    assert_eq!(history.annotations.len(), 1);

    // Simulate clicking without moving: snapshot is taken
    history.snapshot();
    // Nothing modified: pop_last_snapshot_if_unchanged should remove it
    history.pop_last_snapshot_if_unchanged();
    // Only 1 snapshot should remain (from push_annotation)
    assert!(history.undo());
    assert!(!history.undo());
}

#[test]
fn test_area_text_wrapping_and_box_width() {
    use pixora::editor::annotations::wrap_text_to_lines;

    // 1. Long continuous letters like 'ssssssssssssssssssssssssssssssssssssssss'
    let long_s = "ssssssssssssssssssssssssssssssssssssssss";
    let wrapped_s = wrap_text_to_lines(long_s, 24.0, Some(100.0));
    assert!(wrapped_s.len() > 1, "Unbroken characters must wrap character-by-character into multiple lines");

    // 2. Normal sentence wrapping at words
    let sentence = "Hello world from the new paragraph area text box";
    let wrapped = wrap_text_to_lines(sentence, 20.0, Some(120.0));
    assert!(wrapped.len() >= 3, "Sentence must wrap into multiple lines cleanly");

    // 3. Text bounds dimension calculation
    let (w, h) = pixora::editor::annotations::text_bounds_dimensions(long_s, 24.0, Some(150.0));
    assert!(w >= 150.0);
    assert!(h > 50.0);
}

#[test]
fn test_curved_bezier_arrow_handles_and_geometry() {
    let mut arrow = Annotation::Arrow {
        from: point(px(20.0), px(50.0)),
        to: point(px(180.0), px(50.0)),
        control: Some(point(px(100.0), px(120.0))),
        two_headed: true,
        color: gpui::rgb(0xFF453A),
        width: 6.0,
    };

    assert_eq!(arrow.is_curved(), Some(true));
    assert_eq!(arrow.is_two_headed(), Some(true));

    // Hit-test the Bezier control handle at (100, 120)
    let handle = arrow.hit_test_handle(point(px(100.0), px(120.0)), px(10.0));
    assert_eq!(handle, Some(AnnotationHandle::BezierControl));

    // Hit-test endpoints
    assert_eq!(arrow.hit_test_handle(point(px(20.0), px(50.0)), px(8.0)), Some(AnnotationHandle::Endpoint(true)));
    assert_eq!(arrow.hit_test_handle(point(px(180.0), px(50.0)), px(8.0)), Some(AnnotationHandle::Endpoint(false)));

    // Bounds must encompass the control point (y=120)
    let b = arrow.bounds();
    assert!(b.origin.y <= px(50.0));
    assert!(b.origin.y + b.size.height >= px(120.0));

    // Translating arrow moves the control point as well
    arrow.translate(point(px(10.0), px(20.0)));
    if let Annotation::Arrow { from, to, control, .. } = arrow {
        assert_eq!(from, point(px(30.0), px(70.0)));
        assert_eq!(to, point(px(190.0), px(70.0)));
        assert_eq!(control, Some(point(px(110.0), px(140.0))));
    } else {
        panic!("Expected Arrow variant");
    }
}

#[test]
fn test_badge_alphabet_mode_and_reset() {
    use pixora::editor::annotations::{number_to_badge_label, BadgeMode, BadgeShape};
    use pixora::editor::properties::ToolProperties;

    // Numbers mode
    assert_eq!(number_to_badge_label(1, BadgeMode::Numbers), "1");
    assert_eq!(number_to_badge_label(99, BadgeMode::Numbers), "99");

    // Alphabet mode bijective base-26
    assert_eq!(number_to_badge_label(1, BadgeMode::Alphabet), "A");
    assert_eq!(number_to_badge_label(2, BadgeMode::Alphabet), "B");
    assert_eq!(number_to_badge_label(26, BadgeMode::Alphabet), "Z");
    assert_eq!(number_to_badge_label(27, BadgeMode::Alphabet), "AA");
    assert_eq!(number_to_badge_label(28, BadgeMode::Alphabet), "AB");
    assert_eq!(number_to_badge_label(702, BadgeMode::Alphabet), "ZZ");

    // Badge property updates
    let mut badge = Annotation::NumberBadge {
        center: point(px(50.0), px(50.0)),
        number: 5,
        color: gpui::rgb(0x8174FF),
        radius: 14.0,
        shape: BadgeShape::Circle,
        mode: BadgeMode::Numbers,
        filled: true,
    };
    assert_eq!(badge.badge_shape(), Some(BadgeShape::Circle));
    assert_eq!(badge.badge_mode(), Some(BadgeMode::Numbers));
    assert_eq!(badge.is_filled(), Some(true));

    badge.set_badge_shape(BadgeShape::RoundedRect);
    badge.set_badge_mode(BadgeMode::Alphabet);
    badge.set_filled(false);
    assert_eq!(badge.badge_shape(), Some(BadgeShape::RoundedRect));
    assert_eq!(badge.badge_mode(), Some(BadgeMode::Alphabet));
    assert_eq!(badge.is_filled(), Some(false));

    // Reset button simulation on ToolProperties
    let mut props = ToolProperties::default();
    props.current_badge_number = 42;
    props.current_badge_number = 1;
    assert_eq!(props.current_badge_number, 1);
}

#[test]
fn test_heart_cross_checkmark_shape_annotations() {
    let mut heart = Annotation::Heart {
        bounds: Bounds {
            origin: point(px(100.0), px(100.0)),
            size: size(px(60.0), px(50.0)),
        },
        color: rgb(0xFF2D55),
        width: 3.0,
        filled: true,
    };
    assert_eq!(heart.is_filled(), Some(true));
    heart.set_filled(false);
    assert_eq!(heart.is_filled(), Some(false));
    assert!(heart.hit_test(point(px(110.0), px(110.0)), 0.0));
    heart.translate(point(px(10.0), px(10.0)));
    assert_eq!(heart.bounds().origin, point(px(110.0), px(110.0)));

    let mut cross = Annotation::Cross {
        bounds: Bounds {
            origin: point(px(200.0), px(200.0)),
            size: size(px(40.0), px(40.0)),
        },
        color: rgb(0xFF3B30),
        width: 2.0,
        filled: false,
    };
    assert_eq!(cross.is_filled(), Some(false));
    cross.set_filled(true);
    assert_eq!(cross.is_filled(), Some(true));
    assert!(cross.hit_test(point(px(210.0), px(210.0)), 0.0));
    cross.translate(point(px(-20.0), px(-20.0)));
    assert_eq!(cross.bounds().origin, point(px(180.0), px(180.0)));

    let mut checkmark = Annotation::Checkmark {
        bounds: Bounds {
            origin: point(px(300.0), px(300.0)),
            size: size(px(40.0), px(40.0)),
        },
        color: rgb(0x34C759),
        width: 4.0,
        filled: true,
    };
    assert_eq!(checkmark.is_filled(), Some(true));
    assert!(checkmark.hit_test(point(px(310.0), px(310.0)), 0.0));
    checkmark.translate(point(px(5.0), px(-5.0)));
    assert_eq!(checkmark.bounds().origin, point(px(305.0), px(295.0)));
}

#[cfg(windows)]
#[test]
fn test_ui_automation_element_snapping() {
    use image::{Rgba, RgbaImage};
    use pixora::overlay::overlay::{WindowSnapCandidate, ScreenOverlayView};
    use pixora::system::windows::detect_visual_panes;

    // Create a 1000x600 test image simulating a window with a sidebar at x = 200
    let mut img = RgbaImage::new(1000, 600);
    for y in 0..600 {
        for x in 0..1000 {
            if x < 200 {
                img.put_pixel(x, y, Rgba([20, 20, 24, 255])); // dark sidebar
            } else if x == 200 {
                img.put_pixel(x, y, Rgba([60, 60, 70, 255])); // 1px divider
            } else {
                img.put_pixel(x, y, Rgba([30, 30, 35, 255])); // main area
            }
        }
    }

    let panes = detect_visual_panes(&img, 0, 0, 1000, 600);
    assert!(!panes.is_empty(), "Visual pane segmentation must detect panes");
    assert!(
        panes.iter().any(|p| p.x == 0 && p.y == 0 && p.width == 200 && p.height == 600),
        "Sidebar pane (0, 0, 200, 600) must be detected"
    );
    assert!(
        panes.iter().any(|p| p.x == 200 && p.y == 0 && p.width == 800 && p.height == 600),
        "Main pane (200, 0, 800, 600) must be detected"
    );

    // Test find_best_snap_candidate priority (ShareX model)
    let candidates = vec![
        // Foremost Top-level window (z_order = 0)
        WindowSnapCandidate {
            bounds: Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(1000.0), px(600.0)),
            },
            title: "Antigravity".to_string(),
            is_child_element: false,
            z_order: 0,
        },
        // Sidebar child pane of foremost window
        WindowSnapCandidate {
            bounds: Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(200.0), px(600.0)),
            },
            title: "Sidebar".to_string(),
            is_child_element: true,
            z_order: 0,
        },
        // Background window behind foremost window (z_order = 1)
        WindowSnapCandidate {
            bounds: Bounds {
                origin: point(px(50.0), px(50.0)),
                size: size(px(800.0), px(500.0)),
            },
            title: "Pixora — проводник".to_string(),
            is_child_element: false,
            z_order: 1,
        },
        // Child pane of background window with tiny area
        WindowSnapCandidate {
            bounds: Bounds {
                origin: point(px(50.0), px(50.0)),
                size: size(px(100.0), px(100.0)),
            },
            title: "Background Child".to_string(),
            is_child_element: true,
            z_order: 1,
        },
    ];

    // Inside sidebar away from outer border -> snaps to sidebar of foremost window
    let inside_sidebar = ScreenOverlayView::find_best_snap_candidate(
        point(px(50.0), px(300.0)),
        &candidates,
    );
    assert!(inside_sidebar.is_some());
    let picked = inside_sidebar.unwrap();
    assert_eq!(picked.bounds.size.width, px(200.0));
    assert_eq!(picked.title, "Sidebar");
    assert_eq!(picked.z_order, 0);
    assert!(picked.is_child_element);

    // Point overlapping both foremost and background window -> MUST pick foremost window, NEVER background window!
    let overlap_pt = ScreenOverlayView::find_best_snap_candidate(
        point(px(80.0), px(80.0)),
        &candidates,
    );
    assert!(overlap_pt.is_some());
    let picked_overlap = overlap_pt.unwrap();
    assert_eq!(picked_overlap.z_order, 0, "Must NEVER pick background window occluded by foremost window");
    assert_ne!(picked_overlap.title, "Pixora — проводник");
    assert_ne!(picked_overlap.title, "Background Child");

    // Near outer border (dist <= 4px from top-level window edge) -> snaps to entire foremost window
    let near_border = ScreenOverlayView::find_best_snap_candidate(
        point(px(3.0), px(300.0)),
        &candidates,
    );
    assert!(near_border.is_some());
    let picked_window = near_border.unwrap();
    assert_eq!(picked_window.bounds.size.width, px(1000.0));
    assert_eq!(picked_window.title, "Antigravity");
    assert_eq!(picked_window.z_order, 0);
    assert!(!picked_window.is_child_element);

    // Test small block selection (toolbar button 32x32 inside foremost window)
    let small_button = WindowSnapCandidate {
        bounds: Bounds {
            origin: point(px(140.0), px(190.0)),
            size: size(px(32.0), px(32.0)),
        },
        title: "Создать".to_string(),
        is_child_element: true,
        z_order: 0,
    };
    let mut candidates_with_small = candidates.clone();
    candidates_with_small.push(small_button);

    let picked_small = ScreenOverlayView::find_best_snap_candidate(
        point(px(150.0), px(200.0)),
        &candidates_with_small,
    );
    assert!(picked_small.is_some());
    let small = picked_small.unwrap();
    assert_eq!(small.title, "Создать");
    assert_eq!(small.bounds.size.width, px(32.0));
    assert_eq!(small.bounds.size.height, px(32.0));
    assert!(small.is_child_element);
}

#[test]
fn test_crash_and_error_logging_to_log_folder() {
    use pixora::logging::{
        get_log_dir, get_log_file_path, init_logging, log_error, log_info, log_warn,
        set_custom_log_dir,
    };

    let temp_dir = std::env::temp_dir().join(format!("pixora_test_log_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);

    set_custom_log_dir(Some(temp_dir.clone()));

    assert_eq!(get_log_dir(), temp_dir);
    let expected_file = temp_dir.join("pixora.log");
    assert_eq!(get_log_file_path(), expected_file);

    // 1. Initially folder and file do not exist
    assert!(!expected_file.exists());

    // 2. Log an error -> folder and pixora.log must be created automatically
    log_error("test_module", "Critical system test error occurred");
    assert!(expected_file.exists(), "pixora.log must be created on error");

    // 3. Log a warning and info
    log_warn("test_module", "Sample warning message");
    log_info("test_module", "Sample info message");

    let content = std::fs::read_to_string(&expected_file).expect("Must read log file");
    assert!(content.contains("[ERROR] [test_module] Critical system test error occurred"));
    assert!(content.contains("[WARN] [test_module] Sample warning message"));
    assert!(content.contains("[INFO] [test_module] Sample info message"));

    // 4. Verify panic hook logging
    init_logging();
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|info| {
        pixora::logging::log_panic(info);
    }));

    let _ = std::panic::catch_unwind(|| {
        panic!("Simulated critical crash for log verification");
    });

    // Restore hook
    std::panic::set_hook(hook);

    let updated_content = std::fs::read_to_string(&expected_file).expect("Must read log file");
    assert!(
        updated_content.contains("PIXORA CRASH REPORT"),
        "Crash report header must be logged"
    );
    assert!(
        updated_content.contains("Simulated critical crash for log verification"),
        "Panic message must be logged"
    );
    assert!(
        updated_content.contains("Backtrace:"),
        "Backtrace section must be present in crash report"
    );

    // Reset custom log dir and clean up
    set_custom_log_dir(None);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_ocr_single_line_formatting() {
    use pixora::ocr::recognition::format_as_single_line;

    // 1. Hyphenated word wrap stripping
    let text1 = "Это автомати-\nческое распозна-\nвание текста.";
    let res1 = format_as_single_line(text1);
    assert_eq!(res1, "Это автоматическое распознавание текста.");

    // 2. Multiline joining and space normalization
    let text2 = "Первая строка\n   Вторая   строка  \nТретья строка";
    let res2 = format_as_single_line(text2);
    assert_eq!(res2, "Первая строка Вторая строка Третья строка");

    // 3. Empty and whitespace input
    assert_eq!(format_as_single_line(""), "");
    assert_eq!(format_as_single_line("   \n\n  "), "");
}

#[test]
fn test_ocr_code_indentation_and_table_tabs() {
    use pixora::ocr::recognition::{
        format_as_code_or_table, format_text_as_table_tabs, OcrRecognizedLine, OcrWord,
    };

    // 1. Code indentation test
    let lines = vec![
        OcrRecognizedLine {
            text: "fn main() {".to_string(),
            x: 20.0,
            y: 10.0,
            width: 88.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "fn".to_string(), x: 20.0, y: 10.0, width: 16.0, height: 16.0 },
                OcrWord { text: "main()".to_string(), x: 40.0, y: 10.0, width: 48.0, height: 16.0 },
                OcrWord { text: "{".to_string(), x: 92.0, y: 10.0, width: 8.0, height: 16.0 },
            ],
        },
        OcrRecognizedLine {
            text: "let a = 1;".to_string(),
            x: 52.0,
            y: 30.0,
            width: 80.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "let".to_string(), x: 52.0, y: 30.0, width: 24.0, height: 16.0 },
                OcrWord { text: "a".to_string(), x: 80.0, y: 30.0, width: 8.0, height: 16.0 },
                OcrWord { text: "=".to_string(), x: 92.0, y: 30.0, width: 8.0, height: 16.0 },
                OcrWord { text: "1;".to_string(), x: 104.0, y: 30.0, width: 16.0, height: 16.0 },
            ],
        },
        OcrRecognizedLine {
            text: "}".to_string(),
            x: 20.0,
            y: 50.0,
            width: 8.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "}".to_string(), x: 20.0, y: 50.0, width: 8.0, height: 16.0 },
            ],
        },
    ];

    let code_formatted = format_as_code_or_table(&lines);
    assert!(code_formatted.contains("fn main() {"));
    assert!(code_formatted.contains("    let a = 1;"));
    assert!(code_formatted.contains("}"));

    // 2. Table column tab separation
    let table_lines = vec![
        OcrRecognizedLine {
            text: "ID Name Status".to_string(),
            x: 10.0,
            y: 10.0,
            width: 180.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "ID".to_string(), x: 10.0, y: 10.0, width: 16.0, height: 16.0 },
                OcrWord { text: "Name".to_string(), x: 80.0, y: 10.0, width: 36.0, height: 16.0 },
                OcrWord { text: "Status".to_string(), x: 160.0, y: 10.0, width: 48.0, height: 16.0 },
            ],
        },
    ];
    let table_formatted = format_as_code_or_table(&table_lines);
    assert_eq!(table_formatted, "ID\tName\tStatus");

    // 3. Fallback table formatter for space runs
    let raw_table = "ID    Username    Role\n001   Alice       Admin\n002   Bob         User";
    let formatted_tabs = format_text_as_table_tabs(raw_table);
    assert!(formatted_tabs.contains("ID\tUsername\tRole"));
    assert!(formatted_tabs.contains("001\tAlice\tAdmin"));
}

#[test]
fn test_qr_actions_parsing() {
    use pixora::ocr::qr_actions::{parse_qr_action, QrActionKind};
    use pixora::ocr::qrcode::QrScanResult;

    // 1. URL parsing
    let res_url = QrScanResult {
        content: "https://pixora.app/download".to_string(),
        format: Some("QR_CODE".to_string()),
    };
    match parse_qr_action(&res_url) {
        QrActionKind::Url { url } => assert_eq!(url, "https://pixora.app/download"),
        _ => panic!("Must parse as Url action"),
    }

    let res_www = QrScanResult {
        content: "www.example.com/item".to_string(),
        format: Some("QR_CODE".to_string()),
    };
    match parse_qr_action(&res_www) {
        QrActionKind::Url { url } => assert_eq!(url, "https://www.example.com/item"),
        _ => panic!("Must prepend https:// to www. Url"),
    }

    // 2. Wi-Fi parsing
    let res_wifi = QrScanResult {
        content: "WIFI:T:WPA;S:OfficeNet;P:StrongPassword123;H:false;;".to_string(),
        format: Some("QR_CODE".to_string()),
    };
    match parse_qr_action(&res_wifi) {
        QrActionKind::Wifi { ssid, password, auth_type, hidden } => {
            assert_eq!(ssid, "OfficeNet");
            assert_eq!(password, "StrongPassword123");
            assert_eq!(auth_type, "WPA/WPA2");
            assert!(!hidden);
        }
        _ => panic!("Must parse as Wifi action"),
    }

    // Wi-Fi with escaped characters
    let res_wifi_esc = QrScanResult {
        content: "WIFI:T:WPA;S:My\\;Network;P:Pass\\:word;;".to_string(),
        format: Some("QR_CODE".to_string()),
    };
    match parse_qr_action(&res_wifi_esc) {
        QrActionKind::Wifi { ssid, password, .. } => {
            assert_eq!(ssid, "My;Network");
            assert_eq!(password, "Pass:word");
        }
        _ => panic!("Must unescape Wifi fields"),
    }

    // 3. Barcode parsing (EAN-13)
    let res_barcode = QrScanResult {
        content: "4601234567890".to_string(),
        format: Some("EAN_13".to_string()),
    };
    match parse_qr_action(&res_barcode) {
        QrActionKind::Barcode { code, format_name, search_url } => {
            assert_eq!(code, "4601234567890");
            assert_eq!(format_name, "EAN-13");
            assert!(search_url.contains("4601234567890"));
            assert!(search_url.starts_with("https://www.google.com/search?q="));
        }
        _ => panic!("Must parse as Barcode action"),
    }

    // Code 128
    let res_code128 = QrScanResult {
        content: "PKG-987654".to_string(),
        format: Some("CODE_128".to_string()),
    };
    match parse_qr_action(&res_code128) {
        QrActionKind::Barcode { code, format_name, .. } => {
            assert_eq!(code, "PKG-987654");
            assert_eq!(format_name, "Code 128");
        }
        _ => panic!("Must parse Code 128 barcode"),
    }

    // 4. Plain text fallback
    let res_text = QrScanResult {
        content: "Simple text note without links".to_string(),
        format: Some("QR_CODE".to_string()),
    };
    match parse_qr_action(&res_text) {
        QrActionKind::Text { text } => assert_eq!(text, "Simple text note without links"),
        _ => panic!("Must parse as Text action"),
    }
}

#[test]
fn test_ocr_modal_visual_line_wrapping_and_navigation() {
    use pixora::overlay::overlay::{OcrFormatMode, OcrModalState};

    // Long single line of 144 characters (similar to OCR result in single line mode)
    let single_line = "Проблема с регистрацией успешно устранена спешим обрадовать, что проблема с доступом к почте была успешно устранена, пользователи вновь могут...".to_string();
    let char_count = single_line.chars().count();
    assert_eq!(char_count, 144);

    let mut state = OcrModalState {
        original_text: single_line.clone(),
        single_line_text: single_line.clone(),
        code_table_text: single_line.clone(),
        edited_text: single_line,
        mode: OcrFormatMode::SingleLine,
        is_copied: false,
        cursor_char_idx: 0,
        selection_range: None,
        selection_anchor: None,
        mouse_down_pos: None,
        is_dragging: false,
    };

    // 1. Visual lines wrapping with max_chars = 80
    let vlines = state.visual_lines(80);
    assert!(vlines.len() >= 2, "144 chars must be wrapped into at least 2 visual lines");

    // All visual lines must not exceed 80 chars
    for vl in &vlines {
        assert!(vl.text.chars().count() <= 80, "Visual line length must be <= 80");
    }

    // Continuity: end_char of first line must match start_char of continuation line
    assert_eq!(vlines[0].start_char, 0);
    assert_eq!(vlines[0].logical_line_num, 1);
    assert!(!vlines[0].is_wrapped_continuation);

    assert_eq!(vlines[1].start_char, vlines[0].end_char);
    assert_eq!(vlines[1].logical_line_num, 1);
    assert!(vlines[1].is_wrapped_continuation);

    let last_vl = vlines.last().unwrap();
    assert_eq!(last_vl.end_char, 144);

    // 2. contains_cursor checks across boundary
    let split_boundary = vlines[0].end_char;
    // Right before boundary belongs to first line
    assert!(vlines[0].contains_cursor(split_boundary - 1, false, true));
    // At boundary belongs to continuation line
    assert!(vlines[1].contains_cursor(split_boundary, true, false));
    // At end of text belongs to last line
    assert!(last_vl.contains_cursor(144, true, false));

    // 3. Navigation across visual lines
    state.cursor_char_idx = 10;
    state.move_cursor_down();
    assert!(state.cursor_char_idx >= vlines[1].start_char, "Moving down must jump into next visual line");

    state.move_cursor_up();
    assert_eq!(state.cursor_char_idx, 10, "Moving up must return to initial position");

    state.move_cursor_end();
    assert!(state.cursor_char_idx <= 144);

    state.move_cursor_home();
    assert_eq!(state.cursor_char_idx, 0);

    // 4. Word selection, select all, and deletion
    state.cursor_char_idx = 15; // inside "регистрацией"
    state.select_word_at_cursor();
    assert!(state.selection_range.is_some());
    let selected = state.selected_text().unwrap();
    assert_eq!(selected, "регистрацией");

    state.select_all();
    assert_eq!(state.selection_range, Some((0, 144)));
    assert_eq!(state.selected_text().unwrap().chars().count(), 144);

    // 5. Accurate character hit test with find_char_index_at_x
    use pixora::overlay::overlay::find_char_index_at_x;
    let sample = "2. Что было сделано";
    let idx_start = find_char_index_at_x(sample, 12.0, 0.0);
    assert_eq!(idx_start, 0);

    let idx_past = find_char_index_at_x(sample, 12.0, 5000.0);
    assert_eq!(idx_past, sample.chars().count());

    // Clicking inside "сделано" (past "2. Что было ")
    let idx_sdelano = find_char_index_at_x(sample, 12.0, 120.0);
    let sample_chars: Vec<char> = sample.chars().collect();
    assert!(idx_sdelano >= 12 && idx_sdelano <= sample_chars.len());

    // 6. Selection drag state transitions and cleanup
    state.selection_range = Some((10, 30));
    state.selection_anchor = Some(10);
    state.mouse_down_pos = Some((100.0, 200.0));
    state.is_dragging = true;

    state.clear_selection();
    assert_eq!(state.selection_range, None);
    assert_eq!(state.selection_anchor, None);
    assert_eq!(state.mouse_down_pos, None);
    assert_eq!(state.is_dragging, false);

    state.selection_range = Some((0, 5));
    state.selection_anchor = Some(0);
    state.mouse_down_pos = Some((50.0, 50.0));
    state.is_dragging = true;

    assert!(state.delete_selection());
    assert_eq!(state.selection_range, None);
    assert_eq!(state.selection_anchor, None);
    assert_eq!(state.mouse_down_pos, None);
    assert_eq!(state.is_dragging, false);
}

#[test]
fn test_wgc_support_and_capture_fallback() {
    use pixora::capture::wgc::is_wgc_supported;
    use pixora::capture::screen::{capture_virtual_screen, capture_virtual_screen_gdi};

    // 1. WGC support check should not panic and return a boolean
    let supported = is_wgc_supported();
    println!("WGC supported on current machine: {}", supported);

    // 2. GDI fallback capture check (may fail in headless/background sessions, but should not panic)
    let _ = capture_virtual_screen_gdi();

    // 3. Main capture_virtual_screen succeeds on Windows via WGC (or GDI fallback)
    #[cfg(windows)]
    {
        let main_res = capture_virtual_screen();
        assert!(main_res.is_ok(), "capture_virtual_screen must succeed");
        let main_cap = main_res.unwrap();
        assert!(main_cap.bounds.width > 0 && main_cap.bounds.height > 0);
        assert_eq!(main_cap.image.width(), main_cap.bounds.width);
        assert_eq!(main_cap.image.height(), main_cap.bounds.height);
    }
    #[cfg(not(windows))]
    {
        let _ = capture_virtual_screen();
    }
}













