//! Webcam tab view: per-device camera controls with device selector.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::model::WebcamState;
use crate::widgets::form;

/// Render the webcam tab.
pub fn render(frame: &mut Frame, area: Rect, state: &WebcamState) {
    if !state.form_tab.supported {
        let paragraph = Paragraph::new("Webcam controls are unavailable in the TUI.\nUse a GUI application for webcam management.")
            .block(Block::default().borders(Borders::ALL).title(" Webcam "))
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Device selector bar
            Constraint::Min(0),    // Controls form
            Constraint::Length(1), // Key hints
        ])
        .split(area);

    render_device_selector(frame, chunks[0], state);
    form::render(frame, chunks[1], &state.form_tab.form, " Webcam Controls ");
    render_key_hints(frame, chunks[2]);
}

fn render_device_selector(frame: &mut Frame, area: Rect, state: &WebcamState) {
    let mut spans: Vec<Span> = vec![Span::raw(" Devices: ")];
    for (i, name) in state.devices.iter().enumerate() {
        let is_selected = i == state.selected_device;
        let style = if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let bracket = if is_selected { "[" } else { " " };
        let bracket_end = if is_selected { "]" } else { " " };
        spans.push(Span::styled(
            format!("{bracket}{name}{bracket_end} "),
            style,
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_key_hints(frame: &mut Frame, area: Rect) {
    let hints = Line::from(vec![
        Span::styled(" [s] ", Style::default().fg(Color::Cyan)),
        Span::raw("Save  "),
        Span::styled("[Shift+←][Shift+→] ", Style::default().fg(Color::Cyan)),
        Span::raw("Switch device  "),
        Span::styled("[Esc] ", Style::default().fg(Color::Cyan)),
        Span::raw("Discard"),
    ]);
    frame.render_widget(Paragraph::new(hints), area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webcam_device_switching() {
        let mut state = WebcamState::new();
        state.devices = vec!["Camera 1".into(), "Camera 2".into(), "Camera 3".into()];
        assert_eq!(state.selected_device, 0);

        state.select_next_device();
        assert_eq!(state.selected_device, 1);

        state.select_next_device();
        assert_eq!(state.selected_device, 2);

        state.select_next_device();
        assert_eq!(state.selected_device, 0); // Wraps.

        state.select_prev_device();
        assert_eq!(state.selected_device, 2); // Wraps back.
    }

    #[test]
    fn webcam_empty_devices_safe() {
        let mut state = WebcamState::new();
        state.devices = vec![];
        state.select_next_device(); // Should not panic.
        state.select_prev_device(); // Should not panic.
        assert_eq!(state.selected_device, 0);
    }
}
