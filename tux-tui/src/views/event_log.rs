//! Event Log tab: recent meaningful UI/daemon events.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::model::{EventLogState, EventSource};

pub fn render(frame: &mut Frame, area: Rect, state: &EventLogState) {
    let title = if state.show_debug_events {
        "Event Log (debug: all, newest first)"
    } else {
        "Event Log (debug: filtered, newest first)"
    };

    let visible_entries: Vec<_> = state
        .entries
        .iter()
        .filter(|e| state.show_debug_events || !e.debug)
        .collect();

    if visible_entries.is_empty() {
        let msg = if state.entries.is_empty() {
            "No events yet"
        } else {
            "No visible events (press D to show debug events)"
        };
        let paragraph =
            Paragraph::new(msg).block(Block::default().borders(Borders::ALL).title(title));
        frame.render_widget(paragraph, area);
        return;
    }

    // Render newest entries first so recent actions are visible immediately.
    let items: Vec<ListItem> = visible_entries
        .into_iter()
        .rev()
        .map(|e| ListItem::new(Line::from(format_event_line(e))))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(Color::White));

    frame.render_widget(list, area);
}

fn format_event_line(e: &crate::model::EventLogEntry) -> String {
    let src = match e.source {
        EventSource::User => "USER",
        EventSource::Daemon => "DAEMON",
        EventSource::System => "SYSTEM",
    };
    let ts = (e.ts_unix_ms / 1000) % 86_400;
    let h = ts / 3600;
    let m = (ts % 3600) / 60;
    let s = ts % 60;
    if let Some(detail) = &e.detail {
        format!("[{h:02}:{m:02}:{s:02}] {src:7} {} | {detail}", e.summary)
    } else {
        format!("[{h:02}:{m:02}:{s:02}] {src:7} {}", e.summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EventLogState;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn render_event_log_tab_does_not_panic() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = EventLogState::new();
        state.push(EventSource::System, "startup", None);

        terminal
            .draw(|frame| render(frame, frame.area(), &state))
            .unwrap();
    }

    #[test]
    fn render_event_log_with_debug_filter_toggle_does_not_panic() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = EventLogState::new();
        state.push(EventSource::System, "startup", None);
        state.push_debug(
            EventSource::Daemon,
            "debug telemetry",
            Some("fan1 duty 84/255".to_string()),
        );

        terminal
            .draw(|frame| render(frame, frame.area(), &state))
            .unwrap();

        state.toggle_debug_filter();
        terminal
            .draw(|frame| render(frame, frame.area(), &state))
            .unwrap();
    }

    #[test]
    fn newest_events_render_first() {
        let mut state = EventLogState::new();
        state.push(EventSource::System, "older", None);
        state.push(EventSource::System, "newer", None);

        let visible_entries: Vec<_> = state
            .entries
            .iter()
            .filter(|e| state.show_debug_events || !e.debug)
            .collect();
        let rendered: Vec<String> = visible_entries
            .into_iter()
            .rev()
            .map(format_event_line)
            .collect();

        assert!(rendered.first().map(|l| l.contains("newer")).unwrap_or(false));
    }
}
