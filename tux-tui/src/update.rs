//! Update layer: pure state transitions in response to events.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::command::Command;
use crate::event::DbusUpdate;
use crate::model::{EventSource, Form, FormTabState, Model, ProfilesMode, ProfilesState, Tab};

/// Handle a key event, returning commands to execute.
pub fn handle_key(model: &mut Model, key: KeyEvent) -> Vec<Command> {
    model.needs_render = true;

    // Ctrl+C always quits, even during inline text editing.
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        model.should_quit = true;
        let cmds = vec![Command::Quit];
        log_commands(model, &cmds);
        return cmds;
    }

    // While editing text inline, suppress global shortcuts (q/tab/number tabs/etc.).
    if model.editing_text_in.is_some() {
        let (cmds, _) = dispatch_tab_key(model, key);
        log_commands(model, &cmds);
        return cmds;
    }

    // Global key bindings (always active).
    match key.code {
        KeyCode::Char('q') => {
            model.should_quit = true;
            let cmds = vec![Command::Quit];
            log_commands(model, &cmds);
            return cmds;
        }
        KeyCode::Char('?') => {
            model.show_help = !model.show_help;
            model.log_event(
                EventSource::User,
                if model.show_help {
                    "help opened"
                } else {
                    "help closed"
                },
                None,
            );
            return vec![];
        }
        // Number keys switch tabs.
        KeyCode::Char('1') => {
            model.current_tab = Tab::Dashboard;
            return vec![];
        }
        KeyCode::Char('2') => {
            model.current_tab = Tab::Profiles;
            return vec![];
        }
        KeyCode::Char('3') => {
            model.current_tab = Tab::FanCurve;
            return vec![];
        }
        KeyCode::Char('4') => {
            model.current_tab = Tab::Settings;
            return vec![];
        }
        KeyCode::Char('5') => {
            model.current_tab = Tab::Keyboard;
            return vec![];
        }
        KeyCode::Char('6') => {
            model.current_tab = Tab::Charging;
            return vec![];
        }
        KeyCode::Char('7') => {
            model.current_tab = Tab::Power;
            return vec![];
        }
        KeyCode::Char('8') => {
            model.current_tab = Tab::Display;
            return vec![];
        }
        KeyCode::Char('9') => {
            model.current_tab = Tab::Webcam;
            return vec![];
        }
        KeyCode::Char('0') => {
            model.current_tab = Tab::Info;
            return vec![];
        }
        KeyCode::Char('l') => {
            model.current_tab = Tab::EventLog;
            model.log_event(EventSource::User, "opened event log tab", None);
            return vec![];
        }
        KeyCode::Char('D') => {
            model.event_log.toggle_debug_filter();
            let state = if model.event_log.show_debug_events {
                "enabled"
            } else {
                "disabled"
            };
            model.log_event(
                EventSource::User,
                format!("debug event filter {state}"),
                Some("D toggles full-detail debug events".to_string()),
            );
            return vec![];
        }
        KeyCode::Tab => {
            model.current_tab = model.current_tab.next();
            return vec![];
        }
        KeyCode::BackTab => {
            model.current_tab = model.current_tab.prev();
            return vec![];
        }
        _ => {}
    }

    // Tab-specific key handling.
    let (cmds, _) = dispatch_tab_key(model, key);
    log_commands(model, &cmds);
    cmds
}

fn dispatch_tab_key(model: &mut Model, key: KeyEvent) -> (Vec<Command>, bool) {
    let (cmds, editing) = match model.current_tab {
        Tab::FanCurve => handle_fan_curve_key(model, key),
        Tab::Profiles => handle_profiles_key(model, key),
        Tab::Settings => handle_form_tab_key(&mut model.settings, key, Command::SaveSettings),
        Tab::Keyboard => handle_form_tab_key(&mut model.keyboard, key, Command::SaveKeyboard),
        Tab::Charging => handle_form_tab_key(&mut model.charging, key, Command::SaveCharging),
        Tab::Power => handle_form_tab_key(&mut model.power.form_tab, key, Command::SavePower),
        Tab::Display => handle_form_tab_key(&mut model.display, key, Command::SaveDisplay),
        Tab::Webcam => handle_webcam_key(model, key),
        _ => (vec![], false),
    };
    model.editing_text_in = if editing {
        Some(model.current_tab)
    } else {
        None
    };
    (cmds, editing)
}

fn log_commands(model: &mut Model, cmds: &[Command]) {
    for cmd in cmds {
        let (summary, detail) = match cmd {
            Command::Quit => ("command: quit".to_string(), None),
            Command::SaveFanCurve(points) => {
                let selected = model
                    .fan_curve
                    .selected_index
                    .get()
                    .min(points.len().saturating_sub(1));
                let selected_point = points.get(selected).cloned();
                let summary = if let Some(p) = selected_point {
                    format!("save fan curve: selected point {}C -> {}%", p.temp, p.speed)
                } else {
                    "save fan curve".to_string()
                };
                let detail = if points.is_empty() {
                    None
                } else {
                    Some(format_fan_curve_points(points))
                };
                (summary, detail)
            }
            Command::FetchFanCurve => ("command: fetch fan curve".to_string(), None),
            Command::FetchProfiles => ("command: fetch profiles".to_string(), None),
            Command::CopyProfile(id) => (format!("command: copy profile {id}"), None),
            Command::CreateProfile(toml_str) => (
                "command: create profile".to_string(),
                extract_profile_debug_details(toml_str),
            ),
            Command::DeleteProfile(id) => (format!("command: delete profile {id}"), None),
            Command::SaveProfile { id, toml } => (
                format!("command: save profile {id}"),
                extract_profile_debug_details(toml),
            ),
            Command::SetActiveProfile { id, state } => {
                (format!("set active profile '{id}' for {state}"), None)
            }
            Command::SaveSettings(toml_str) => (
                "save settings".to_string(),
                extract_settings_debug_details(toml_str),
            ),
            Command::SaveKeyboard(toml_str) => (
                "save keyboard settings".to_string(),
                extract_keyboard_debug_details(toml_str),
            ),
            Command::SaveCharging(toml_str) => (
                "save charging settings".to_string(),
                extract_charging_debug_details(toml_str),
            ),
            Command::SavePower(toml_str) => (
                "save power settings".to_string(),
                extract_power_debug_details(toml_str),
            ),
            Command::SaveDisplay(toml_str) => (
                "save display settings".to_string(),
                extract_display_debug_details(toml_str),
            ),
            Command::SaveWebcam { device, toml } => (
                format!("save webcam settings for {device}"),
                Some(toml.trim().replace('\n', "; ")),
            ),
            Command::None => ("command: none".to_string(), None),
        };
        model.log_event(EventSource::User, summary, None);
        if let Some(detail) = detail {
            model.log_debug_event(EventSource::User, "command detail", Some(detail));
        }
    }
}

fn toml_table(toml_str: &str) -> Option<toml::Table> {
    toml_str.parse::<toml::Table>().ok()
}

fn toml_string(table: &toml::Table, key: &str) -> Option<String> {
    table
        .get(key)
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
}

fn toml_int(table: &toml::Table, key: &str) -> Option<i64> {
    table.get(key).and_then(|v| v.as_integer())
}

fn extract_keyboard_debug_details(toml_str: &str) -> Option<String> {
    let table = toml_table(toml_str)?;
    let brightness = toml_int(&table, "brightness").unwrap_or(0);
    let mode = toml_string(&table, "mode").unwrap_or_else(|| "unknown".to_string());
    let color = toml_string(&table, "color").unwrap_or_else(|| "unknown".to_string());
    Some(format!(
        "keyboard -> brightness {}%, mode '{}', color {}",
        brightness, mode, color
    ))
}

fn extract_display_debug_details(toml_str: &str) -> Option<String> {
    let table = toml_table(toml_str)?;
    let brightness = toml_int(&table, "brightness")?;
    Some(format!("display -> brightness {}%", brightness))
}

fn extract_power_debug_details(toml_str: &str) -> Option<String> {
    let table = toml_table(toml_str)?;
    let tgp_offset = toml_int(&table, "tgp_offset")?;
    Some(format!("power -> tgp_offset {}", tgp_offset))
}

fn extract_settings_debug_details(toml_str: &str) -> Option<String> {
    let table = toml_table(toml_str)?;
    let unit = toml_string(&table, "temperature_unit").unwrap_or_else(|| "unknown".to_string());
    let fan = table
        .get("fan_control_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let cpu = table
        .get("cpu_settings_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    Some(format!(
        "settings -> temp_unit '{}', fan_control {}, cpu_settings {}",
        unit, fan, cpu
    ))
}

fn extract_charging_debug_details(toml_str: &str) -> Option<String> {
    let table = toml_table(toml_str)?;
    let profile = toml_string(&table, "profile").unwrap_or_else(|| "unknown".to_string());
    let priority = toml_string(&table, "priority").unwrap_or_else(|| "unknown".to_string());
    let start = toml_int(&table, "start_threshold").unwrap_or(-1);
    let end = toml_int(&table, "end_threshold").unwrap_or(-1);
    Some(format!(
        "charging -> profile '{}', priority '{}', start {}%, end {}%",
        profile, priority, start, end
    ))
}

