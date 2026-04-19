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
    let has_d = !state.dgpu_name.is_empty();
    let has_i = !state.igpu_name.is_empty();

    match (has_d, has_i) {
        (true, true) => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(area);
            frame.render_widget(build_dgpu_paragraph(state), chunks[0]);
            frame.render_widget(build_igpu_paragraph(state), chunks[1]);
        }
        (true, false) => {
            frame.render_widget(build_dgpu_paragraph(state), area);
        }
        (false, true) => {
            frame.render_widget(build_igpu_paragraph(state), area);
        }
        (false, false) => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(area);
            frame.render_widget(build_dgpu_paragraph(state), chunks[0]);
            frame.render_widget(build_igpu_paragraph(state), chunks[1]);
        }
    }
}

fn build_dgpu_paragraph(state: &PowerState) -> Paragraph<'_> {
    let dgpu_name = if state.dgpu_name.is_empty() {
        "No dGPU detected"
    } else {
        &state.dgpu_name
    };
    let mut lines = vec![Line::from(Span::styled(
        dgpu_name,
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    if let Some(temp) = state.dgpu_temp {
        lines.push(Line::from(format!("Temp: {temp:.0}°C")));
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
        lines.push(Line::from(usage_power));
    }
    Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" dGPU "))
}

fn build_igpu_paragraph(state: &PowerState) -> Paragraph<'_> {
    let igpu_name = if state.igpu_name.is_empty() {
        "No iGPU detected"
    } else {
        &state.igpu_name
    };
    let mut lines = vec![Line::from(Span::styled(
        igpu_name,
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    if let Some(usage) = state.igpu_usage {
        lines.push(Line::from(format!("Usage: {usage}%")));
    }
    Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" iGPU "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn power_state_renders_without_gpu_data() {
        let state = PowerState::new();
        assert!(state.dgpu_name.is_empty());
        assert!(state.igpu_name.is_empty());
        // Fresh PowerState starts unsupported with the tgp_offset field
        // disabled; update.rs flips both on when the daemon reports
        // gpu_control.
        assert!(!state.form_tab.supported);
        let tgp = state
            .form_tab
            .form
            .fields
            .iter()
            .find(|f| f.key == "tgp_offset")
            .expect("tgp_offset field should always exist in the model");
        assert!(!tgp.enabled);
    }

    #[test]
    fn power_view_collapses_dgpu_panel_when_apu_only() {
        let mut state = PowerState::new();
        state.form_tab.supported = true;
        state.igpu_name = "amdgpu".into();

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, f.area(), &state)).unwrap();

        let buf = terminal.backend().buffer().clone();
        let rendered: String = buf
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<Vec<_>>()
            .join("");
        assert!(
            !rendered.contains("No dGPU detected"),
            "dGPU panel should not render on APU-only layout, got: {rendered}"
        );
        assert!(
            rendered.contains("amdgpu"),
            "iGPU name should appear in full-width layout, got: {rendered}"
        );
    }

    #[test]
    fn power_view_collapses_igpu_panel_when_dgpu_only() {
        let mut state = PowerState::new();
        state.form_tab.supported = true;
        state.dgpu_name = "RTX 4060".into();

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, f.area(), &state)).unwrap();

        let buf = terminal.backend().buffer().clone();
        let rendered: String = buf
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<Vec<_>>()
            .join("");
        assert!(
            !rendered.contains("No iGPU detected"),
            "iGPU panel should not render on dGPU-only layout, got: {rendered}"
        );
        assert!(
            rendered.contains("RTX 4060"),
            "dGPU name should appear in full-width layout, got: {rendered}"
        );
    }
}
