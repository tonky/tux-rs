//! View layer: renders the Model into terminal frames.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Tabs};

use crate::model::{ConnectionStatus, Model, Tab};

/// Render the full UI from the current model state.
pub fn render(frame: &mut Frame, model: &Model) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tab bar
            Constraint::Min(0),    // Tab content
            Constraint::Length(1), // Status bar
        ])
        .split(frame.area());

    render_tab_bar(frame, chunks[0], model);
    render_tab_content(frame, chunks[1], model);
    render_status_bar(frame, chunks[2], model);

    if model.show_help {
        render_help_overlay(frame, frame.area());
    }
}

fn render_tab_bar(frame: &mut Frame, area: Rect, model: &Model) {
    let titles: Vec<Line> = Tab::ALL.iter().map(|t| Line::from(t.label())).collect();
    let selected = Tab::ALL
        .iter()
        .position(|&t| t == model.current_tab)
        .unwrap_or(0);

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::BOTTOM).title("tux-tui"))
        .select(selected)
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tabs, area);
}

fn render_tab_content(frame: &mut Frame, area: Rect, model: &Model) {
    match model.current_tab {
        Tab::Dashboard => crate::views::dashboard::render(frame, area, &model.dashboard),
        Tab::FanCurve => crate::views::fan_curve::render(frame, area, &model.fan_curve),
        Tab::Info => crate::views::info::render(frame, area, &model.info),
        Tab::EventLog => crate::views::event_log::render(frame, area, &model.event_log),
        Tab::Profiles => crate::views::profiles::render(frame, area, &model.profiles),
        Tab::Settings => crate::views::settings::render(frame, area, &model.settings),
        Tab::Keyboard => crate::views::keyboard::render(frame, area, &model.keyboard),
        Tab::Charging => crate::views::charging::render(frame, area, &model.charging),
        Tab::Power => crate::views::power::render(frame, area, &model.power),
        Tab::Display => crate::views::display::render(frame, area, &model.display),
        Tab::Webcam => crate::views::webcam::render(frame, area, &model.webcam),
    }
}

fn render_status_bar(frame: &mut Frame, area: Rect, model: &Model) {
    let (status_text, status_color) = match model.connection_status {
        ConnectionStatus::Connected => ("● Connected", Color::Green),
        ConnectionStatus::Disconnected => ("● Disconnected", Color::Red),
        ConnectionStatus::Connecting => ("● Connecting…", Color::Yellow),
    };

    let spans = Line::from(vec![
        Span::styled(status_text, Style::default().fg(status_color)),
        Span::raw("  "),
        Span::styled("? Help", Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled("l Event Log", Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled(
            "D Toggle Debug Filter",
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw("  "),
        Span::styled("q Quit", Style::default().fg(Color::DarkGray)),
    ]);

    frame.render_widget(Paragraph::new(spans), area);
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Key Bindings",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  1–9, 0    Switch to tab"),
        Line::from("  Tab       Next tab"),
        Line::from("  Shift+Tab Previous tab"),
        Line::from("  l         Open Event Log"),
        Line::from("  D         Toggle debug log filter"),
        Line::from("  ?         Toggle this help"),
        Line::from("  q         Quit"),
        Line::from(""),
    ];

    // Center a popup
    let popup = centered_rect(40, help_text.len() as u16 + 2, area);
    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::DarkGray));
    let paragraph = Paragraph::new(help_text).block(block);

    frame.render_widget(Clear, popup);
    frame.render_widget(paragraph, popup);
}

/// Create a centered rectangle within `area`.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn centered_rect_within_bounds() {
        let area = Rect::new(0, 0, 80, 24);
        let popup = centered_rect(40, 10, area);
        assert_eq!(popup.x, 20);
        assert_eq!(popup.y, 7);
        assert_eq!(popup.width, 40);
        assert_eq!(popup.height, 10);
    }

    #[test]
    fn centered_rect_clamps_to_area() {
        let area = Rect::new(0, 0, 20, 10);
        let popup = centered_rect(40, 20, area);
        assert_eq!(popup.width, 20);
        assert_eq!(popup.height, 10);
    }

    /// Render every tab without panicking.
    #[test]
    fn render_all_tabs_headless() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut model = Model::new();

        for tab in Tab::ALL {
            model.current_tab = tab;
            terminal.draw(|frame| render(frame, &model)).unwrap();
        }
    }

    /// Render at minimum terminal size without panicking.
    #[test]
    fn render_minimum_size() {
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        let model = Model::new();
        terminal.draw(|frame| render(frame, &model)).unwrap();
    }

    /// Render with help overlay visible.
    #[test]
    fn render_help_overlay() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut model = Model::new();
        model.show_help = true;
        terminal.draw(|frame| render(frame, &model)).unwrap();
    }
}
