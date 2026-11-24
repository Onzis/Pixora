use crate::ocr::qrcode::QrScanResult;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QrActionKind {
    Url {
        url: String,
    },
    Wifi {
        ssid: String,
        password: String,
        auth_type: String,
        hidden: bool,
    },
    Barcode {
        code: String,
        format_name: String,
        search_url: String,
    },
    Text {
        text: String,
    },
}

/// Simple percent-encoding for query string parameter
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// Parses raw Wi-Fi payload formatted as `WIFI:T:WPA;S:MyNetwork;P:MyPassword;H:false;;`
fn parse_wifi_payload(payload: &str) -> Option<QrActionKind> {
    let lower = payload.to_ascii_lowercase();
    if !lower.starts_with("wifi:") {
        return None;
    }

    let body = &payload[5..];
    let mut ssid = String::new();
    let mut password = String::new();
    let mut auth_type = "WPA/WPA2".to_string();
    let mut hidden = false;

    // Split by non-escaped semicolons
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut escape = false;

    for ch in body.chars() {
        if escape {
            current.push(ch);
            escape = false;
        } else if ch == '\\' {
            escape = true;
        } else if ch == ';' {
            tokens.push(current.clone());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    for token in tokens {
        let t = token.trim();
        if t.is_empty() {
            continue;
        }
        if t.starts_with("S:") || t.starts_with("s:") {
            ssid = t[2..].to_string();
        } else if t.starts_with("P:") || t.starts_with("p:") {
            password = t[2..].to_string();
        } else if t.starts_with("T:") || t.starts_with("t:") {
            let val = t[2..].to_uppercase();
            auth_type = match val.as_str() {
                "WPA" | "WPA2" | "WPA/WPA2" => "WPA/WPA2".to_string(),
                "WEP" => "WEP".to_string(),
                "NOPASS" => "None".to_string(),
                _ => val,
            };
        } else if t.starts_with("H:") || t.starts_with("h:") {
            hidden = t[2..].eq_ignore_ascii_case("true") || &t[2..] == "1";
        }
    }

    if !ssid.is_empty() || !password.is_empty() {
        Some(QrActionKind::Wifi {
            ssid,
            password,
            auth_type,
            hidden,
        })
    } else {
        None
    }
}

/// Friendly barcode format display names
fn pretty_barcode_format(format_str: &str) -> String {
    let upper = format_str.to_ascii_uppercase().replace('_', "");
    match upper.as_str() {
        "EAN13" => "EAN-13".to_string(),
        "EAN8" => "EAN-8".to_string(),
        "CODE128" => "Code 128".to_string(),
        "CODE39" => "Code 39".to_string(),
        "CODE93" => "Code 93".to_string(),
        "UPCA" => "UPC-A".to_string(),
        "UPCE" => "UPC-E".to_string(),
        "ITF" => "ITF (Interleaved 2 of 5)".to_string(),
        "CODABAR" => "Codabar".to_string(),
        "DATAMATRIX" => "Data Matrix".to_string(),
        "PDF417" => "PDF-417".to_string(),
        "AZTEC" => "Aztec".to_string(),
        _ => format_str.to_string(),
    }
}

/// Checks if a string represents an online URL
fn is_url_content(content: &str) -> bool {
    let s = content.trim();
    if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("ftp://") {
        return true;
    }
    if s.starts_with("www.") && s.len() > 6 && s.contains('.') {
        return true;
    }
    false
}

/// Checks if format is a 1D barcode or numeric product barcode
fn is_barcode_type(format: Option<&str>, content: &str) -> bool {
    if let Some(f) = format {
        let clean = f.to_ascii_uppercase().replace(['_', '-'], "");
        if clean.contains("EAN")
            || clean.contains("UPC")
            || clean.contains("CODE128")
            || clean.contains("CODE39")
            || clean.contains("CODE93")
            || clean.contains("ITF")
            || clean.contains("CODABAR")
            || clean.contains("DATAMATRIX")
            || clean.contains("PDF417")
            || clean.contains("AZTEC")
        {
            return true;
        }
    }

    // Digit-only sequence typical for barcodes (GTIN-8, UPC-12, EAN-13, ITF-14)
    let s = content.trim();
    let is_numeric = !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    if is_numeric && (s.len() == 8 || s.len() == 12 || s.len() == 13 || s.len() == 14) {
        if let Some(f) = format {
            if !f.to_ascii_uppercase().contains("QR") {
                return true;
            }
        } else {
            return true;
        }
    }

    false
}

/// Analyzes a scanned code result and determines actionable categories
pub fn parse_qr_action(result: &QrScanResult) -> QrActionKind {
    let trimmed = result.content.trim();

    // 1. Wi-Fi QR Code
    if let Some(wifi) = parse_wifi_payload(trimmed) {
        return wifi;
    }

    // 2. Web URL
    if is_url_content(trimmed) {
        let final_url = if trimmed.starts_with("www.") {
            format!("https://{}", trimmed)
        } else {
            trimmed.to_string()
        };
        return QrActionKind::Url { url: final_url };
    }

    // 3. Barcode (EAN-13, Code 128, etc.)
    if is_barcode_type(result.format.as_deref(), trimmed) {
        let fmt_raw = result.format.as_deref().unwrap_or("Barcode");
        let format_name = pretty_barcode_format(fmt_raw);
        let search_url = format!("https://www.google.com/search?q={}", percent_encode(trimmed));
        return QrActionKind::Barcode {
            code: trimmed.to_string(),
            format_name,
            search_url,
        };
    }

    // 4. Plain Text fallback
    QrActionKind::Text {
        text: trimmed.to_string(),
    }
}

/// Opens a web link in the user's default web browser
pub fn open_url_in_browser(url: &str) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let _ = ShellExecuteW(
                None,
                windows::core::w!("open"),
                windows::core::PCWSTR(wide.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            );
        }
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}
