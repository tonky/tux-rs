//! Dashboard tab: real-time fan and temperature telemetry.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Bar, BarChart, BarGroup, Block, Borders, Paragraph, Sparkline};

use crate::model::DashboardState;

/// Render the dashboard tab.
pub fn render(frame: &mut Frame, area: Rect, state: &DashboardState) {
    let num_cores = state.cpu_load_per_core.len();
    // Status block: 1 line for summary + per-core bars + 2 for borders
    let status_height = if num_cores > 0 {
        (1 + num_cores as u16 + 2).max(4)
    } else {
        3 // summary line + borders
    };

    let constraints = vec![
        Constraint::Length(5),          // Fans + Fan Speed (side by side)
        Constraint::Length(5),          // CPU Temp + CPU Load (side by side)
        Constraint::Min(status_height), // Status + per-core bars
    ];

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Row 1: Fans (left) + Fan Speed (right)
    let fan_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);
    render_fan_gauges(frame, fan_row[0], state);
    render_speed_sparkline(frame, fan_row[1], state);

    // Row 2: CPU Temp (left) + CPU Load (right)
    let cpu_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);
    render_temp_sparkline(frame, cpu_row[0], state);
    render_cpu_load_sparkline(frame, cpu_row[1], state);

    // Row 3: Status (summary + per-core)
    render_status_block(frame, chunks[2], state);
}

fn render_fan_gauges(frame: &mut Frame, area: Rect, state: &DashboardState) {
    if state.fan_data.is_empty() {
        let p = Paragraph::new("No fans detected")
            .block(Block::default().borders(Borders::ALL).title("Fans"));
        frame.render_widget(p, area);
        return;
    }

    let bars: Vec<Bar> = state
        .fan_data
        .iter()
        .enumerate()
        .map(|(i, fan)| {
            let color = fan_gauge_color(fan.speed_percent);
            Bar::default()
                .value(fan.speed_percent as u64)
                .label(Line::from(format!("Fan {} ({} RPM)", i + 1, fan.rpm)))
                .style(Style::default().fg(color))
                .text_value(format!("{}%", fan.speed_percent))
        })
        .collect();

    let chart = BarChart::default()
        .block(Block::default().borders(Borders::ALL).title("Fans"))
        .data(BarGroup::default().bars(&bars))
        .max(100)
        .bar_gap(2)
        .bar_width(12);

    frame.render_widget(chart, area);
}

/// Get the color for a fan speed gauge based on percentage thresholds.
pub fn fan_gauge_color(pct: u8) -> Color {
    match pct {
        0..=40 => Color::Green,
        41..=70 => Color::Yellow,
        _ => Color::Red,
    }
}

fn render_temp_sparkline(frame: &mut Frame, area: Rect, state: &DashboardState) {
    let data: Vec<u64> = state
        .temp_history
        .iter()
        .map(|&t| t.max(0.0) as u64)
        .collect();

    let temp_str = state
        .cpu_temp
        .map(|t| format!("{t:.0}°C"))
        .unwrap_or_else(|| "—".to_string());

    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("CPU Temperature  {temp_str}")),
        )
        .data(&data)
        .max(105)
        .style(Style::default().fg(Color::Red));

    frame.render_widget(sparkline, area);
}

fn render_speed_sparkline(frame: &mut Frame, area: Rect, state: &DashboardState) {
    let data: Vec<u64> = state
        .speed_history
        .iter()
        .map(|&s| s.max(0.0) as u64)
        .collect();

    let speed_str = state
        .fan_data
        .first()
        .map(|f| format!("{}%", f.speed_percent))
        .unwrap_or_else(|| "—".to_string());

    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Fan Speed  {speed_str}")),
        )
        .data(&data)
        .max(100)
        .style(Style::default().fg(Color::Cyan));

    frame.render_widget(sparkline, area);
}

