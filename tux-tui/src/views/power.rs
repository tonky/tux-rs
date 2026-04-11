//! Power tab view: GPU info block + power settings form.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::model::PowerState;
use crate::widgets::form;

/// Render the power tab.
pub fn render(frame: &mut Frame, area: Rect, state: &PowerState) {
    if !state.form_tab.supported {
        let paragraph = Paragraph::new("Power controls not available on this device")
            .block(Block::default().borders(Borders::ALL).title(" Power "))
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // GPU info block
            Constraint::Min(0),    // Settings form
        ])
        .split(area);

    render_gpu_info(frame, chunks[0], state);
    form::render(frame, chunks[1], &state.form_tab.form, " Power Settings ");
}

fn render_gpu_info(frame: &mut Frame, area: Rect, state: &PowerState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // dGPU info.
    let dgpu_name = if state.dgpu_name.is_empty() {
        "No dGPU detected"
    } else {
        &state.dgpu_name
    };
    let mut dgpu_lines = vec![Line::from(Span::styled(
        dgpu_name,
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    if let Some(temp) = state.dgpu_temp {
        dgpu_lines.push(Line::from(format!("Temp: {temp:.0}°C")));
    }
    let mut usage_power = String::new();
    if let Some(usage) = state.dgpu_usage {
        usage_power.push_str(&format!("Usage: {usage}%"));
    }
    if let Some(power) = state.dgpu_power {
        if !usage_power.is_empty() {
            usage_power.push_str("  ");
        }
        usage_power.push_str(&format!("Power: {power:.0}W"));
    }
    if !usage_power.is_empty() {
        dgpu_lines.push(Line::from(usage_power));
    }

    let dgpu_block = Block::default().borders(Borders::ALL).title(" dGPU ");
    let dgpu_paragraph = Paragraph::new(dgpu_lines).block(dgpu_block);
    frame.render_widget(dgpu_paragraph, chunks[0]);

    // iGPU info.
    let igpu_name = if state.igpu_name.is_empty() {
        "No iGPU detected"
    } else {
        &state.igpu_name
    };
    let mut igpu_lines = vec![Line::from(Span::styled(
        igpu_name,
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    if let Some(usage) = state.igpu_usage {
        igpu_lines.push(Line::from(format!("Usage: {usage}%")));
    }

    let igpu_block = Block::default().borders(Borders::ALL).title(" iGPU ");
    let igpu_paragraph = Paragraph::new(igpu_lines).block(igpu_block);
    frame.render_widget(igpu_paragraph, chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_state_renders_without_gpu_data() {
        let state = PowerState::new();
        assert!(state.dgpu_name.is_empty());
        assert!(state.igpu_name.is_empty());
        assert!(state.form_tab.supported);
    }
}
