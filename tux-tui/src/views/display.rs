//! Display tab view: brightness controls.

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::model::FormTabState;

/// Render the display tab.
pub fn render(frame: &mut Frame, area: Rect, state: &FormTabState) {
    super::render_form_tab(
        frame,
        area,
        state,
        " Display ",
        "No display backlight controller found",
    );
}