fn extract_profile_debug_details(toml_str: &str) -> Option<String> {
    let table = toml_table(toml_str)?;
    let id = toml_string(&table, "id").unwrap_or_else(|| "unknown".to_string());
    let name = toml_string(&table, "name").unwrap_or_else(|| "unknown".to_string());
    let fan_enabled = table
        .get("fan")
        .and_then(|fan| fan.as_table())
        .and_then(|fan| fan.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    Some(format!(
        "profile '{}' ('{}'), fan_enabled {}",
        id, name, fan_enabled
    ))
}

fn format_fan_curve_points(points: &[tux_core::fan_curve::FanCurvePoint]) -> String {
    points
        .iter()
        .map(|p| format!("{}C->{}%", p.temp, p.speed))
        .collect::<Vec<_>>()
        .join(", ")
}

fn form_string(form: &crate::model::Form, key: &str) -> Option<String> {
    use crate::model::FieldType;
    let field = form.fields.iter().find(|f| f.key == key)?;
    match &field.field_type {
        FieldType::Text(v) => Some(v.clone()),
        FieldType::Select { options, selected } => options.get(*selected).cloned(),
        _ => None,
    }
}

fn form_int(form: &crate::model::Form, key: &str) -> Option<i64> {
    use crate::model::FieldType;
    let field = form.fields.iter().find(|f| f.key == key)?;
    match &field.field_type {
        FieldType::Number { value, .. } => Some(*value),
        _ => None,
    }
}

fn summarize_keyboard_form(form: &crate::model::Form) -> Option<String> {
    let brightness = form_int(form, "brightness")?;
    let mode = form_string(form, "mode").unwrap_or_else(|| "unknown".to_string());
    Some(format!(
        "keyboard set to {}% brightness, mode '{}'",
        brightness, mode
    ))
}

fn summarize_charging_form(form: &crate::model::Form) -> Option<String> {
    let profile = form_string(form, "profile")?;
    let priority = form_string(form, "priority")?;
    let start = form_int(form, "start_threshold").unwrap_or(-1);
    let end = form_int(form, "end_threshold").unwrap_or(-1);
    Some(format!(
        "charging profile '{}' (priority '{}', {}%-{}%)",
        profile, priority, start, end
    ))
}

fn summarize_display_form(form: &crate::model::Form) -> Option<String> {
    let brightness = form_int(form, "brightness")?;
    Some(format!("display brightness set to {}%", brightness))
}

fn summarize_power_form(form: &crate::model::Form) -> Option<String> {
    let offset = form_int(form, "tgp_offset")?;
    Some(format!("power limit offset set to {}", offset))
}

fn form_bool(form: &crate::model::Form, key: &str) -> Option<bool> {
    use crate::model::FieldType;
    let field = form.fields.iter().find(|f| f.key == key)?;
    match &field.field_type {
        FieldType::Bool(v) => Some(*v),
        _ => None,
    }
}

fn summarize_settings_form(form: &crate::model::Form) -> Option<String> {
    let unit = form_string(form, "temperature_unit")?;
    let fan_enabled = form_bool(form, "fan_control_enabled").unwrap_or(true);
    Some(format!(
        "settings updated: temp unit '{}', fan control {}",
        unit,
        if fan_enabled { "on" } else { "off" }
    ))
}

/// Fan curve tab key handling.
fn handle_fan_curve_key(model: &mut Model, key: KeyEvent) -> (Vec<Command>, bool) {
    model.fan_curve.status_message = None;
    match key.code {
        KeyCode::Left => {
            model.fan_curve.select_prev();
        }
        KeyCode::Right => {
            model.fan_curve.select_next();
        }
        KeyCode::Up => {
            model.fan_curve.increase_speed();
        }
        KeyCode::Down => {
            model.fan_curve.decrease_speed();
        }
        KeyCode::Char('i') => {
            model.fan_curve.insert_point();
        }
        KeyCode::Char('x') => {
            model.fan_curve.delete_point();
        }
        KeyCode::Char('r') => {
            model.fan_curve.reset();
        }
        KeyCode::Char('s') => {
            if model.fan_curve.dirty {
                let points = model.fan_curve.points.clone();
                return (vec![Command::SaveFanCurve(points)], false);
            }
        }
        KeyCode::Esc => {
            model.fan_curve.revert();
        }
        _ => {}
    }
    (vec![], false)
}

/// Profiles tab key handling.
fn handle_profiles_key(model: &mut Model, key: KeyEvent) -> (Vec<Command>, bool) {
    match &model.profiles.mode {
        ProfilesMode::List => handle_profiles_list_key(model, key),
        ProfilesMode::Editor { .. } => handle_profiles_editor_key(model, key),
    }
}

/// Keys in profile list mode.
fn handle_profiles_list_key(model: &mut Model, key: KeyEvent) -> (Vec<Command>, bool) {
    // Clear status message on any list interaction.
    model.profiles.status_message = None;
    match key.code {
        KeyCode::Up => {
            model.profiles.select_prev();
        }
        KeyCode::Down => {
            model.profiles.select_next();
        }
        KeyCode::Enter => {
            if let Some(profile) = model.profiles.selected_profile().cloned() {
                let form = ProfilesState::build_editor_form(
                    &profile,
                    model.profiles.cpu_hw_limits.as_ref(),
                );
                model.profiles.mode = ProfilesMode::Editor {
                    form,
                    profile_id: profile.id,
                };
            }
        }
        KeyCode::Char('c') => {
            if let Some(profile) = model.profiles.selected_profile() {
                return (vec![Command::CopyProfile(profile.id.clone())], false);
            }
        }
        KeyCode::Char('d') => {
            if let Some(profile) = model.profiles.selected_profile() {
                if profile.is_default {
                    model.profiles.status_message =
                        Some("Cannot delete built-in profiles".to_string());
                } else {
                    return (vec![Command::DeleteProfile(profile.id.clone())], false);
                }
            }
        }
        KeyCode::Char('a') => {
            if let Some(profile) = model.profiles.selected_profile() {
                return (
                    vec![Command::SetActiveProfile {
                        id: profile.id.clone(),
                        state: "ac".to_string(),
                    }],
                    false,
                );
            }
        }
        KeyCode::Char('b') => {
            if let Some(profile) = model.profiles.selected_profile() {
                return (
                    vec![Command::SetActiveProfile {
                        id: profile.id.clone(),
                        state: "battery".to_string(),
                    }],
                    false,
                );
            }
        }
        _ => {}
    }
    (vec![], false)
}

/// Keys in profile editor mode.
fn handle_profiles_editor_key(model: &mut Model, key: KeyEvent) -> (Vec<Command>, bool) {
    // When a text field is being edited, intercept all keys for inline editing.
    if let ProfilesMode::Editor { form, .. } = &mut model.profiles.mode
        && form.is_editing_text()
    {
        return (vec![], handle_text_edit_key(form, key));
    }
    match key.code {
        KeyCode::Esc => {
            model.profiles.mode = ProfilesMode::List;
        }
        KeyCode::Enter => {
            if let ProfilesMode::Editor { form, .. } = &mut model.profiles.mode {
                form.start_text_edit();
            }
        }
        KeyCode::Up => {
            if let ProfilesMode::Editor { form, .. } = &mut model.profiles.mode {
                form.select_prev();
            }
        }
        KeyCode::Down => {
            if let ProfilesMode::Editor { form, .. } = &mut model.profiles.mode {
                form.select_next();
            }
        }
        KeyCode::Left => {
            if let ProfilesMode::Editor { form, .. } = &mut model.profiles.mode {
                form.adjust(-1);
            }
        }
        KeyCode::Right => {
            if let ProfilesMode::Editor { form, .. } = &mut model.profiles.mode {
                form.adjust(1);
            }
        }
        KeyCode::Char(' ') => {
            if let ProfilesMode::Editor { form, .. } = &mut model.profiles.mode {
                form.toggle();
            }
        }
        KeyCode::Char('s') => {
            if let ProfilesMode::Editor {
                form, profile_id, ..
            } = &model.profiles.mode
            {
                if !form.dirty {
                    return (vec![], false);
                }
                // Find the base profile to apply form changes to.
                let base = model
                    .profiles
                    .profiles
                    .iter()
                    .find(|p| p.id == *profile_id)
                    .cloned();
                let pid = profile_id.clone();
                if let Some(base) = base {
                    let updated = ProfilesState::apply_form_to_profile(form, &base);
                    if let Ok(toml_str) = toml::to_string_pretty(&updated) {
                        return (
                            vec![Command::SaveProfile {
                                id: pid,
                                toml: toml_str,
                            }],
                            false,
                        );
                    }
                } else {
                    // Profile was deleted while editor was open.
                    model.profiles.mode = ProfilesMode::List;
                    model.profiles.status_message = Some("Profile no longer exists".to_string());
                }
            }
        }
        _ => {}
    }

    let editing = if let ProfilesMode::Editor { form, .. } = &model.profiles.mode {
        form.is_editing_text()
    } else {
        false
    };
    (vec![], editing)
}

/// Handle keys while a text field is being edited inline.
fn handle_text_edit_key(form: &mut Form, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Enter => form.confirm_text_edit(),
        KeyCode::Esc => form.cancel_text_edit(),
        KeyCode::Backspace => form.text_backspace(),
        KeyCode::Delete => form.text_delete(),
        KeyCode::Left => form.text_cursor_left(),
        KeyCode::Right => form.text_cursor_right(),
        KeyCode::Char(c) => form.text_input(c),
        _ => {}
    }
    form.is_editing_text()
}

/// Generic key handler for form-backed tabs (Settings, Keyboard, Charging, Power, Display).
fn handle_form_tab_key(
    state: &mut FormTabState,
    key: KeyEvent,
    save_cmd: fn(String) -> Command,
) -> (Vec<Command>, bool) {
    if !state.supported {
        return (vec![], false);
    }
    state.status_message = None;
    // Text edit mode intercepts all keys.
    if state.form.is_editing_text() {
        return (vec![], handle_text_edit_key(&mut state.form, key));
    }
    match key.code {
        KeyCode::Up => state.form.select_prev(),
        KeyCode::Down => state.form.select_next(),
        KeyCode::Left => state.form.adjust(-1),
        KeyCode::Right => state.form.adjust(1),
        KeyCode::Char(' ') => state.form.toggle(),
        KeyCode::Enter => state.form.start_text_edit(),
        KeyCode::Esc => state.form.discard(),
        KeyCode::Char('s') => {
            if state.form.dirty {
                // Serialize form fields as a simple TOML table.
                let toml_str = serialize_form_to_toml(&state.form);
                return (vec![save_cmd(toml_str)], false);
            }
        }
        _ => {}
    }
    (vec![], state.form.is_editing_text())
}

/// Webcam tab key handler: form controls + device switching.
fn handle_webcam_key(model: &mut Model, key: KeyEvent) -> (Vec<Command>, bool) {
    if !model.webcam.form_tab.supported {
        return (vec![], false);
    }
    model.webcam.form_tab.status_message = None;
    if model.webcam.form_tab.form.is_editing_text() {
        return (
            vec![],
            handle_text_edit_key(&mut model.webcam.form_tab.form, key),
        );
    }
    match key.code {
        KeyCode::Up => model.webcam.form_tab.form.select_prev(),
        KeyCode::Down => model.webcam.form_tab.form.select_next(),
        KeyCode::Left => {
            // Shift+Left switches device, plain Left adjusts value.
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                model.webcam.select_prev_device();
            } else {
                model.webcam.form_tab.form.adjust(-1);
            }
        }
        KeyCode::Right => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                model.webcam.select_next_device();
            } else {
                model.webcam.form_tab.form.adjust(1);
            }
        }
        KeyCode::Char(' ') => model.webcam.form_tab.form.toggle(),
        KeyCode::Esc => model.webcam.form_tab.form.discard(),
        KeyCode::Char('s') => {
            if model.webcam.form_tab.form.dirty {
                let device = model
                    .webcam
                    .devices
                    .get(model.webcam.selected_device.get())
                    .cloned()
                    .unwrap_or_default();
                let toml_str = serialize_form_to_toml(&model.webcam.form_tab.form);
                return (
                    vec![Command::SaveWebcam {
                        device,
                        toml: toml_str,
                    }],
                    false,
                );
            }
        }
        _ => {}
    }
    (vec![], model.webcam.form_tab.form.is_editing_text())
}

