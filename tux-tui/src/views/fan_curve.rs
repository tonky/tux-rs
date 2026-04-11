//! Fan curve tab: interactive chart editor with point selection.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph};

use crate::model::FanCurveState;

/// Render the fan curve tab.
pub fn render(frame: &mut Frame, area: Rect, state: &FanCurveState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),   // Chart area
            Constraint::Length(3), // Info + key hints
        ])
        .split(area);

    render_chart(frame, chunks[0], state);
    render_info_bar(frame, chunks[1], state);
}

fn render_chart(frame: &mut Frame, area: Rect, state: &FanCurveState) {
    // Build curve line data (connected path through all points).
    let curve_data: Vec<(f64, f64)> = state
        .points
        .iter()
        .map(|p| (p.temp as f64, p.speed as f64))
        .collect();

    // Point markers share the same data as curve line.
    let point_data = &curve_data;

    // Build selected point highlight.
    let selected_data: Vec<(f64, f64)> = state
        .points
        .get(state.selected_index)
        .map(|p| vec![(p.temp as f64, p.speed as f64)])
        .unwrap_or_default();

    // Build current operating point.
    let current_data: Vec<(f64, f64)> = match (state.current_temp, state.current_speed) {
        (Some(t), Some(s)) => vec![(t as f64, s as f64)],
        _ => vec![],
    };

    let mut datasets = vec![
        Dataset::default()
            .name("Curve")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::White))
            .data(&curve_data),
        Dataset::default()
            .name("Points")
            .marker(Marker::Dot)
            .graph_type(GraphType::Scatter)
            .style(Style::default().fg(Color::Green))
            .data(point_data),
    ];

    if !selected_data.is_empty() {
        datasets.push(
            Dataset::default()
                .name("Selected")
                .marker(Marker::Block)
                .graph_type(GraphType::Scatter)
                .style(
                    Style::default()
                        .fg(Color::LightYellow)
                        .add_modifier(Modifier::BOLD),
                )
                .data(&selected_data),
        );
    }

    if !current_data.is_empty() {
        datasets.push(
            Dataset::default()
                .name("Current")
                .marker(Marker::Dot)
                .graph_type(GraphType::Scatter)
                .style(Style::default().fg(Color::Yellow))
                .data(&current_data),
        );
    }

    let dirty_marker = if state.dirty { " *" } else { "" };
    let title = format!("Fan Curve{dirty_marker}");

    let chart = Chart::new(datasets)
        .block(Block::default().borders(Borders::ALL).title(title))
        .x_axis(
            Axis::default()
                .title("Temperature (°C)")
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, 105.0])
                .labels(
                    ["0", "20", "40", "60", "80", "100"]
                        .map(Span::from)
                        .to_vec(),
                ),
        )
        .y_axis(
            Axis::default()
                .title("Speed (%)")
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, 105.0])
                .labels(["0", "25", "50", "75", "100"].map(Span::from).to_vec()),
        );

    frame.render_widget(chart, area);
}

fn render_info_bar(frame: &mut Frame, area: Rect, state: &FanCurveState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Selected point info.
    let selected_info = if let Some(p) = state.points.get(state.selected_index) {
        format!(
            "◆ Selected: {}°C → {}%  [{}/{}]",
            p.temp,
            p.speed,
            state.selected_index + 1,
            state.points.len()
        )
    } else {
        "No points".to_string()
    };

    let current_info = match (state.current_temp, state.current_speed) {
        (Some(t), Some(s)) => format!("● Current: {t}°C → {s}%"),
        _ => "● Current: —".to_string(),
    };

    let left = Paragraph::new(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(selected_info, Style::default().fg(Color::LightYellow)),
        Span::raw("    "),
        Span::styled(current_info, Style::default().fg(Color::Yellow)),
    ]));

    let right = Paragraph::new(Line::from(vec![
        Span::styled("←→", Style::default().fg(Color::Cyan)),
        Span::raw(" select  "),
        Span::styled("↑↓", Style::default().fg(Color::Cyan)),
        Span::raw(" speed  "),
        Span::styled("i", Style::default().fg(Color::Cyan)),
        Span::raw("nsert  "),
        Span::styled("x", Style::default().fg(Color::Cyan)),
        Span::raw(" del  "),
        Span::styled("r", Style::default().fg(Color::Cyan)),
        Span::raw("eset  "),
        Span::styled("s", Style::default().fg(Color::Cyan)),
        Span::raw("ave"),
    ]));

    frame.render_widget(left, chunks[0]);
    frame.render_widget(right, chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tux_core::fan_curve::FanCurvePoint;

    #[test]
    fn chart_handles_empty_points() {
        // Should not panic with empty points.
        let state = FanCurveState {
            points: vec![],
            selected_index: 0,
            current_temp: None,
            current_speed: None,
            dirty: false,
            original_points: vec![],
        };
        // Build same data as render_chart — verify no panic.
        let curve_data: Vec<(f64, f64)> = state
            .points
            .iter()
            .map(|p| (p.temp as f64, p.speed as f64))
            .collect();
        assert!(curve_data.is_empty());
    }

    #[test]
    fn selected_info_shows_point_position() {
        let state = FanCurveState {
            points: vec![
                FanCurvePoint { temp: 40, speed: 0 },
                FanCurvePoint {
                    temp: 80,
                    speed: 80,
                },
            ],
            selected_index: 1,
            current_temp: Some(65),
            current_speed: Some(40),
            dirty: false,
            original_points: vec![],
        };
        let p = &state.points[state.selected_index];
        let info = format!(
            "◆ Selected: {}°C → {}%  [{}/{}]",
            p.temp,
            p.speed,
            state.selected_index + 1,
            state.points.len()
        );
        assert_eq!(info, "◆ Selected: 80°C → 80%  [2/2]");
    }
}
