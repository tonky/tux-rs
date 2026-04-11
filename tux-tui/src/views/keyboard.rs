//! Keyboard tab view: backlight controls.

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::model::FormTabState;

/// Render the keyboard tab.
pub fn render(frame: &mut Frame, area: Rect, state: &FormTabState) {
    super::render_form_tab(
        frame,
        area,
        state,
        " Keyboard ",
        "Keyboard backlight not available on this device",
    );
}
