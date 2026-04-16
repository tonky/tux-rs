//! Generic form widget: renders a list of labeled, typed fields.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::model::{FieldType, Form, TextEditState};

/// Render a form within the given area.
#[allow(dead_code)]
pub fn render(frame: &mut Frame, area: Rect, form: &Form, title: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title.to_string());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if form.fields.is_empty() {
        return;
    }

    // Each field takes 1 line; allocate constraints (skip disabled fields).
    let visible_fields: Vec<(usize, &crate::model::FormField)> = form
        .fields
        .iter()
        .enumerate()
        .filter(|(_, f)| f.enabled)
        .collect();

    let constraints: Vec<Constraint> = visible_fields
        .iter()
        .map(|_| Constraint::Length(1))
        .chain(std::iter::once(Constraint::Min(0))) // spacing
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (chunk_i, &(orig_i, field)) in visible_fields.iter().enumerate() {
        let is_selected = orig_i == form.selected_index;
        let text_edit = if is_selected {
            form.text_edit.as_ref()
        } else {
            None
        };
        let line = render_field_line(field, is_selected, text_edit);
        frame.render_widget(Paragraph::new(line), chunks[chunk_i]);
    }

    // Footer: save/discard hints (or text-edit hints).
    let footer_idx = visible_fields.len();
    if footer_idx < chunks.len() {
        let footer = if form.is_editing_text() {
            Line::from(vec![Span::styled(
                "  [Enter] Confirm   [Esc] Cancel",
                Style::default().fg(Color::DarkGray),
            )])
        } else {
            let dirty_marker = if form.dirty { " (modified)" } else { "" };
            Line::from(vec![
                Span::styled(
                    "  [s] Save   [Esc] Discard   [Enter] Edit text",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(dirty_marker, Style::default().fg(Color::Yellow)),
            ])
        };
        frame.render_widget(Paragraph::new(footer), chunks[footer_idx]);
    }
}

fn render_field_line(
    field: &crate::model::FormField,
    selected: bool,
    text_edit: Option<&TextEditState>,
) -> Line<'static> {
    let label_style = if !field.enabled {
        Style::default().fg(Color::DarkGray)
    } else if selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let pointer = if selected { "▸ " } else { "  " };
    let label = format!("{pointer}{:<20}", field.label);

    let value_str = match &field.field_type {
        FieldType::Text(s) => {
            if let (true, Some(edit)) = (selected, text_edit) {
                // Show buffer with cursor.
                let (before, after) = edit.buffer.split_at(edit.cursor);
                format!("[{before}▏{after}]")
            } else {
                format!("[{s}]")
            }
        }
        FieldType::Number { value, .. } => format!("[{value:>5}]"),
        FieldType::FreqMhz { value, .. } => format!("[{:.1} GHz]", *value as f64 / 1_000.0),
        FieldType::Bool(b) => {
            if *b {
                "[✓]".to_string()
            } else {
                "[✗]".to_string()
            }
        }
        FieldType::Select { options, selected } => {
            let val = options.get(*selected).map(|s| s.as_str()).unwrap_or("—");
            format!("[▸ {val}]")
        }
    };

    Line::from(vec![
        Span::styled(label, label_style),
        Span::styled(value_str, label_style),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FieldType, FormField};

    #[test]
    fn render_field_line_bool_true() {
        let field = FormField {
            label: "Enabled".into(),
            key: None,
            field_type: FieldType::Bool(true),
            enabled: true,
        };
        let line = render_field_line(&field, false, None);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("✓"));
    }

    #[test]
    fn render_field_line_number() {
        let field = FormField {
            label: "Speed".into(),
            key: None,
            field_type: FieldType::Number {
                value: 42,
                min: 0,
                max: 100,
                step: 1,
            },
            enabled: true,
        };
        let line = render_field_line(&field, true, None);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("42"));
        assert!(text.contains("▸"));
    }

    #[test]
    fn render_field_line_disabled() {
        let field = FormField {
            label: "Locked".into(),
            key: None,
            field_type: FieldType::Bool(false),
            enabled: false,
        };
        let line = render_field_line(&field, false, None);
        // Check that the style is DarkGray for disabled.
        assert_eq!(line.spans[0].style.fg, Some(Color::DarkGray));
    }
}
