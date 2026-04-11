//! Charging tab view: battery thresholds and charging profiles.

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::model::FormTabState;

/// Render the charging tab.
pub fn render(frame: &mut Frame, area: Rect, state: &FormTabState) {
    super::render_form_tab(
        frame,
        area,
        state,
        " Charging ",
        "Charging control not available on this device",
    );
}
