use crossterm::style::Color;
use held_core::view::colors::Colors;
use syntect::highlighting::Theme;

use super::to_rgb;

pub trait ColorMap {
    fn map_colors(&self, colors: Colors) -> Colors;
}

impl ColorMap for Theme {
    fn map_colors(&self, colors: Colors) -> Colors {
        let fg = self.settings.foreground.map(to_rgb).unwrap_or(Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        });

        let bg = self
            .settings
            .background
            .map(to_rgb)
            .unwrap_or(Color::Rgb { r: 0, g: 0, b: 0 });

        let alt_bg = self
            .settings
            .line_highlight
            .map(to_rgb)
            .unwrap_or(Color::Rgb {
                r: 55,
                g: 55,
                b: 55,
            });

        match colors {
            Colors::Default => Colors::CustomForeground(fg),
            Colors::Focused => Colors::Custom(fg, alt_bg),
            Colors::Inverted => Colors::Custom(bg, fg),
            Colors::Insert => Colors::Custom(
                Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                },
                Color::Rgb { r: 0, g: 180, b: 0 },
            ),
            Colors::Warning => Colors::Custom(
                Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                },
                Color::Rgb {
                    r: 240,
                    g: 140,
                    b: 20,
                },
            ),
            Colors::PathMode => Colors::Custom(
                Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                },
                Color::Rgb {
                    r: 255,
                    g: 20,
                    b: 137,
                },
            ),
            Colors::SearchMode => Colors::Custom(
                Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                },
                Color::Rgb {
                    r: 120,
                    g: 0,
                    b: 120,
                },
            ),
            Colors::SelectMode => Colors::Custom(
                Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                },
                Color::Rgb {
                    r: 0,
                    g: 120,
                    b: 160,
                },
            ),
            Colors::RegionEdge => Colors::Custom(
                Color::Rgb { r: 0, g: 0, b: 0 },
                Color::Rgb {
                    r: 105,
                    g: 105,
                    b: 105,
                },
            ),
            Colors::FocusedRegionEdge => Colors::Custom(
                Color::Rgb { r: 0, g: 0, b: 0 },
                Color::Rgb {
                    r: 190,
                    g: 190,
                    b: 190,
                },
            ),
            Colors::CustomForeground(custom_fg) => Colors::CustomForeground(custom_fg),
            Colors::CustomFocusedForeground(custom_fg) => Colors::Custom(custom_fg, alt_bg),
            Colors::Custom(custom_fg, custom_bg) => Colors::Custom(custom_fg, custom_bg),
        }
    }
}
