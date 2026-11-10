use gpui::{Rgba, rgb, rgba};

#[derive(Clone, Copy, Debug)]
pub struct PixoraTheme {
    pub bg_dark: Rgba,
    pub bg_surface: Rgba,
    pub bg_secondary: Rgba,
    pub bg_hover: Rgba,
    pub bg_active: Rgba,

    pub border_subtle: Rgba,
    pub border_focus: Rgba,

    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub accent_muted: Rgba,

    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,

    pub mask_overlay: Rgba,
    pub selection_border: Rgba,
    pub guide_line: Rgba,

    pub palette_red: Rgba,
    pub palette_orange: Rgba,
    pub palette_yellow: Rgba,
    pub palette_green: Rgba,
    pub palette_cyan: Rgba,
    pub palette_blue: Rgba,
    pub palette_purple: Rgba,
    pub palette_white: Rgba,
    pub palette_black: Rgba,
}

impl Default for PixoraTheme {
    fn default() -> Self {
        Self {
            bg_dark: rgb(0x0B0B0D),
            bg_surface: rgb(0x151519),
            bg_secondary: rgb(0x1C1C22),
            bg_hover: rgb(0x25252E),
            bg_active: rgb(0x2F2E3D),

            border_subtle: rgb(0x282832),
            border_focus: rgb(0x8174FF),

            accent: rgb(0x8174FF),
            accent_hover: rgb(0x958BFF),
            accent_muted: rgba(0x8174FF40),

            text_primary: rgb(0xF2F2F5),
            text_secondary: rgb(0xA2A2B0),
            text_muted: rgb(0x656575),

            mask_overlay: rgba(0x00000088),
            selection_border: rgb(0x8174FF),
            guide_line: rgba(0x8174FF80),

            palette_red: rgb(0xFF453A),
            palette_orange: rgb(0xFF9F0A),
            palette_yellow: rgb(0xFFD60A),
            palette_green: rgb(0x30D158),
            palette_cyan: rgb(0x64D2FF),
            palette_blue: rgb(0x0A84FF),
            palette_purple: rgb(0xBF5AF2),
            palette_white: rgb(0xFFFFFF),
            palette_black: rgb(0x000000),
        }
    }
}
