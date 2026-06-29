use crate::app::Theme;
use crate::data::schema::PillColor;
use ratatui::style::Color;

/// Resolve a select value's `PillColor` to a concrete terminal color. Kept off
/// the per-theme structs because the pill palette is shared across themes (the
/// meaning — green = done, red = critical — should read the same everywhere).
pub fn pill_color(p: PillColor) -> Color {
    match p {
        PillColor::Grey => Color::Indexed(245),   // #8a8a8a
        PillColor::Blue => Color::Indexed(111),    // #87afff
        PillColor::Green => Color::Indexed(114),   // #87d75f
        PillColor::Yellow => Color::Indexed(222),  // #ffd787
        PillColor::Orange => Color::Indexed(215),  // #ffaf5f
        PillColor::Red => Color::Indexed(210),     // #ff8787
        PillColor::Purple => Color::Indexed(141),  // #af87ff
        PillColor::Pink => Color::Indexed(212),    // #ff87d7
        PillColor::Cyan => Color::Indexed(116),    // #87d7d7
    }
}

// Uses Color::Indexed (256-color palette) for universal terminal compatibility.
//
// 256-color palette reference:
//   232-255: Grayscale ramp (fixed, not affected by terminal themes)
//     232=#080808, 233=#121212, 234=#1c1c1c, 235=#262626, 236=#303030,
//     237=#3a3a3a, 238=#444444, 239=#4e4e4e, 240=#585858, 241=#626262,
//     242=#6c6c6c, 243=#767676, 244=#808080, 245=#8a8a8a, 246=#949494,
//     247=#9e9e9e, 248=#a8a8a8, 249=#b2b2b2, 250=#bcbcbc, 251=#c6c6c6,
//     252=#d0d0d0, 253=#dadada, 254=#e4e4e4, 255=#eeeeee
//   16-231: 6x6x6 color cube (fixed) — formula: 16 + 36*r + 6*g + b

pub struct ThemeColors {
    pub bg: Color,
    pub fg: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent2: Color,
    pub border: Color,
    pub header_bg: Color,
    pub header_fg: Color,
    pub row_alt: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub cursor_bg: Color,
    pub cursor_fg: Color,
    pub search_match: Color,
    pub error: Color,
    pub success: Color,
    pub warning: Color,
    pub yellow: Color,
    pub green: Color,
    pub cyan: Color,
    pub purple: Color,
    pub pink: Color,
}

