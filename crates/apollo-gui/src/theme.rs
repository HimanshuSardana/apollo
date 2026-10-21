//! Theme setup. Apollo ships dark by default.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

pub fn init(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
}
