//! Per-tab view modules.

pub mod charging;
pub mod dashboard;
pub mod display;
pub mod fan_curve;
pub mod info;
pub mod keyboard;
pub mod power;
pub mod profiles;
pub mod settings;
pub mod webcam;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::model::FormTabState;
use crate::widgets::form;

/// Render a form-backed tab with unsupported fallback and status message.
pub fn render_form_tab(
    frame: &mut Frame,
    area: Rect,
    state: &FormTabState,
    title: &str,
    unsupported_msg: &str,
) {
    if !state.supported {
        let paragraph = Paragraph::new(unsupported_msg)
            .block(Block::default().borders(Borders::ALL).title(title))
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }
    form::render(frame, area, &state.form, title);
    if let Some(msg) = &state.status_message
        && area.height > 1
    {
        let status_area = Rect::new(
            area.x + 1,
            area.y + area.height.saturating_sub(1),
            area.width.saturating_sub(2),
            1,
        );
        frame.render_widget(
            Paragraph::new(Span::styled(
                msg.as_str(),
                Style::default().fg(Color::Yellow),
            )),
            status_area,
        );
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::model::Form;

    fn make_state(supported: bool, status: Option<&str>) -> FormTabState {
        FormTabState {
            form: Form::new(vec![]),
            supported,
            status_message: status.map(|s| s.to_string()),
        }
    }

    #[test]
    fn render_form_tab_unsupported_does_not_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = make_state(false, None);
        terminal
            .draw(|frame| {
                render_form_tab(frame, frame.area(), &state, " Test ", "Not available");
            })
            .unwrap();
    }

    #[test]
    fn render_form_tab_supported_does_not_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = make_state(true, Some("Saved!"));
        terminal
            .draw(|frame| {
                render_form_tab(frame, frame.area(), &state, " Test ", "Not available");
            })
            .unwrap();
    }
}