pub fn get_theme_colors(theme: &Theme) -> ThemeColors {
    match theme {
        Theme::Dracula => ThemeColors {
            bg: Color::Indexed(235),            // #262626
            fg: Color::Indexed(253),            // #dadada
            muted: Color::Indexed(60),          // #5f5f87 (blue-gray)
            accent: Color::Indexed(141),        // #af87ff (purple)
            accent2: Color::Indexed(117),       // #87d7ff (cyan)
            border: Color::Indexed(238),        // #444444
            header_bg: Color::Indexed(237),     // #3a3a3a
            header_fg: Color::Indexed(141),     // #af87ff (purple)
            row_alt: Color::Indexed(236),       // #303030
            selection_bg: Color::Indexed(238),  // #444444
            selection_fg: Color::Indexed(255),  // #eeeeee
            cursor_bg: Color::Indexed(141),     // #af87ff (purple)
            cursor_fg: Color::Indexed(235),     // #262626
            search_match: Color::Indexed(215),  // #ffaf5f (orange)
            error: Color::Indexed(203),         // #ff5f5f
            success: Color::Indexed(84),        // #5fff87
            warning: Color::Indexed(215),       // #ffaf5f
            yellow: Color::Indexed(228),        // #ffff87
            green: Color::Indexed(84),          // #5fff87
            cyan: Color::Indexed(117),          // #87d7ff
            purple: Color::Indexed(141),        // #af87ff
            pink: Color::Indexed(212),          // #ff87d7
        },
        Theme::Nord => ThemeColors {
            bg: Color::Indexed(235),
            fg: Color::Indexed(252),            // #d0d0d0
            muted: Color::Indexed(60),          // #5f5f87
            accent: Color::Indexed(110),        // #87afd7 (frost blue)
            accent2: Color::Indexed(67),        // #5f87af
            border: Color::Indexed(237),
            header_bg: Color::Indexed(237),
            header_fg: Color::Indexed(110),
            row_alt: Color::Indexed(236),
            selection_bg: Color::Indexed(238),
            selection_fg: Color::Indexed(255),
            cursor_bg: Color::Indexed(110),
            cursor_fg: Color::Indexed(235),
            search_match: Color::Indexed(222),  // #ffd787
            error: Color::Indexed(131),         // #af5f5f
            success: Color::Indexed(108),       // #87af87
            warning: Color::Indexed(222),
            yellow: Color::Indexed(222),        // #ffd787
            green: Color::Indexed(108),         // #87af87
            cyan: Color::Indexed(110),          // #87afd7
            purple: Color::Indexed(139),        // #af87af
            pink: Color::Indexed(139),
        },
        Theme::Catppuccin => ThemeColors {
            bg: Color::Indexed(234),            // #1c1c1c
            fg: Color::Indexed(252),
            muted: Color::Indexed(243),         // #767676
            accent: Color::Indexed(111),        // #87afff (blue)
            accent2: Color::Indexed(147),       // #afafff (lavender)
            border: Color::Indexed(237),
            header_bg: Color::Indexed(236),
            header_fg: Color::Indexed(111),
            row_alt: Color::Indexed(235),
            selection_bg: Color::Indexed(237),
            selection_fg: Color::Indexed(255),
            cursor_bg: Color::Indexed(111),
            cursor_fg: Color::Indexed(234),
            search_match: Color::Indexed(223),  // #ffd7af (peach)
            error: Color::Indexed(211),         // #ff87af
            success: Color::Indexed(114),       // #87d75f
            warning: Color::Indexed(223),
            yellow: Color::Indexed(223),
            green: Color::Indexed(114),
            cyan: Color::Indexed(116),          // #87d7d7 (teal)
            purple: Color::Indexed(177),        // #d787ff
            pink: Color::Indexed(218),          // #ffafd7
        },
        Theme::TokyoNight => ThemeColors {
            bg: Color::Indexed(234),
            fg: Color::Indexed(252),
            muted: Color::Indexed(60),
            accent: Color::Indexed(73),         // #5fafaf (teal)
            accent2: Color::Indexed(141),       // #af87ff (purple)
            border: Color::Indexed(236),
            header_bg: Color::Indexed(236),
            header_fg: Color::Indexed(73),
            row_alt: Color::Indexed(235),
            selection_bg: Color::Indexed(237),
            selection_fg: Color::Indexed(255),
            cursor_bg: Color::Indexed(73),
            cursor_fg: Color::Indexed(234),
            search_match: Color::Indexed(179),  // #d7af5f
            error: Color::Indexed(204),         // #ff5f87
            success: Color::Indexed(114),       // #87d75f
            warning: Color::Indexed(179),
            yellow: Color::Indexed(179),
            green: Color::Indexed(114),
            cyan: Color::Indexed(117),          // #87d7ff
            purple: Color::Indexed(141),
            pink: Color::Indexed(204),
        },
        Theme::Solarized => ThemeColors {
            bg: Color::Indexed(234),
            fg: Color::Indexed(247),            // #9e9e9e
            muted: Color::Indexed(242),         // #6c6c6c
            accent: Color::Indexed(33),         // #0087ff (blue)
            accent2: Color::Indexed(37),        // #00afaf (cyan)
            border: Color::Indexed(236),
            header_bg: Color::Indexed(236),
            header_fg: Color::Indexed(33),
            row_alt: Color::Indexed(235),
            selection_bg: Color::Indexed(237),
            selection_fg: Color::Indexed(254),  // #e4e4e4
            cursor_bg: Color::Indexed(33),
            cursor_fg: Color::Indexed(254),
            search_match: Color::Indexed(136),  // #af8700 (yellow)
            error: Color::Indexed(160),         // #d70000
            success: Color::Indexed(64),        // #5f8700
            warning: Color::Indexed(136),
            yellow: Color::Indexed(136),
            green: Color::Indexed(64),
            cyan: Color::Indexed(37),
            purple: Color::Indexed(61),         // #5f5faf
            pink: Color::Indexed(125),          // #af005f
        },
    }
}
