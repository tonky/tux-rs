//! Settings tab view: global configuration form.

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::model::FormTabState;

/// Render the settings tab.
pub fn render(frame: &mut Frame, area: Rect, state: &FormTabState) {
    super::render_form_tab(frame, area, state, " Settings ", "Settings not available");
}
