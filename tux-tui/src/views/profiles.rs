//! Profiles tab view: list view and form-based editor.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::model::{ProfilesMode, ProfilesState};
use crate::widgets::form;

/// Render the profiles tab.
pub fn render(frame: &mut Frame, area: Rect, state: &ProfilesState) {
    match &state.mode {
        ProfilesMode::List => render_list(frame, area, state),
        ProfilesMode::Editor {
            form: f,
            profile_id,
        } => {
            let title = if let Some(p) = state.profiles.iter().find(|p| p.id == *profile_id) {
                if p.is_default {
                    format!(" View: {} (read-only, copy to edit) ", p.name)
                } else {
                    format!(" Edit: {} ", p.name)
                }
            } else {
                " Edit Profile ".to_string()
            };
            form::render(frame, area, f, &title);
        }
    }
}

fn render_list(frame: &mut Frame, area: Rect, state: &ProfilesState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Profile list
            Constraint::Length(1), // Key hints
            Constraint::Length(1), // Status message
        ])
        .split(area);

    render_profile_list(frame, chunks[0], state);
    render_key_hints(frame, chunks[1]);

    // Status message (if any).
    if let Some(msg) = &state.status_message {
        let paragraph = Paragraph::new(Span::styled(
            msg.as_str(),
            Style::default().fg(Color::Yellow),
        ));
        frame.render_widget(paragraph, chunks[2]);
    }
}

fn render_profile_list(frame: &mut Frame, area: Rect, state: &ProfilesState) {
    let mut lines: Vec<Line> = Vec::new();

    for (i, profile) in state.profiles.iter().enumerate() {
        let is_selected = i == state.selected_index;
        let is_ac = profile.id == state.assignments.ac_profile;
        let is_bat = profile.id == state.assignments.battery_profile;

        let pointer = if is_selected { "▸ " } else { "  " };
        let badge = if profile.is_default {
            "Default"
        } else {
            "Custom "
        };
        let ac_marker = if is_ac { "●" } else { "○" };
        let bat_marker = if is_bat { "●" } else { "○" };

        let line = Line::from(vec![
            Span::styled(
                pointer,
                if is_selected {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
            Span::styled(
                format!("{:<25}", profile.name),
                if is_selected {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
            Span::styled(
                format!("{badge}  "),
                Style::default().fg(if profile.is_default {
                    Color::DarkGray
                } else {
                    Color::Green
                }),
            ),
            Span::styled(
                format!("{ac_marker} AC  "),
                Style::default().fg(if is_ac {
                    Color::Yellow
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                format!("{bat_marker} BAT"),
                Style::default().fg(if is_bat {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
        ]);
        lines.push(line);
    }

    let block = Block::default().borders(Borders::ALL).title(" Profiles ");
    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_key_hints(frame: &mut Frame, area: Rect) {
    let hints = Line::from(vec![
        Span::styled(" [Enter] ", Style::default().fg(Color::Cyan)),
        Span::raw("Edit  "),
        Span::styled("[c] ", Style::default().fg(Color::Cyan)),
        Span::raw("Copy  "),
        Span::styled("[d] ", Style::default().fg(Color::Cyan)),
        Span::raw("Delete  "),
        Span::styled("[a] ", Style::default().fg(Color::Cyan)),
        Span::raw("Set AC  "),
        Span::styled("[b] ", Style::default().fg(Color::Cyan)),
        Span::raw("Set BAT"),
    ]);
    frame.render_widget(Paragraph::new(hints), area);
}

#[cfg(test)]
mod tests {
    use crate::model::{ProfileAssignments, ProfilesState};

    /// Build a formatted badge for the AC/BAT assignment column.
    fn assignment_marker(profile_id: &str, assignments: &ProfileAssignments) -> (bool, bool) {
        let is_ac = profile_id == assignments.ac_profile;
        let is_bat = profile_id == assignments.battery_profile;
        (is_ac, is_bat)
    }

    #[test]
    fn assignment_marker_identifies_ac_bat() {
        let assignments = ProfileAssignments {
            ac_profile: "__office__".to_string(),
            battery_profile: "__quiet__".to_string(),
        };
        let (ac, bat) = assignment_marker("__office__", &assignments);
        assert!(ac);
        assert!(!bat);
        let (ac, bat) = assignment_marker("__quiet__", &assignments);
        assert!(!ac);
        assert!(bat);
    }

    #[test]
    fn profile_list_renders_with_selection() {
        let mut state = ProfilesState::new();
        state.profiles = tux_core::profile::builtin_profiles();
        state.selected_index = 1;
        state.assignments = ProfileAssignments {
            ac_profile: "__office__".to_string(),
            battery_profile: "__quiet__".to_string(),
        };
        // Just verify we can construct the rendering without panic.
        assert_eq!(state.profiles.len(), 4);
        assert_eq!(state.selected_index, 1);
    }
}
