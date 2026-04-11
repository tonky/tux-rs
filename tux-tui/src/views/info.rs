//! Info tab: system information display.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::model::InfoState;

/// Render the info tab.
pub fn render(frame: &mut Frame, area: Rect, state: &InfoState) {
    let check = Span::styled("✓", Style::default().fg(Color::Green));
    let cross = Span::styled("✗", Style::default().fg(Color::Red));

    let bold = Style::default().add_modifier(Modifier::BOLD);

    let mut lines = vec![
        Line::from(""),
        info_line("TUI Version:", env!("CARGO_PKG_VERSION"), bold),
        info_line(
            "Daemon Version:",
            if state.daemon_version.is_empty() {
                "—"
            } else {
                &state.daemon_version
            },
            bold,
        ),
        info_line(
            "Device:",
            if state.device_name.is_empty() {
                "—"
            } else {
                &state.device_name
            },
            bold,
        ),
        info_line(
            "Platform:",
            if state.platform.is_empty() {
                "—"
            } else {
                &state.platform
            },
            bold,
        ),
        info_line(
            "Hostname:",
            if state.hostname.is_empty() {
                "—"
            } else {
                &state.hostname
            },
            bold,
        ),
        info_line(
            "Kernel:",
            if state.kernel.is_empty() {
                "—"
            } else {
                &state.kernel
            },
            bold,
        ),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Fan Control:       ", bold),
            if state.fan_control {
                check.clone()
            } else {
                cross.clone()
            },
            Span::raw(if state.fan_control {
                format!(" ({} fans)", state.fan_count)
            } else {
                String::new()
            }),
        ]),
        Line::from(vec![
            Span::styled("  Keyboard:          ", bold),
            Span::raw(if state.keyboard_type.is_empty() {
                "—"
            } else {
                &state.keyboard_type
            }),
        ]),
        Line::from(vec![
            Span::styled("  Charging Control:  ", bold),
            if state.charging_control { check } else { cross },
        ]),
    ];

    // ── Battery section ─────────────────────────────────────────
    if state.battery.present {
        let bat = &state.battery;
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ── Battery ──────────────────────",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));

        // Status + capacity
        let status_color = match bat.status.as_str() {
            "Charging" => Color::Green,
            "Discharging" => Color::Yellow,
            "Full" => Color::Green,
            _ => Color::Gray,
        };
        lines.push(Line::from(vec![
            Span::styled("  Status:            ", bold),
            Span::styled(&bat.status, Style::default().fg(status_color)),
            Span::raw(format!("  ({}%)", bat.capacity_percent)),
        ]));

        // Capacity: current charge / current full
        let nominal_v = bat.voltage_design_mv as f64 / 1000.0;
        let charge_now_wh = bat.charge_now_mah as f64 * nominal_v / 1000.0;
        let charge_full_wh = bat.charge_full_mah as f64 * nominal_v / 1000.0;
        lines.push(owned_info_line(
            "Charge:",
            &format!(
                "{} / {} mAh  ({:.1} / {:.1} Wh)",
                bat.charge_now_mah, bat.charge_full_mah, charge_now_wh, charge_full_wh
            ),
            bold,
        ));

        // Design vs current full capacity + health
        let design_wh = bat.charge_full_design_mah as f64 * nominal_v / 1000.0;
        let health_color = if bat.health_percent >= 80 {
            Color::Green
        } else if bat.health_percent >= 50 {
            Color::Yellow
        } else {
            Color::Red
        };
        let degradation = 100u32.saturating_sub(bat.health_percent);
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<21}", "Capacity:"), bold),
            Span::raw(format!(
                "{} / {} mAh  ({:.1} / {:.1} Wh)  ",
                bat.charge_full_mah, bat.charge_full_design_mah, charge_full_wh, design_wh
            )),
            Span::styled(
                format!("{}%", bat.health_percent),
                Style::default().fg(health_color),
            ),
            if degradation > 0 {
                Span::styled(
                    format!("  (-{}%)", degradation),
                    Style::default().fg(Color::DarkGray),
                )
            } else {
                Span::raw("")
            },
        ]));

        lines.push(owned_info_line(
            "Cycle Count:",
            &bat.cycle_count.to_string(),
            bold,
        ));

        // Current draw
        if bat.current_now_ma != 0 {
            let (label, val) = if bat.current_now_ma < 0 {
                let ma = -bat.current_now_ma;
                let watts = ma as f64 * bat.voltage_now_mv as f64 / 1_000_000.0;
                ("Draw:", format!("{} mA  ({:.1} W)", ma, watts))
            } else {
                let watts = bat.current_now_ma as f64 * bat.voltage_now_mv as f64 / 1_000_000.0;
                (
                    "Charge Rate:",
                    format!("{} mA  ({:.1} W)", bat.current_now_ma, watts),
                )
            };
            lines.push(owned_info_line(label, &val, bold));
        }

        lines.push(owned_info_line(
            "Voltage:",
            &format!(
                "{:.2} V  (design {:.2} V)",
                bat.voltage_now_mv as f64 / 1000.0,
                nominal_v
            ),
            bold,
        ));
        lines.push(owned_info_line("Technology:", &bat.technology, bold));

        if !bat.manufacturer.is_empty() {
            lines.push(owned_info_line("Manufacturer:", &bat.manufacturer, bold));
        }
        if !bat.model_name.is_empty() {
            lines.push(owned_info_line("Model:", &bat.model_name, bold));
        }
    }

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("System Information"),
    );
    frame.render_widget(paragraph, area);
}

fn info_line<'a>(label: &'a str, value: &'a str, bold: Style) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {label:<21}"), bold),
        Span::raw(value),
    ])
}

/// Like `info_line` but takes owned values, avoiding lifetime issues with temporaries.
fn owned_info_line(label: &str, value: &str, bold: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {label:<21}"), bold),
        Span::raw(value.to_string()),
    ])
}

#[cfg(test)]
mod tests {
    use crate::model::InfoState;

    #[test]
    fn info_state_defaults() {
        let state = InfoState::default();
        assert!(state.device_name.is_empty());
        assert!(!state.fan_control);
        assert_eq!(state.fan_count, 0);
    }
}