/// Serialize form fields into a TOML string for D-Bus transmission.
fn serialize_form_to_toml(form: &crate::model::Form) -> String {
    use crate::model::FieldType;
    let mut table = toml::map::Map::new();
    for field in &form.fields {
        let key = field.key.clone();
        let value = match &field.field_type {
            FieldType::Text(v) => toml::Value::String(v.clone()),
            FieldType::Number { value, .. } => toml::Value::Integer(*value),
            FieldType::FreqMhz { value, .. } => toml::Value::Integer(*value),
            FieldType::Bool(v) => toml::Value::Boolean(*v),
            FieldType::Select { options, selected } => {
                toml::Value::String(options.get(*selected).cloned().unwrap_or_default())
            }
        };
        table.insert(key, value);
    }
    toml::to_string(&table).unwrap_or_default()
}

/// Handle a D-Bus data update.
pub fn handle_data(model: &mut Model, update: DbusUpdate) {
    model.needs_render = true;
    match update {
        DbusUpdate::ConnectionStatus(status) => {
            if model.connection_status != status {
                model.log_event(
                    EventSource::Daemon,
                    format!("connection status: {:?}", status),
                    None,
                );
            }
            model.connection_status = status;
        }
        DbusUpdate::DashboardTelemetry {
            cpu_temp,
            fan_speeds,
            fan_duties,
            fan_rpm_available,
            power_state,
            cpu_freq_mhz,
            active_profile,
            cpu_load_overall,
            cpu_load_per_core,
            cpu_freq_per_core,
        } => {
            let prev_temp = model.dashboard.cpu_temp;
            let prev_profile = model.dashboard.active_profile.clone();
            let prev_power = model.dashboard.power_state.clone();
            let prev_fan_data = model.dashboard.fan_data.clone();
            if let Some(temp) = cpu_temp {
                model.dashboard.cpu_temp = Some(temp);
                model.dashboard.push_temp(temp);
                // Feed live temperature to fan curve state.
                model.fan_curve.current_temp = Some(temp as u8);
            }
            // Update fan data, capping at num_fans (or 8 as absolute max).
            let max_fans = if model.dashboard.num_fans > 0 {
                model.dashboard.num_fans as usize
            } else {
                8
            };
            model.dashboard.fan_data.truncate(max_fans);
            for i in 0..fan_speeds.len().min(max_fans) {
                if i >= model.dashboard.fan_data.len() {
                    model
                        .dashboard
                        .fan_data
                        .push(crate::model::FanData::default());
                }
                let rpm = fan_speeds.get(i).copied().unwrap_or(0);
                let duty = fan_duties.get(i).copied().unwrap_or(0);
                let rpm_avail = fan_rpm_available.get(i).copied().unwrap_or(false);
                model.dashboard.fan_data[i].rpm = rpm;
                model.dashboard.fan_data[i].duty_percent = duty;
                model.dashboard.fan_data[i].rpm_available = rpm_avail;
                // Derive speed_percent from PWM duty (authoritative), not RPM.
                model.dashboard.fan_data[i].speed_percent =
                    ((duty as f32 * 100.0) / 255.0).min(100.0) as u8;
            }
            // Push average fan speed to history (derived from duty, not RPM).
            if !fan_duties.is_empty() {
                let avg = fan_duties
                    .iter()
                    .map(|&d| d as f32 * 100.0 / 255.0)
                    .sum::<f32>()
                    / fan_duties.len() as f32;
                let avg_clamped = avg.min(100.0);
                model.dashboard.push_speed(avg_clamped);
                model.fan_curve.current_speed = Some(avg_clamped as u8);
            } else if !fan_speeds.is_empty() {
                // Fallback when duty data is absent (older daemon).
                let max_rpm = model.dashboard.max_rpm;
                let avg = fan_speeds.iter().sum::<u32>() as f32
                    / fan_speeds.len() as f32
                    / max_rpm as f32
                    * 100.0;
                let avg_clamped = avg.min(100.0);
                model.dashboard.push_speed(avg_clamped);
                model.fan_curve.current_speed = Some(avg_clamped as u8);
            }
            if let Some(ps) = power_state {
                model.dashboard.power_state = ps;
            }
            if let Some(freq) = cpu_freq_mhz {
                model.dashboard.cpu_freq_mhz = Some(freq);
            }
            if let Some(profile) = active_profile {
                model.dashboard.active_profile = Some(profile);
            }
            if let Some(load) = cpu_load_overall {
                model.dashboard.cpu_load_overall = Some(load);
                model.dashboard.push_load(load);
            }
            if let Some(per_core) = cpu_load_per_core {
                model.dashboard.cpu_load_per_core = per_core;
            }
            if let Some(freqs) = cpu_freq_per_core {
                model.dashboard.cpu_freq_per_core = freqs;
            }

            if let (Some(prev), Some(now)) = (prev_temp, model.dashboard.cpu_temp)
                && (now - prev).abs() >= 5.0
            {
                model.log_event(
                    EventSource::Daemon,
                    format!("CPU temp changed: {prev:.1}C -> {now:.1}C"),
                    None,
                );
            }
            if prev_profile != model.dashboard.active_profile {
                model.log_event(
                    EventSource::Daemon,
                    format!(
                        "active profile: {}",
                        model
                            .dashboard
                            .active_profile
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string())
                    ),
                    None,
                );
            }
            if prev_power != model.dashboard.power_state {
                model.log_event(
                    EventSource::Daemon,
                    format!("power state: {}", model.dashboard.power_state),
                    None,
                );
            }

            let current_fan_data = model.dashboard.fan_data.clone();
            let cpu_temp_now = model.dashboard.cpu_temp;
            for (i, fan) in current_fan_data.iter().enumerate() {
                let prev = prev_fan_data.get(i);
                let changed = prev
                    .map(|p| p.duty_percent != fan.duty_percent || p.rpm != fan.rpm)
                    .unwrap_or(true);
                if changed {
                    let temp_note = cpu_temp_now
                        .map(|t| format!("cpu temp {:.1}C", t))
                        .unwrap_or_else(|| "cpu temp n/a".to_string());
                    model.log_event(
                        EventSource::Daemon,
                        format!(
                            "fan {} changed: {}% (pwm {}/{})",
                            i + 1,
                            fan.speed_percent,
                            fan.duty_percent,
                            255
                        ),
                        Some(format!("rpm {}, {}", fan.rpm, temp_note)),
                    );
                }
                model.log_debug_event(
                    EventSource::Daemon,
                    format!(
                        "fan {} telemetry: {}% @ {} rpm",
                        i + 1,
                        fan.speed_percent,
                        fan.rpm
                    ),
                    Some(format!(
                        "duty {}/{}, rpm_available {}, cpu_temp {:.1}C",
                        fan.duty_percent,
                        255,
                        fan.rpm_available,
                        cpu_temp_now.unwrap_or(0.0)
                    )),
                );
            }
        }
        DbusUpdate::FanHealth(toml_str) => {
            let prev = model.dashboard.fan_health.clone();
            if let Ok(health) = toml::from_str::<tux_core::dbus_types::FanHealthResponse>(&toml_str)
            {
                model.dashboard.fan_health = if health.status == "ok" {
                    None
                } else {
                    Some(health.status)
                };
            }
            if model.dashboard.fan_health != prev {
                model.log_event(
                    EventSource::Daemon,
                    format!(
                        "fan health: {}",
                        model
                            .dashboard
                            .fan_health
                            .clone()
                            .unwrap_or_else(|| "ok".to_string())
                    ),
                    None,
                );
            }
        }
        DbusUpdate::FanInfo { num_fans, max_rpm } => {
            model.dashboard.num_fans = num_fans;
            if max_rpm > 0 {
                model.dashboard.max_rpm = max_rpm;
            }
        }
        DbusUpdate::CpuCoreCount(count) => {
            model.dashboard.core_count = Some(count);
        }
        DbusUpdate::CpuHwLimits(limits) => {
            model.profiles.cpu_hw_limits = Some(limits);
        }
        DbusUpdate::DeviceName(name) => {
            model.info.device_name = name;
        }
        DbusUpdate::Platform(platform) => {
            model.info.platform = platform;
        }
        DbusUpdate::DaemonVersion(version) => {
            model.info.daemon_version = version;
        }
        DbusUpdate::SystemInfo(toml_str) => {
            if let Ok(info) = toml::from_str::<tux_core::dbus_types::SystemInfoResponse>(&toml_str)
            {
                model.info.hostname = info.hostname;
                model.info.kernel = info.kernel;
            }
        }
        DbusUpdate::BatteryInfo(toml_str) => {
            if let Ok(bat) = toml::from_str::<tux_core::dbus_types::BatteryInfoResponse>(&toml_str)
            {
                model.info.battery = bat;
            }
        }
        DbusUpdate::Capabilities(toml_str) => {
            if let Ok(caps) =
                toml::from_str::<tux_core::dbus_types::CapabilitiesResponse>(&toml_str)
            {
                model.info.fan_control = caps.fan_control;
                model.info.fan_count = caps.fan_count;
                model.info.keyboard_type = caps.keyboard_type.clone();
                if caps.keyboard_type == "none" {
                    model.keyboard.supported = false;
                }
                // For white-only keyboards, disable color and mode fields.
                if caps.keyboard_type == "white" {
                    for field in &mut model.keyboard.form.fields {
                        if field.key == "color" || field.key == "mode" {
                            field.enabled = false;
                        }
                    }
                }
                // Update keyboard mode options from hardware capabilities.
                if !caps.keyboard_modes.is_empty() {
                    for field in &mut model.keyboard.form.fields {
                        if field.key == "mode"
                            && let crate::model::FieldType::Select { options, selected } =
                                &mut field.field_type
                        {
                            *options = caps.keyboard_modes.clone();
                            if *selected >= options.len() {
                                *selected = 0;
                            }
                        }
                    }
                }
                let has_charging = caps.charging_thresholds || caps.charging_profiles;
                model.info.charging_control = has_charging;
                if !has_charging {
                    model.charging.supported = false;
                }
                // Disable individual fields based on what the hardware supports.
                for field in &mut model.charging.form.fields {
                    match field.key.as_str() {
                        "start_threshold" | "end_threshold" => {
                            field.enabled = caps.charging_thresholds;
                        }
                        "profile" | "priority" => {
                            field.enabled = caps.charging_profiles;
                        }
                        _ => {}
                    }
                }
                // Display brightness capability.
                if !caps.display_brightness {
                    model.display.supported = false;
                }
            }
        }
        DbusUpdate::FanCurve(points) => {
            if model.fan_curve.dirty {
                model.fan_curve.status_message =
                    Some("Daemon update skipped (unsaved changes)".into());
                model.log_debug_event(
                    EventSource::Daemon,
                    "update skipped",
                    Some("unsaved local changes".into()),
                );
            } else {
                model.fan_curve.load_curve(points);
            }
        }
        DbusUpdate::FanCurveSaved => {
            // Mark current points as the new baseline.
            model.fan_curve.original_points = model.fan_curve.points.clone();
            model.fan_curve.dirty = false;
        }
        DbusUpdate::ProfileList(profiles) => {
            if let ProfilesMode::Editor { form, .. } = &model.profiles.mode
                && form.dirty
            {
                model.profiles.status_message =
                    Some("Daemon update skipped (unsaved changes)".into());
                model.log_debug_event(
                    EventSource::Daemon,
                    "update skipped",
                    Some("unsaved local profile changes".into()),
                );
            }
            model.profiles.profiles = profiles;
            model
                .profiles
                .selected_index
                .clamp_to(model.profiles.profiles.len());
            // Auto-close editor if the edited profile was removed.
            if let ProfilesMode::Editor { profile_id, .. } = &model.profiles.mode
                && !model.profiles.profiles.iter().any(|p| p.id == *profile_id)
            {
                model.profiles.mode = ProfilesMode::List;
                model.profiles.status_message = Some("Edited profile was deleted".to_string());
            }
        }
        DbusUpdate::ProfileAssignments {
            ac_profile,
            battery_profile,
        } => {
            model.profiles.assignments.ac_profile = ac_profile;
            model.profiles.assignments.battery_profile = battery_profile;
        }
        DbusUpdate::ProfileOperationDone(msg) => {
            model.profiles.status_message = Some(msg);
            model.log_event(
                EventSource::Daemon,
                "profile operation succeeded",
                model.profiles.status_message.clone(),
            );
            // After successful operations, return to list and the caller will refetch.
            if let ProfilesMode::Editor { form, .. } = &mut model.profiles.mode {
                form.mark_saved();
            }
        }
        DbusUpdate::ProfileOperationError(msg) => {
            model.profiles.status_message = Some(format!("Error: {msg}"));
            model.log_event(
                EventSource::Daemon,
                "profile operation failed",
                model.profiles.status_message.clone(),
            );
        }
        DbusUpdate::SettingsData(toml_str) => {
            if model.settings.form.dirty {
                model.settings.status_message =
                    Some("Daemon update skipped (unsaved changes)".into());
                model.log_debug_event(
                    EventSource::Daemon,
                    "update skipped",
                    Some("unsaved local changes".into()),
                );
            } else {
                load_form_from_toml(&mut model.settings.form, &toml_str);
            }
        }
        DbusUpdate::KeyboardData(toml_str) => {
            if model.keyboard.form.dirty {
                model.keyboard.status_message =
                    Some("Daemon update skipped (unsaved changes)".into());
                model.log_debug_event(
                    EventSource::Daemon,
                    "update skipped",
                    Some("unsaved local changes".into()),
                );
            } else {
                load_form_from_toml(&mut model.keyboard.form, &toml_str);
            }
            // Check if keyboard is supported (from capabilities).
            if model.info.keyboard_type == "None" || model.info.keyboard_type.is_empty() {
                model.keyboard.supported = false;
            }
        }
        DbusUpdate::ChargingData(toml_str) => {
            if model.charging.form.dirty {
                model.charging.status_message =
                    Some("Daemon update skipped (unsaved changes)".into());
                model.log_debug_event(
                    EventSource::Daemon,
                    "update skipped",
                    Some("unsaved local changes".into()),
                );
            } else {
                load_form_from_toml(&mut model.charging.form, &toml_str);
            }
        }
        DbusUpdate::GpuInfo(toml_str) => {
            if let Ok(table) = toml_str.parse::<toml::Table>() {
                if let Some(name) = table.get("dgpu_name").and_then(|v| v.as_str()) {
                    model.power.dgpu_name = name.to_string();
                }
                if let Some(name) = table.get("igpu_name").and_then(|v| v.as_str()) {
                    model.power.igpu_name = name.to_string();
                }
                if let Some(temp) = table.get("dgpu_temp").and_then(|v| v.as_float()) {
                    model.power.dgpu_temp = Some(temp as f32);
                }
                if let Some(usage) = table.get("dgpu_usage").and_then(|v| v.as_integer()) {
                    model.power.dgpu_usage = Some(usage as u8);
                }
                if let Some(power) = table.get("dgpu_power").and_then(|v| v.as_float()) {
                    model.power.dgpu_power = Some(power as f32);
                }
                if let Some(usage) = table.get("igpu_usage").and_then(|v| v.as_integer()) {
                    model.power.igpu_usage = Some(usage as u8);
                }
            }
        }
        DbusUpdate::PowerData(toml_str) => {
            if model.power.form_tab.form.dirty {
                model.power.form_tab.status_message =
                    Some("Daemon update skipped (unsaved changes)".into());
                model.log_debug_event(
                    EventSource::Daemon,
                    "update skipped",
                    Some("unsaved local changes".into()),
                );
            } else {
                load_form_from_toml(&mut model.power.form_tab.form, &toml_str);
            }
        }
        DbusUpdate::DisplayData(toml_str) => {
            if model.display.form.dirty {
                model.display.status_message =
                    Some("Daemon update skipped (unsaved changes)".into());
                model.log_debug_event(
                    EventSource::Daemon,
                    "update skipped",
                    Some("unsaved local changes".into()),
                );
            } else {
                load_form_from_toml(&mut model.display.form, &toml_str);
            }
            model.display.supported = true; // Backend responded
        }
        DbusUpdate::WebcamDevices(devices) => {
            model.webcam.devices = devices;
            if !model.webcam.devices.is_empty() {
                model.webcam.form_tab.supported = true; // Backend has devices
            }
            model
                .webcam
                .selected_device
                .clamp_to(model.webcam.devices.len());
        }
        DbusUpdate::WebcamData(toml_str) => {
            if model.webcam.form_tab.form.dirty {
                model.webcam.form_tab.status_message =
                    Some("Daemon update skipped (unsaved changes)".into());
                model.log_debug_event(
                    EventSource::Daemon,
                    "update skipped",
                    Some("unsaved local changes".into()),
                );
            } else {
                load_form_from_toml(&mut model.webcam.form_tab.form, &toml_str);
            }
            model.webcam.form_tab.supported = true; // Backend responded
        }
        DbusUpdate::FormSaved(tab_name) => match tab_name.as_str() {
            "settings" => {
                model.settings.form.mark_saved();
                model.settings.status_message = Some("Settings saved".into());
                let detail = summarize_settings_form(&model.settings.form);
                model.log_event(EventSource::Daemon, "settings saved", detail.clone());
                if let Some(d) = detail {
                    model.log_debug_event(EventSource::Daemon, "settings detail", Some(d));
                }
            }
            "keyboard" => {
                model.keyboard.form.mark_saved();
                model.keyboard.status_message = Some("Keyboard settings saved".into());
                let detail = summarize_keyboard_form(&model.keyboard.form);
                model.log_event(
                    EventSource::Daemon,
                    "keyboard settings saved",
                    detail.clone(),
                );
                if let Some(d) = detail {
                    model.log_debug_event(EventSource::Daemon, "keyboard detail", Some(d));
                }
            }
            "charging" => {
                model.charging.form.mark_saved();
                model.charging.status_message = Some("Charging settings saved".into());
                let detail = summarize_charging_form(&model.charging.form);
                model.log_event(
                    EventSource::Daemon,
                    "charging settings saved",
                    detail.clone(),
                );
                if let Some(d) = detail {
                    model.log_debug_event(EventSource::Daemon, "charging detail", Some(d));
                }
            }
            "power" => {
                model.power.form_tab.form.mark_saved();
                model.power.form_tab.status_message = Some("Power settings saved".into());
                let detail = summarize_power_form(&model.power.form_tab.form);
                model.log_event(EventSource::Daemon, "power settings saved", detail.clone());
                if let Some(d) = detail {
                    model.log_debug_event(EventSource::Daemon, "power detail", Some(d));
                }
            }
            "display" => {
                model.display.form.mark_saved();
                model.display.status_message = Some("Display settings saved".into());
                let detail = summarize_display_form(&model.display.form);
                model.log_event(
                    EventSource::Daemon,
                    "display settings saved",
                    detail.clone(),
                );
                if let Some(d) = detail {
                    model.log_debug_event(EventSource::Daemon, "display detail", Some(d));
                }
            }
            "webcam" => {
                model.webcam.form_tab.form.mark_saved();
                model.webcam.form_tab.status_message = Some("Webcam settings saved".into());
                model.log_event(EventSource::Daemon, "webcam settings saved", None);
            }
            _ => {}
        },
        DbusUpdate::FormSaveError(msg) => {
            // Show on whichever tab is active.
            let status = Some(format!("Error: {msg}"));
            match model.current_tab {
                Tab::Settings => model.settings.status_message = status,
                Tab::Keyboard => model.keyboard.status_message = status,
                Tab::Charging => model.charging.status_message = status,
                Tab::Power => model.power.form_tab.status_message = status,
                Tab::Display => model.display.status_message = status,
                Tab::Webcam => model.webcam.form_tab.status_message = status,
                _ => {}
            }
            model.log_event(EventSource::Daemon, "form save failed", Some(msg));
        }
    }
}

