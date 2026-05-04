//! Theme and font registration for the desktop UI.

pub(crate) mod font;

use crate::theme::font::{FontPair, FontRegistry};
use gpui::{App, SharedString};
use gpui_component::{Theme, ThemeConfig, ThemeRegistry};
use std::rc::Rc;

pub(crate) fn init(cx: &mut App) {
    font::init_fonts(cx);
    apply_theme(cx, "Default Dark", "geist");
}

fn apply_theme(cx: &mut App, theme_name: &str, font_pair: &str) -> bool {
    if let Some(theme_config) = ThemeRegistry::global(cx).themes().get(theme_name).cloned() {
        let mut theme_config = (*theme_config).clone();
        theme_config = overwrite_colors(&mut theme_config, font_pair);
        let theme_config_rc = Rc::new(theme_config);
        Theme::global_mut(cx).apply_config(&theme_config_rc);

        return true;
    }
    false
}

fn overwrite_colors(theme: &mut ThemeConfig, font_pair: &str) -> ThemeConfig {
    let default_font = FontPair::geist();
    let font = FontRegistry::get(font_pair).unwrap_or(&default_font);

    theme.colors.list_active = Some(theme.colors.switch.clone().unwrap_or_default());
    theme.colors.list_active_border =
        Some(theme.colors.primary_foreground.clone().unwrap_or_default());

    theme.colors.selection = Some(SharedString::from(format!(
        "{}50",
        theme.colors.primary.clone().unwrap_or_default()
    )));

    theme.font_family = Some(SharedString::from(font.ui));
    theme.mono_font_family = Some(SharedString::from(font.mono));

    theme.clone()
}