fn render_cpu_load_sparkline(frame: &mut Frame, area: Rect, state: &DashboardState) {
    let data: Vec<u64> = state
        .load_history
        .iter()
        .map(|&l| l.max(0.0) as u64)
        .collect();

    let load_str = state
        .cpu_load_overall
        .map(|l| format!("{l:.0}%"))
        .unwrap_or_else(|| "—".to_string());

    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("CPU Load  {load_str}")),
        )
        .data(&data)
        .max(100)
        .style(Style::default().fg(Color::Yellow));

    frame.render_widget(sparkline, area);
}

fn render_status_block(frame: &mut Frame, area: Rect, state: &DashboardState) {
    let power_icon = if state.power_state == "ac" {
        "⚡ AC"
    } else if state.power_state == "battery" {
        "🔋 Battery"
    } else {
        "? Unknown"
    };

    let temp_str = state
        .cpu_temp
        .map(|t| format!("{t:.0}°C"))
        .unwrap_or_else(|| "—".to_string());

    let freq_str = state
        .cpu_freq_mhz
        .map(|f| format!("{f} MHz"))
        .unwrap_or_else(|| "—".to_string());

    let cores_str = state
        .core_count
        .map(|c| format!("{c}"))
        .unwrap_or_else(|| "—".to_string());

    let profile_str = state.active_profile.as_deref().unwrap_or("—");

    let block = Block::default().borders(Borders::ALL).title("Status");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width < 10 {
        return;
    }

    // Summary line
    let summary = Line::from(vec![
        Span::styled("CPU: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(&temp_str),
        Span::raw(" / "),
        Span::raw(&freq_str),
        Span::raw(format!(" ({cores_str} cores)")),
        Span::raw("  "),
        Span::styled("Power: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(power_icon),
        Span::raw("  "),
        Span::styled("Fans: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format!("{}", state.num_fans)),
        Span::raw("  "),
        Span::styled("Profile: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(profile_str),
    ]);
    let summary_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(summary), summary_area);

    // Per-core bars below the summary
    if state.cpu_load_per_core.is_empty() {
        return;
    }
    let num_cores = state.cpu_load_per_core.len();
    let available = inner.height.saturating_sub(1) as usize; // rows below summary
    let visible = available.min(num_cores);

    for i in 0..visible {
        let load = state.cpu_load_per_core[i];
        let freq = state.cpu_freq_per_core.get(i).copied().unwrap_or(0);

        let bar_area = Rect {
            x: inner.x,
            y: inner.y + 1 + i as u16,
            width: inner.width,
            height: 1,
        };

        let ghz = freq as f32 / 1000.0;
        let label = format!("C{i:02} {ghz:4.1}G ");
        let label_width = label.len() as u16;
        let bar_width = bar_area.width.saturating_sub(label_width + 5);

        let filled = ((load / 100.0) * bar_width as f32) as u16;
        let color = load_color(load);

        let pct_str = format!(" {load:3.0}%");

        let line = Line::from(vec![
            Span::styled(label, Style::default().fg(Color::Gray)),
            Span::styled("█".repeat(filled as usize), Style::default().fg(color)),
            Span::styled(
                "░".repeat((bar_width - filled) as usize),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(pct_str, Style::default().fg(color)),
        ]);

        frame.render_widget(Paragraph::new(line), bar_area);
    }
}

/// Color for CPU load percentage.
fn load_color(pct: f32) -> Color {
    match pct as u8 {
        0..=40 => Color::Green,
        41..=70 => Color::Yellow,
        _ => Color::Red,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_color_green_at_30() {
        assert_eq!(fan_gauge_color(30), Color::Green);
    }

    #[test]
    fn fan_color_yellow_at_55() {
        assert_eq!(fan_gauge_color(55), Color::Yellow);
    }

    #[test]
    fn fan_color_red_at_85() {
        assert_eq!(fan_gauge_color(85), Color::Red);
    }

    #[test]
    fn fan_color_boundaries() {
        assert_eq!(fan_gauge_color(0), Color::Green);
        assert_eq!(fan_gauge_color(40), Color::Green);
        assert_eq!(fan_gauge_color(41), Color::Yellow);
        assert_eq!(fan_gauge_color(70), Color::Yellow);
        assert_eq!(fan_gauge_color(71), Color::Red);
        assert_eq!(fan_gauge_color(100), Color::Red);
    }
}
