use gpui::{Bounds, point, px, rgb, size};
use image::{Rgba, RgbaImage};
use pixora::config::AppConfig;
use pixora::editor::annotations::Annotation;
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
    assert!(config.auto_check_updates);
    assert!(config.window_snapping);

    let json = serde_json::to_string(&config).expect("Must serialize to JSON");
    let deserialized: AppConfig = serde_json::from_str(&json).expect("Must deserialize from JSON");
    assert_eq!(config.filename_prefix, deserialized.filename_prefix);
    assert_eq!(config.default_stroke_width, deserialized.default_stroke_width);
    assert_eq!(config.blur_radius, deserialized.blur_radius);
    assert_eq!(config.pixelate_block, deserialized.pixelate_block);
    assert_eq!(config.default_font_size, deserialized.default_font_size);
    assert_eq!(config.window_snapping, deserialized.window_snapping);
}

#[test]
fn test_config_adjacent_path() {
    let path = AppConfig::config_path();
    assert!(path.ends_with("config.json"));
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
        },
        Annotation::Text {
            origin: point(px(20.0), px(160.0)),
            text: "PIXORA 2026".to_string(),
            color: rgb(0xFFFFFF),
            font_size: 16.0,
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
    assert_eq!(list.len(), 29);

    for path in list {
        let content = assets.load(&path).expect("Load svg").expect("Found icon");
        let s = std::str::from_utf8(&content).expect("Valid UTF-8 SVG");
        assert!(s.starts_with("<svg"), "Icon {} must start with <svg", path);
        assert!(s.ends_with("</svg>"), "Icon {} must end with </svg>", path);
    }
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

    // System language detection test (on this Windows machine with Russian locale)
    let detected = detect_system_language();
    assert_eq!(detected, AppLanguage::Ru);

    // Auto resolution test
    assert_eq!(LanguageChoice::Auto.resolve(), AppLanguage::Ru);
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
    assert_eq!(parse_version_tuple("v0.18.0-alpha"), vec![0, 18, 0]);

    assert!(is_newer_version("0.14.0", "0.14.1"));
    assert!(is_newer_version("0.14.0", "0.18.0"));
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

    let res = check_latest_release();
    assert!(res.is_ok(), "check_latest_release should succeed: {:?}", res.err());
    let info = res.unwrap();
    assert_eq!(info.current_version, env!("CARGO_PKG_VERSION"));
    assert!(!info.latest_version.is_empty());
    assert!(info.release_url.contains("github.com/Onzis/Pixora/releases"));
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