/// Load TOML values into a form's fields, matching by normalized label.
fn load_form_from_toml(form: &mut crate::model::Form, toml_str: &str) {
    use crate::model::FieldType;
    let Ok(table) = toml_str.parse::<toml::Table>() else {
        return;
    };
    for field in &mut form.fields {
        let key = &field.key;
        if let Some(value) = table.get(key) {
            match &mut field.field_type {
                FieldType::Text(v) => {
                    if let Some(s) = value.as_str() {
                        *v = s.to_string();
                    }
                }
                FieldType::Number {
                    value: val,
                    min,
                    max,
                    ..
                } => {
                    if let Some(n) = value.as_integer() {
                        *val = n.clamp(*min, *max);
                    }
                }
                FieldType::Bool(b) => {
                    if let Some(v) = value.as_bool() {
                        *b = v;
                    }
                }
                FieldType::FreqMhz {
                    value: val,
                    min,
                    max,
                    ..
                } => {
                    if let Some(n) = value.as_integer() {
                        *val = n.clamp(*min, *max);
                    }
                }
                FieldType::Select { options, selected } => {
                    if let Some(s) = value.as_str()
                        && let Some(idx) = options.iter().position(|o| o == s)
                    {
                        *selected = idx;
                    }
                }
            }
        }
    }
    // Reset dirty flag and snapshot after loading from daemon.
    form.mark_saved();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn quit_sets_should_quit() {
        let mut model = Model::new();
        let cmds = handle_key(&mut model, key(KeyCode::Char('q')));
        assert!(model.should_quit);
        assert!(matches!(cmds.first(), Some(Command::Quit)));
    }

    #[test]
    fn help_toggles() {
        let mut model = Model::new();
        assert!(!model.show_help);

        handle_key(&mut model, key(KeyCode::Char('?')));
        assert!(model.show_help);

        handle_key(&mut model, key(KeyCode::Char('?')));
        assert!(!model.show_help);
    }

    #[test]
    fn number_keys_switch_tabs() {
        let mut model = Model::new();

        handle_key(&mut model, key(KeyCode::Char('3')));
        assert_eq!(model.current_tab, Tab::FanCurve);

        handle_key(&mut model, key(KeyCode::Char('0')));
        assert_eq!(model.current_tab, Tab::Info);

        handle_key(&mut model, key(KeyCode::Char('1')));
        assert_eq!(model.current_tab, Tab::Dashboard);
    }

    #[test]
    fn tab_cycles_forward() {
        let mut model = Model::new();
        assert_eq!(model.current_tab, Tab::Dashboard);

        handle_key(&mut model, key(KeyCode::Tab));
        assert_eq!(model.current_tab, Tab::Profiles);

        handle_key(&mut model, key(KeyCode::Tab));
        assert_eq!(model.current_tab, Tab::FanCurve);
    }

    #[test]
    fn backtab_cycles_backward() {
        let mut model = Model::new();
        assert_eq!(model.current_tab, Tab::Dashboard);

        handle_key(&mut model, key(KeyCode::BackTab));
        assert_eq!(model.current_tab, Tab::EventLog);
    }

    #[test]
    fn l_opens_event_log_tab() {
        let mut model = Model::new();
        let before = model.event_log.entries.len();
        handle_key(&mut model, key(KeyCode::Char('l')));
        assert_eq!(model.current_tab, Tab::EventLog);
        assert_eq!(model.event_log.entries.len(), before + 1);
        assert_eq!(
            model.event_log.entries.back().map(|e| e.summary.as_str()),
            Some("opened event log tab")
        );
    }

    #[test]
    fn d_toggles_debug_event_filter() {
        let mut model = Model::new();
        assert!(!model.event_log.show_debug_events);

        handle_key(&mut model, key(KeyCode::Char('D')));
        assert!(model.event_log.show_debug_events);

        handle_key(&mut model, key(KeyCode::Char('D')));
        assert!(!model.event_log.show_debug_events);
    }

    #[test]
    fn ctrl_c_quits() {
        let mut model = Model::new();
        let key_event = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        let cmds = handle_key(&mut model, key_event);
        assert!(model.should_quit);
        assert!(matches!(cmds.first(), Some(Command::Quit)));
    }

    #[test]
    fn connection_status_update() {
        let mut model = Model::new();
        let before = model.event_log.entries.len();
        handle_data(
            &mut model,
            DbusUpdate::ConnectionStatus(crate::model::ConnectionStatus::Connected),
        );
        assert_eq!(
            model.connection_status,
            crate::model::ConnectionStatus::Connected
        );
        assert_eq!(model.event_log.entries.len(), before + 1);
        assert!(
            model
                .event_log
                .entries
                .back()
                .map(|e| e.summary.contains("connection status"))
                .unwrap_or(false)
        );
    }

    #[test]
    fn dashboard_telemetry_updates_model() {
        let mut model = Model::new();
        let before = model.event_log.entries.len();
        handle_data(
            &mut model,
            DbusUpdate::DashboardTelemetry {
                cpu_temp: Some(68.5),
                fan_speeds: vec![2400, 2300],
                fan_duties: vec![128, 115],
                fan_rpm_available: vec![true, true],
                power_state: Some("ac".to_string()),
                cpu_freq_mhz: Some(3200),
                active_profile: Some("Office".to_string()),
                cpu_load_overall: Some(45.0),
                cpu_load_per_core: Some(vec![30.0, 60.0]),
                cpu_freq_per_core: Some(vec![3200, 3100]),
            },
        );
        assert_eq!(model.dashboard.cpu_temp, Some(68.5));
        assert_eq!(model.dashboard.fan_data.len(), 2);
        assert_eq!(model.dashboard.fan_data[0].rpm, 2400);
        assert_eq!(model.dashboard.power_state, "ac");
        assert_eq!(model.dashboard.temp_history.len(), 1);
        assert_eq!(model.dashboard.speed_history.len(), 1);
        assert_eq!(model.dashboard.cpu_freq_mhz, Some(3200));
        assert_eq!(model.dashboard.active_profile.as_deref(), Some("Office"));
        assert!(model.event_log.entries.len() > before);
        assert!(model.event_log.entries.iter().any(|e| {
            e.summary.contains("fan 1 changed")
                && e.detail.as_deref().unwrap_or("").contains("cpu temp")
        }));
    }

    #[test]
    fn debug_fan_telemetry_is_recorded_with_filter_off() {
        let mut model = Model::new();
        assert!(!model.event_log.show_debug_events);
        let debug_before = model.event_log.entries.iter().filter(|e| e.debug).count();

        handle_data(
            &mut model,
            DbusUpdate::DashboardTelemetry {
                cpu_temp: Some(60.0),
                fan_speeds: vec![2100],
                fan_duties: vec![84],
                fan_rpm_available: vec![true],
                power_state: Some("battery".to_string()),
                cpu_freq_mhz: Some(2800),
                active_profile: Some("Quiet".to_string()),
                cpu_load_overall: Some(22.0),
                cpu_load_per_core: Some(vec![20.0, 24.0]),
                cpu_freq_per_core: Some(vec![2800, 2750]),
            },
        );

        let debug_after = model.event_log.entries.iter().filter(|e| e.debug).count();
        assert!(debug_after > debug_before);
        assert!(
            model
                .event_log
                .entries
                .iter()
                .any(|e| e.debug && e.summary.contains("fan 1 telemetry"))
        );
    }

    #[test]
    fn cpu_core_count_updates_model() {
        let mut model = Model::new();
        handle_data(&mut model, DbusUpdate::CpuCoreCount(16));
        assert_eq!(model.dashboard.core_count, Some(16));
    }

    #[test]
    fn cpu_hw_limits_updates_profiles_state() {
        let mut model = Model::new();
        let limits = tux_core::dbus_types::CpuHwLimits {
            core_count: 12,
            freq_min_mhz: 400,
            freq_max_mhz: 5200,
        };
        handle_data(&mut model, DbusUpdate::CpuHwLimits(limits.clone()));
        let stored = model.profiles.cpu_hw_limits.expect("hw limits should be stored");
        assert_eq!(stored.core_count, 12);
        assert_eq!(stored.freq_min_mhz, 400);
        assert_eq!(stored.freq_max_mhz, 5200);
    }

    #[test]
    fn system_info_parses_toml() {
        let mut model = Model::new();
        handle_data(
            &mut model,
            DbusUpdate::SystemInfo(
                "version = \"0.1.0\"\nhostname = \"my-laptop\"\nkernel = \"6.8.12\"".to_string(),
            ),
        );
        assert_eq!(model.info.hostname, "my-laptop");
        assert_eq!(model.info.kernel, "6.8.12");
    }

    #[test]
    fn capabilities_parses_toml() {
        let mut model = Model::new();
        handle_data(
            &mut model,
            DbusUpdate::Capabilities(
                "fan_control = true\nfan_count = 2\nkeyboard_type = \"rgb\"\ncharging_thresholds = false".to_string(),
            ),
        );
        assert!(model.info.fan_control);
        assert_eq!(model.info.fan_count, 2);
        assert_eq!(model.info.keyboard_type, "rgb");
        assert!(!model.info.charging_control);
    }

    #[test]
    fn dashboard_handles_no_fans() {
        let mut model = Model::new();
        handle_data(
            &mut model,
            DbusUpdate::DashboardTelemetry {
                cpu_temp: Some(50.0),
                fan_speeds: vec![],
                fan_duties: vec![],
                fan_rpm_available: vec![],
                power_state: None,
                cpu_freq_mhz: None,
                active_profile: None,
                cpu_load_overall: None,
                cpu_load_per_core: None,
                cpu_freq_per_core: None,
            },
        );
        assert!(model.dashboard.fan_data.is_empty());
        assert!(model.dashboard.speed_history.is_empty());
        assert!(model.dashboard.cpu_freq_mhz.is_none());
        assert!(model.dashboard.active_profile.is_none());
    }

    #[test]
    fn speed_percent_derived_from_duty_not_rpm() {
        let mut model = Model::new();
        // duty=255 → speed_percent should be 100%, regardless of rpm=0.
        handle_data(
            &mut model,
            DbusUpdate::DashboardTelemetry {
                cpu_temp: None,
                fan_speeds: vec![0],
                fan_duties: vec![255],
                fan_rpm_available: vec![false],
                power_state: None,
                cpu_freq_mhz: None,
                active_profile: None,
                cpu_load_overall: None,
                cpu_load_per_core: None,
                cpu_freq_per_core: None,
            },
        );
        assert_eq!(model.dashboard.fan_data[0].speed_percent, 100);
        assert!(!model.dashboard.fan_data[0].rpm_available);
    }

    #[test]
    fn fan_health_update_sets_dashboard_state() {
        let mut model = Model::new();
        handle_data(
            &mut model,
            DbusUpdate::FanHealth("status = \"degraded\"\nconsecutive_failures = 7\n".to_string()),
        );
        assert_eq!(model.dashboard.fan_health.as_deref(), Some("degraded"));
    }

    #[test]
    fn fan_health_ok_clears_state() {
        let mut model = Model::new();
        model.dashboard.fan_health = Some("degraded".to_string());
        handle_data(
            &mut model,
            DbusUpdate::FanHealth("status = \"ok\"\nconsecutive_failures = 0\n".to_string()),
        );
        assert!(model.dashboard.fan_health.is_none());
    }

    #[test]
    fn unknown_key_does_nothing() {
        let mut model = Model::new();
        let tab_before = model.current_tab;
        let cmds = handle_key(&mut model, key(KeyCode::Char('z')));
        assert_eq!(model.current_tab, tab_before);
        assert!(!model.should_quit);
        assert!(cmds.is_empty());
    }

    // ── Fan Curve key handling tests ──

    #[test]
    fn fan_curve_arrow_selects_points() {
        let mut model = Model::new();
        model.current_tab = Tab::FanCurve;
        assert_eq!(model.fan_curve.selected_index.get(), 0);

        handle_key(&mut model, key(KeyCode::Right));
        assert_eq!(model.fan_curve.selected_index.get(), 1);

        handle_key(&mut model, key(KeyCode::Left));
        assert_eq!(model.fan_curve.selected_index.get(), 0);
    }

    #[test]
    fn fan_curve_up_down_adjusts_speed() {
        let mut model = Model::new();
        model.current_tab = Tab::FanCurve;
        let original_speed = model.fan_curve.points[0].speed;

        handle_key(&mut model, key(KeyCode::Up));
        assert_eq!(model.fan_curve.points[0].speed, original_speed + 5);
        assert!(model.fan_curve.dirty);

        handle_key(&mut model, key(KeyCode::Down));
        assert_eq!(model.fan_curve.points[0].speed, original_speed);
    }

    #[test]
    fn fan_curve_insert_key() {
        let mut model = Model::new();
        model.current_tab = Tab::FanCurve;
        let original_len = model.fan_curve.points.len();

        handle_key(&mut model, key(KeyCode::Char('i')));
        assert_eq!(model.fan_curve.points.len(), original_len + 1);
    }

    #[test]
    fn fan_curve_delete_key() {
        let mut model = Model::new();
        model.current_tab = Tab::FanCurve;
        // Default curve has 4 points, so delete should work.
        let original_len = model.fan_curve.points.len();
        assert!(original_len > 2);

        handle_key(&mut model, key(KeyCode::Char('x')));
        assert_eq!(model.fan_curve.points.len(), original_len - 1);
    }

    #[test]
    fn fan_curve_reset_key() {
        let mut model = Model::new();
        model.current_tab = Tab::FanCurve;
        // Load a non-default curve.
        model.fan_curve.load_curve(vec![
            tux_core::fan_curve::FanCurvePoint {
                temp: 50,
                speed: 50,
            },
            tux_core::fan_curve::FanCurvePoint {
                temp: 90,
                speed: 100,
            },
        ]);
        assert!(!model.fan_curve.dirty);

        let cmds = handle_key(&mut model, key(KeyCode::Char('r')));
        // Reset restores default 5-point curve and marks dirty.
        assert!(model.fan_curve.dirty);
        assert_eq!(model.fan_curve.points.len(), 5);
        assert_eq!(model.fan_curve.points[0].temp, 0);
        assert_eq!(model.fan_curve.points[4].temp, 100);
        assert!(cmds.is_empty());
    }

    #[test]
    fn fan_curve_save_returns_command() {
        let mut model = Model::new();
        model.current_tab = Tab::FanCurve;
        handle_key(&mut model, key(KeyCode::Up)); // make dirty

        let cmds = handle_key(&mut model, key(KeyCode::Char('s')));
        assert!(matches!(cmds.first(), Some(Command::SaveFanCurve(_))));
    }

    #[test]
    fn fan_curve_save_ignored_when_clean() {
        let mut model = Model::new();
        model.current_tab = Tab::FanCurve;
        assert!(!model.fan_curve.dirty);

        let cmds = handle_key(&mut model, key(KeyCode::Char('s')));
        assert!(cmds.is_empty());
    }

    #[test]
    fn fan_curve_esc_resets() {
        let mut model = Model::new();
        model.current_tab = Tab::FanCurve;
        handle_key(&mut model, key(KeyCode::Up));
        assert!(model.fan_curve.dirty);

        handle_key(&mut model, key(KeyCode::Esc));
        assert!(!model.fan_curve.dirty);
    }

    #[test]
    fn fan_curve_loaded_from_dbus() {
        let mut model = Model::new();
        let points = vec![
            tux_core::fan_curve::FanCurvePoint {
                temp: 30,
                speed: 10,
            },
            tux_core::fan_curve::FanCurvePoint {
                temp: 95,
                speed: 100,
            },
        ];
        handle_data(&mut model, DbusUpdate::FanCurve(points.clone()));
        assert_eq!(model.fan_curve.points, points);
        assert!(!model.fan_curve.dirty);
    }

    #[test]
    fn fan_curve_saved_clears_dirty() {
        let mut model = Model::new();
        model.fan_curve.dirty = true;
        handle_data(&mut model, DbusUpdate::FanCurveSaved);
        assert!(!model.fan_curve.dirty);
    }

    // ── Profile list key tests ──

    #[test]
    fn profiles_up_down_navigates() {
        let mut model = Model::new();
        model.current_tab = Tab::Profiles;
        model.profiles.profiles = tux_core::profile::builtin_profiles();
        handle_key(&mut model, key(KeyCode::Down));
        assert_eq!(model.profiles.selected_index.get(), 1);
        handle_key(&mut model, key(KeyCode::Up));
        assert_eq!(model.profiles.selected_index.get(), 0);
    }

    #[test]
    fn profiles_enter_opens_editor() {
        let mut model = Model::new();
        model.current_tab = Tab::Profiles;
        model.profiles.profiles = tux_core::profile::builtin_profiles();
        handle_key(&mut model, key(KeyCode::Enter));
        assert!(matches!(model.profiles.mode, ProfilesMode::Editor { .. }));
    }

    #[test]
    fn profiles_editor_esc_returns_to_list() {
        let mut model = Model::new();
        model.current_tab = Tab::Profiles;
        model.profiles.profiles = tux_core::profile::builtin_profiles();
        handle_key(&mut model, key(KeyCode::Enter));
        assert!(matches!(model.profiles.mode, ProfilesMode::Editor { .. }));
        handle_key(&mut model, key(KeyCode::Esc));
        assert!(matches!(model.profiles.mode, ProfilesMode::List));
    }

    #[test]
    fn profiles_copy_emits_command() {
        let mut model = Model::new();
        model.current_tab = Tab::Profiles;
        model.profiles.profiles = tux_core::profile::builtin_profiles();
        let cmds = handle_key(&mut model, key(KeyCode::Char('c')));
        assert!(cmds.iter().any(|c| matches!(c, Command::CopyProfile(_))));
    }

    #[test]
    fn profiles_delete_builtin_shows_error() {
        let mut model = Model::new();
        model.current_tab = Tab::Profiles;
        model.profiles.profiles = tux_core::profile::builtin_profiles();
        let cmds = handle_key(&mut model, key(KeyCode::Char('d')));
        assert!(cmds.is_empty());
        assert!(model.profiles.status_message.is_some());
    }

    #[test]
    fn profiles_set_ac_emits_command() {
        let mut model = Model::new();
        model.current_tab = Tab::Profiles;
        model.profiles.profiles = tux_core::profile::builtin_profiles();
        let cmds = handle_key(&mut model, key(KeyCode::Char('a')));
        assert!(cmds.iter().any(|c| matches!(
            c,
            Command::SetActiveProfile { state, .. } if state == "ac"
        )));
    }

    #[test]
    fn profiles_set_battery_emits_command() {
        let mut model = Model::new();
        model.current_tab = Tab::Profiles;
        model.profiles.profiles = tux_core::profile::builtin_profiles();
        let cmds = handle_key(&mut model, key(KeyCode::Char('b')));
        assert!(cmds.iter().any(|c| matches!(
            c,
            Command::SetActiveProfile { state, .. } if state == "battery"
        )));
    }

    #[test]
    fn profiles_save_ignored_when_clean() {
        let mut model = Model::new();
        model.current_tab = Tab::Profiles;
        model.profiles.profiles = tux_core::profile::builtin_profiles();
        // Enter editor
        handle_key(&mut model, key(KeyCode::Enter));
        // Try save without changes
        let cmds = handle_key(&mut model, key(KeyCode::Char('s')));
        assert!(cmds.is_empty());
    }

    #[test]
    fn profile_list_loaded_from_dbus() {
        let mut model = Model::new();
        let profiles = tux_core::profile::builtin_profiles();
        handle_data(&mut model, DbusUpdate::ProfileList(profiles.clone()));
        assert_eq!(model.profiles.profiles.len(), 4);
    }

    #[test]
    fn profile_assignments_loaded_from_dbus() {
        let mut model = Model::new();
        handle_data(
            &mut model,
            DbusUpdate::ProfileAssignments {
                ac_profile: "__office__".to_string(),
                battery_profile: "__quiet__".to_string(),
            },
        );
        assert_eq!(model.profiles.assignments.ac_profile, "__office__");
        assert_eq!(model.profiles.assignments.battery_profile, "__quiet__");
    }

    #[test]
    fn profiles_editor_closes_when_edited_profile_deleted() {
        let mut model = Model::new();
        model.current_tab = Tab::Profiles;
        model.profiles.profiles = tux_core::profile::builtin_profiles();
        // Open editor on first (builtin) profile.
        handle_key(&mut model, key(KeyCode::Enter));
        assert!(matches!(model.profiles.mode, ProfilesMode::Editor { .. }));

        // Simulate list update that removes all profiles (extreme case).
        handle_data(&mut model, DbusUpdate::ProfileList(vec![]));
        assert!(matches!(model.profiles.mode, ProfilesMode::List));
        assert_eq!(
            model.profiles.status_message.as_deref(),
            Some("Edited profile was deleted")
        );
    }

    #[test]
    fn profiles_editor_stays_open_when_profile_still_exists() {
        let mut model = Model::new();
        model.current_tab = Tab::Profiles;
        model.profiles.profiles = tux_core::profile::builtin_profiles();
        handle_key(&mut model, key(KeyCode::Enter));
        assert!(matches!(model.profiles.mode, ProfilesMode::Editor { .. }));

        // Simulate list update that still contains the edited profile.
        handle_data(
            &mut model,
            DbusUpdate::ProfileList(tux_core::profile::builtin_profiles()),
        );
        assert!(matches!(model.profiles.mode, ProfilesMode::Editor { .. }));
    }

    #[test]
    fn profiles_delete_selected_clamps_index() {
        let mut model = Model::new();
        model.profiles.profiles = tux_core::profile::builtin_profiles();
        model.profiles.selected_index.set(3); // Select last (index 3 of 4 profiles).

        // Simulate profile list shrinking to 3.
        handle_data(
            &mut model,
            DbusUpdate::ProfileList(tux_core::profile::builtin_profiles()[0..3].to_vec()),
        );
        assert_eq!(model.profiles.selected_index.get(), 2);
    }

    #[test]
    fn profiles_empty_list_then_populate() {
        let mut ps = ProfilesState::new();
        assert!(ps.selected_profile().is_none());

        ps.select_next(); // Should be safe (noop).
        assert_eq!(ps.selected_index.get(), 0);

        ps.profiles = tux_core::profile::builtin_profiles();
        assert!(ps.selected_profile().is_some());
        assert_eq!(ps.selected_index.get(), 0);
    }

    #[test]
    fn profiles_list_navigation_clears_status() {
        let mut model = Model::new();
        model.current_tab = Tab::Profiles;
        model.profiles.profiles = tux_core::profile::builtin_profiles();
        model.profiles.status_message = Some("old message".to_string());

        handle_key(&mut model, key(KeyCode::Down));
        assert!(model.profiles.status_message.is_none());
    }

    // ── Text edit tests ──

    /// Helper: set up a model with a custom profile in the editor.
    fn model_in_custom_profile_editor() -> Model {
        let mut model = Model::new();
        model.current_tab = Tab::Profiles;
        let mut profile = tux_core::profile::builtin_profiles()[0].clone();
        profile.id = "my-custom".into();
        profile.name = "My Custom".into();
        profile.is_default = false;
        model.profiles.profiles = vec![profile];
        // Open editor
        handle_key(&mut model, key(KeyCode::Enter));
        assert!(matches!(model.profiles.mode, ProfilesMode::Editor { .. }));
        model
    }

    #[test]
    fn text_edit_enter_starts_editing() {
        let mut model = model_in_custom_profile_editor();
        // First field is Name (Text). Press Enter to start editing.
        handle_key(&mut model, key(KeyCode::Enter));
        if let ProfilesMode::Editor { form, .. } = &model.profiles.mode {
            assert!(form.is_editing_text());
            let edit = form.text_edit.as_ref().unwrap();
            assert_eq!(edit.buffer, "My Custom");
        } else {
            panic!("expected editor mode");
        }
    }

    #[test]
    fn text_edit_type_and_confirm() {
        let mut model = model_in_custom_profile_editor();
        handle_key(&mut model, key(KeyCode::Enter)); // start editing Name
        handle_key(&mut model, key(KeyCode::Char('!'))); // type a char
        handle_key(&mut model, key(KeyCode::Enter)); // confirm
        if let ProfilesMode::Editor { form, .. } = &model.profiles.mode {
            assert!(!form.is_editing_text());
            assert!(form.dirty);
            if let crate::model::FieldType::Text(ref s) = form.fields[0].field_type {
                assert_eq!(s, "My Custom!");
            } else {
                panic!("expected Text field");
            }
        }
    }

    #[test]
    fn text_edit_q_does_not_trigger_quit_hotkey() {
        let mut model = model_in_custom_profile_editor();
        handle_key(&mut model, key(KeyCode::Enter)); // start editing Name
        handle_key(&mut model, key(KeyCode::Char('q')));

        assert!(!model.should_quit);
        if let ProfilesMode::Editor { form, .. } = &model.profiles.mode {
            assert!(form.is_editing_text());
            let edit = form.text_edit.as_ref().unwrap();
            assert!(edit.buffer.ends_with('q'));
        } else {
            panic!("expected editor mode");
        }
    }

    #[test]
    fn text_edit_number_does_not_switch_tab() {
        let mut model = model_in_custom_profile_editor();
        handle_key(&mut model, key(KeyCode::Enter)); // start editing Name
        handle_key(&mut model, key(KeyCode::Char('1')));

        assert_eq!(model.current_tab, Tab::Profiles);
        if let ProfilesMode::Editor { form, .. } = &model.profiles.mode {
            assert!(form.is_editing_text());
            let edit = form.text_edit.as_ref().unwrap();
            assert!(edit.buffer.ends_with('1'));
        } else {
            panic!("expected editor mode");
        }
    }

    #[test]
    fn text_edit_esc_cancels() {
        let mut model = model_in_custom_profile_editor();
        handle_key(&mut model, key(KeyCode::Enter)); // start editing
        handle_key(&mut model, key(KeyCode::Char('X'))); // type
        handle_key(&mut model, key(KeyCode::Esc)); // cancel
        if let ProfilesMode::Editor { form, .. } = &model.profiles.mode {
            assert!(!form.is_editing_text());
            assert!(!form.dirty);
            if let crate::model::FieldType::Text(ref s) = form.fields[0].field_type {
                assert_eq!(s, "My Custom"); // unchanged
            }
        }
    }

    #[test]
    fn text_edit_backspace() {
        let mut model = model_in_custom_profile_editor();
        handle_key(&mut model, key(KeyCode::Enter));
        handle_key(&mut model, key(KeyCode::Backspace));
        if let ProfilesMode::Editor { form, .. } = &model.profiles.mode {
            let edit = form.text_edit.as_ref().unwrap();
            assert_eq!(edit.buffer, "My Custo");
        }
    }

    #[test]
    fn text_edit_cursor_movement() {
        let mut model = model_in_custom_profile_editor();
        handle_key(&mut model, key(KeyCode::Enter));
        // Cursor starts at end (9). Move left, type character.
        handle_key(&mut model, key(KeyCode::Left));
        handle_key(&mut model, key(KeyCode::Char('Z')));
        handle_key(&mut model, key(KeyCode::Enter)); // confirm
        if let ProfilesMode::Editor { form, .. } = &model.profiles.mode
            && let crate::model::FieldType::Text(ref s) = form.fields[0].field_type
        {
            assert_eq!(s, "My CustoZm");
        }
    }

    #[test]
    fn text_edit_on_non_text_field_is_noop() {
        let mut model = model_in_custom_profile_editor();
        // Move to a non-text field (e.g., Fan Control = Bool)
        handle_key(&mut model, key(KeyCode::Down)); // Description
        handle_key(&mut model, key(KeyCode::Down)); // Fan Control (Bool)
        handle_key(&mut model, key(KeyCode::Enter)); // try to start text edit
        if let ProfilesMode::Editor { form, .. } = &model.profiles.mode {
            assert!(!form.is_editing_text());
        }
    }

    // ── Form-backed tab tests ──

    #[test]
    fn settings_form_navigation() {
        let mut model = Model::new();
        model.current_tab = Tab::Settings;
        assert_eq!(model.settings.form.selected_index, 0);

        handle_key(&mut model, key(KeyCode::Down));
        assert_eq!(model.settings.form.selected_index, 1);

        handle_key(&mut model, key(KeyCode::Up));
        assert_eq!(model.settings.form.selected_index, 0);
    }

    #[test]
    fn settings_form_adjust() {
        let mut model = Model::new();
        model.current_tab = Tab::Settings;
        // Select the "Temperature Unit" field (Select type).
        handle_key(&mut model, key(KeyCode::Right));
        assert!(model.settings.form.dirty);
    }

    #[test]
    fn settings_form_save_returns_command() {
        let mut model = Model::new();
        model.current_tab = Tab::Settings;
        handle_key(&mut model, key(KeyCode::Right)); // Make dirty.
        let cmds = handle_key(&mut model, key(KeyCode::Char('s')));
        assert!(cmds.iter().any(|c| matches!(c, Command::SaveSettings(_))));
    }

    #[test]
    fn settings_form_save_ignored_when_clean() {
        let mut model = Model::new();
        model.current_tab = Tab::Settings;
        let cmds = handle_key(&mut model, key(KeyCode::Char('s')));
        assert!(cmds.is_empty());
    }

    #[test]
    fn settings_form_esc_discards() {
        let mut model = Model::new();
        model.current_tab = Tab::Settings;
        handle_key(&mut model, key(KeyCode::Right)); // Make dirty.
        assert!(model.settings.form.dirty);

        handle_key(&mut model, key(KeyCode::Esc));
        assert!(!model.settings.form.dirty);
    }

    #[test]
    fn keyboard_form_save_returns_command() {
        let mut model = Model::new();
        model.current_tab = Tab::Keyboard;
        handle_key(&mut model, key(KeyCode::Right)); // Make dirty.
        let cmds = handle_key(&mut model, key(KeyCode::Char('s')));
        assert!(cmds.iter().any(|c| matches!(c, Command::SaveKeyboard(_))));
    }

    #[test]
    fn charging_form_save_returns_command() {
        let mut model = Model::new();
        model.current_tab = Tab::Charging;
        handle_key(&mut model, key(KeyCode::Right)); // Make dirty.
        let cmds = handle_key(&mut model, key(KeyCode::Char('s')));
        assert!(cmds.iter().any(|c| matches!(c, Command::SaveCharging(_))));
    }

    #[test]
    fn power_form_save_returns_command() {
        let mut model = Model::new();
        model.current_tab = Tab::Power;
        handle_key(&mut model, key(KeyCode::Right)); // Make dirty.
        let cmds = handle_key(&mut model, key(KeyCode::Char('s')));
        assert!(cmds.iter().any(|c| matches!(c, Command::SavePower(_))));
    }

    #[test]
    fn display_form_save_returns_command() {
        let mut model = Model::new();
        model.current_tab = Tab::Display;
        model.display.supported = true;
        handle_key(&mut model, key(KeyCode::Right)); // Make dirty.
        let cmds = handle_key(&mut model, key(KeyCode::Char('s')));
        assert!(cmds.iter().any(|c| matches!(c, Command::SaveDisplay(_))));
    }

    #[test]
    fn unsupported_tab_ignores_keys() {
        let mut model = Model::new();
        model.current_tab = Tab::Charging;
        model.charging.supported = false;
        let cmds = handle_key(&mut model, key(KeyCode::Right));
        assert!(cmds.is_empty());
    }

    #[test]
    fn webcam_save_returns_command_with_device() {
        let mut model = Model::new();
        model.current_tab = Tab::Webcam;
        model.webcam.form_tab.supported = true;
        handle_key(&mut model, key(KeyCode::Right)); // Make dirty.
        let cmds = handle_key(&mut model, key(KeyCode::Char('s')));
        assert!(cmds.iter().any(|c| matches!(c, Command::SaveWebcam { .. })));
    }

    #[test]
    fn webcam_shift_arrows_switch_device() {
        let mut model = Model::new();
        model.current_tab = Tab::Webcam;
        model.webcam.form_tab.supported = true;
        model.webcam.devices = vec!["Cam1".into(), "Cam2".into()];
        assert_eq!(model.webcam.selected_device.get(), 0);

        let shift_right = KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        handle_key(&mut model, shift_right);
        assert_eq!(model.webcam.selected_device.get(), 1);
    }

    #[test]
    fn form_tab_navigation_clears_status() {
        let mut model = Model::new();
        model.current_tab = Tab::Settings;
        model.settings.status_message = Some("old message".to_string());

        handle_key(&mut model, key(KeyCode::Down));
        assert!(model.settings.status_message.is_none());
    }

    #[test]
    fn form_saved_event_sets_status() {
        let mut model = Model::new();
        // Make settings form dirty first.
        model.settings.form.adjust(1);
        assert!(model.settings.form.dirty);

        handle_data(&mut model, DbusUpdate::FormSaved("settings".into()));
        assert_eq!(
            model.settings.status_message.as_deref(),
            Some("Settings saved")
        );
        // Form should be marked saved (dirty cleared, snapshot updated).
        assert!(!model.settings.form.dirty);
        assert!(model.event_log.entries.iter().any(|e| {
            e.summary == "settings saved" && e.detail.as_deref().unwrap_or("").contains("temp unit")
        }));
    }

    #[test]
    fn charging_saved_logs_profile_values() {
        let mut model = Model::new();
        model.current_tab = Tab::Charging;
        // Set profile to stationary.
        handle_key(&mut model, key(KeyCode::Right));
        handle_key(&mut model, key(KeyCode::Right));

        handle_data(&mut model, DbusUpdate::FormSaved("charging".into()));
        assert!(model.event_log.entries.iter().any(|e| {
            e.summary == "charging settings saved"
                && e.detail.as_deref().unwrap_or("").contains("stationary")
        }));
    }

    #[test]
    fn form_save_error_event_sets_status_on_active_tab() {
        let mut model = Model::new();
        model.current_tab = Tab::Keyboard;
        handle_data(
            &mut model,
            DbusUpdate::FormSaveError("connection timeout".into()),
        );
        assert!(
            model
                .keyboard
                .status_message
                .as_deref()
                .unwrap()
                .contains("Error")
        );
    }

    #[test]
    fn gpu_info_updates_power_state() {
        let mut model = Model::new();
        handle_data(
            &mut model,
            DbusUpdate::GpuInfo(
                "dgpu_name = \"RTX 4060\"\ndgpu_temp = 45.0\ndgpu_usage = 3\ndgpu_power = 15.0\nigpu_name = \"Iris Xe\"\nigpu_usage = 12".to_string(),
            ),
        );
        assert_eq!(model.power.dgpu_name, "RTX 4060");
        assert_eq!(model.power.dgpu_temp, Some(45.0));
        assert_eq!(model.power.dgpu_usage, Some(3));
        assert_eq!(model.power.igpu_name, "Iris Xe");
    }

    #[test]
    fn settings_data_loads_into_form() {
        let mut model = Model::new();
        handle_data(
            &mut model,
            DbusUpdate::SettingsData(
                "temperature_unit = \"Fahrenheit\"\nfan_control_enabled = false".to_string(),
            ),
        );
        // First field is "Temperature Unit" — should now be Fahrenheit (index 1).
        if let crate::model::FieldType::Select { selected, .. } =
            &model.settings.form.fields[0].field_type
        {
            assert_eq!(*selected, 1);
        } else {
            panic!("Expected Select field");
        }
        // Second field is "Fan Control Enabled" — should be false.
        if let crate::model::FieldType::Bool(v) = &model.settings.form.fields[1].field_type {
            assert!(!v);
        } else {
            panic!("Expected Bool field");
        }
    }

    #[test]
    fn webcam_devices_update_clamps_index() {
        let mut model = Model::new();
        model.webcam.selected_device.set(5);
        handle_data(
            &mut model,
            DbusUpdate::WebcamDevices(vec!["Cam1".into(), "Cam2".into()]),
        );
        assert_eq!(model.webcam.selected_device.get(), 1); // Clamped to last.
    }

    #[test]
    fn serialize_form_roundtrip() {
        let form = crate::model::settings_form();
        let toml_str = serialize_form_to_toml(&form.form);
        assert!(toml_str.contains("temperature_unit"));
        assert!(toml_str.contains("fan_control_enabled"));
    }

    #[test]
    fn capabilities_disables_charging_tab() {
        let mut model = Model::new();
        assert!(model.charging.supported);
        handle_data(
            &mut model,
            DbusUpdate::Capabilities("charging_thresholds = false".to_string()),
        );
        assert!(!model.charging.supported);
    }

    #[test]
    fn capabilities_profiles_only_enables_tab_disables_thresholds() {
        let mut model = Model::new();
        handle_data(
            &mut model,
            DbusUpdate::Capabilities(
                "charging_thresholds = false\ncharging_profiles = true".to_string(),
            ),
        );
        // Tab should be enabled (profiles are supported).
        assert!(model.charging.supported);
        // Profile/priority fields enabled, threshold fields disabled.
        for field in &model.charging.form.fields {
            match field.key.as_str() {
                "profile" | "priority" => {
                    assert!(field.enabled, "field '{}' should be enabled", field.label);
                }
                "start_threshold" | "end_threshold" => {
                    assert!(!field.enabled, "field '{}' should be disabled", field.label);
                }
                _ => {}
            }
        }
    }

    #[test]
    fn capabilities_disables_keyboard_tab() {
        let mut model = Model::new();
        assert!(model.keyboard.supported);
        handle_data(
            &mut model,
            DbusUpdate::Capabilities("keyboard_type = \"none\"".to_string()),
        );
        assert!(!model.keyboard.supported);
    }

    #[test]
    fn charging_form_loads_from_toml_keys() {
        let mut model = Model::new();
        let toml = r#"profile = "stationary"
priority = "performance"
start_threshold = 20
end_threshold = 80"#;
        load_form_from_toml(&mut model.charging.form, toml);
        let serialized = serialize_form_to_toml(&model.charging.form);
        assert!(
            serialized.contains("profile = \"stationary\""),
            "expected stationary, got: {serialized}"
        );
        assert!(
            serialized.contains("priority = \"performance\""),
            "expected performance, got: {serialized}"
        );
    }

    #[test]
    fn text_edit_suppresses_global_hotkeys() {
        let mut model = Model::new();
        let profile = tux_core::profile::TuxProfile {
            id: "test".into(),
            name: "Test".into(),
            ..Default::default()
        };
        model.current_tab = Tab::Profiles;
        model.profiles.mode = crate::model::ProfilesMode::Editor {
            form: crate::model::ProfilesState::build_editor_form(&profile, None),
            profile_id: profile.id.clone(),
        };
        // Select first field ("Name", which is Text).
        if let crate::model::ProfilesMode::Editor { form, .. } = &mut model.profiles.mode {
            form.selected_index = 0;
        }

        // Initially not editing.
        assert!(model.editing_text_in.is_none());

        // Press Enter to start editing.
        handle_key(&mut model, key(KeyCode::Enter));
        assert_eq!(model.editing_text_in, Some(Tab::Profiles));

        // Press '1' (Dashboard shortcut). It should be suppressed.
        handle_key(&mut model, key(KeyCode::Char('1')));
        assert_eq!(model.current_tab, Tab::Profiles);
        assert_eq!(model.editing_text_in, Some(Tab::Profiles));

        // Press Esc to cancel editing.
        handle_key(&mut model, key(KeyCode::Esc));
        assert!(model.editing_text_in.is_none());

        // Now '1' should work.
        handle_key(&mut model, key(KeyCode::Char('1')));
        assert_eq!(model.current_tab, Tab::Dashboard);
    }

    #[test]
    fn daemon_updates_skipped_when_form_dirty() {
        let mut model = Model::new();

        // 1. Settings: change to Fahrenheit locally, daemon sends Celsius.
        model.settings.form.adjust(1); // 0 (Celsius) -> 1 (Fahrenheit)
        assert!(model.settings.form.dirty);
        handle_data(
            &mut model,
            DbusUpdate::SettingsData("temperature_unit = \"Celsius\"".into()),
        );
        assert!(
            model
                .settings
                .status_message
                .as_ref()
                .unwrap()
                .contains("skipped")
        );
        // Check that it didn't load (remains 1 / Fahrenheit).
        if let crate::model::FieldType::Select { selected, .. } =
            &model.settings.form.fields[0].field_type
        {
            assert_eq!(*selected, 1);
        }

        // 2. Keyboard: change brightness to 55 locally, daemon sends 99.
        model.keyboard.form.adjust(1); // 50 -> 55
        handle_data(
            &mut model,
            DbusUpdate::KeyboardData("brightness = 99".into()),
        );
        assert!(
            model
                .keyboard
                .status_message
                .as_ref()
                .unwrap()
                .contains("skipped")
        );
        if let crate::model::FieldType::Number { value, .. } =
            &model.keyboard.form.fields[0].field_type
        {
            assert_eq!(*value, 55);
        }

        // 3. Charging: change to Stationary locally, daemon sends Balanced.
        model.charging.form.adjust(2); // 0 (High Capacity) -> 2 (Stationary)
        handle_data(
            &mut model,
            DbusUpdate::ChargingData("profile = \"balanced\"".into()),
        );
        assert!(
            model
                .charging
                .status_message
                .as_ref()
                .unwrap()
                .contains("skipped")
        );
        if let crate::model::FieldType::Select { selected, .. } =
            &model.charging.form.fields[0].field_type
        {
            assert_eq!(*selected, 2);
        }

        // 4. Power: change offset to 1 locally, daemon sends 10.
        model.power.form_tab.form.adjust(1); // 0 -> 1
        handle_data(&mut model, DbusUpdate::PowerData("tgp_offset = 10".into()));
        assert!(
            model
                .power
                .form_tab
                .status_message
                .as_ref()
                .unwrap()
                .contains("skipped")
        );
        if let crate::model::FieldType::Number { value, .. } =
            &model.power.form_tab.form.fields[0].field_type
        {
            assert_eq!(*value, 1);
        }

        // 5. Display: change brightness to 55 locally, daemon sends 10.
        model.display.form.adjust(1); // 50 -> 55
        handle_data(
            &mut model,
            DbusUpdate::DisplayData("brightness = 10".into()),
        );
        assert!(
            model
                .display
                .status_message
                .as_ref()
                .unwrap()
                .contains("skipped")
        );
        if let crate::model::FieldType::Number { value, .. } =
            &model.display.form.fields[0].field_type
        {
            assert_eq!(*value, 55);
        }

        // 6. Webcam: change brightness to 55 locally, daemon sends 10.
        model.webcam.form_tab.form.adjust(1); // 50 -> 55
        handle_data(&mut model, DbusUpdate::WebcamData("brightness = 10".into()));
        assert!(
            model
                .webcam
                .form_tab
                .status_message
                .as_ref()
                .unwrap()
                .contains("skipped")
        );
        if let crate::model::FieldType::Number { value, .. } =
            &model.webcam.form_tab.form.fields[0].field_type
        {
            assert_eq!(*value, 55);
        }

        // 7. Fan Curve: change speed locally, daemon sends 100.
        model.fan_curve.increase_speed();
        let local_speed = model.fan_curve.points[0].speed;
        handle_data(
            &mut model,
            DbusUpdate::FanCurve(vec![tux_core::fan_curve::FanCurvePoint {
                temp: 0,
                speed: 100,
            }]),
        );
        // Should still have local speed (increased by 5 from default).
        assert_eq!(model.fan_curve.points[0].speed, local_speed);
        assert_ne!(local_speed, 100);
        // Verify debug event was logged.
        assert!(
            model
                .event_log
                .entries
                .iter()
                .any(|e| e.summary == "update skipped" && e.debug)
        );
    }
}
