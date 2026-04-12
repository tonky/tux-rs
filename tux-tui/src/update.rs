//! Update layer: pure state transitions in response to events.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::command::Command;
use crate::event::DbusUpdate;
use crate::model::{FormTabState, Model, ProfilesMode, ProfilesState, Tab};

/// Handle a key event, returning commands to execute.
pub fn handle_key(model: &mut Model, key: KeyEvent) -> Vec<Command> {
    model.needs_render = true;
    // Global key bindings (always active).
    match key.code {
        KeyCode::Char('q') => {
            model.should_quit = true;
            return vec![Command::Quit];
        }
        KeyCode::Char('?') => {
            model.show_help = !model.show_help;
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

    // Ctrl+C also quits.
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        model.should_quit = true;
        return vec![Command::Quit];
    }

    // Tab-specific key handling.
    match model.current_tab {
        Tab::FanCurve => handle_fan_curve_key(model, key),
        Tab::Profiles => handle_profiles_key(model, key),
        Tab::Settings => handle_form_tab_key(&mut model.settings, key, Command::SaveSettings),
        Tab::Keyboard => handle_form_tab_key(&mut model.keyboard, key, Command::SaveKeyboard),
        Tab::Charging => handle_form_tab_key(&mut model.charging, key, Command::SaveCharging),
        Tab::Power => handle_form_tab_key(&mut model.power.form_tab, key, Command::SavePower),
        Tab::Display => handle_form_tab_key(&mut model.display, key, Command::SaveDisplay),
        Tab::Webcam => handle_webcam_key(model, key),
        _ => vec![],
    }
}

/// Fan curve tab key handling.
fn handle_fan_curve_key(model: &mut Model, key: KeyEvent) -> Vec<Command> {
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
                return vec![Command::SaveFanCurve(points)];
            }
        }
        KeyCode::Esc => {
            model.fan_curve.revert();
        }
        _ => {}
    }
    vec![]
}

/// Profiles tab key handling.
fn handle_profiles_key(model: &mut Model, key: KeyEvent) -> Vec<Command> {
    match &model.profiles.mode {
        ProfilesMode::List => handle_profiles_list_key(model, key),
        ProfilesMode::Editor { .. } => handle_profiles_editor_key(model, key),
    }
}

/// Keys in profile list mode.
fn handle_profiles_list_key(model: &mut Model, key: KeyEvent) -> Vec<Command> {
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
                let form = ProfilesState::build_editor_form(&profile);
                model.profiles.mode = ProfilesMode::Editor {
                    form,
                    profile_id: profile.id,
                };
            }
        }
        KeyCode::Char('c') => {
            if let Some(profile) = model.profiles.selected_profile().cloned() {
                // Build copy with current keyboard/display state from the model.
                let mut copy = profile;
                copy.id = format!("{}-copy", copy.id);
                copy.name = format!("{} (Copy)", copy.name);
                copy.is_default = false;

                // Inherit current keyboard state from Keyboard tab.
                apply_current_keyboard_to_profile(&model.keyboard.form, &mut copy);
                // Inherit current display brightness from Display tab.
                apply_current_display_to_profile(&model.display.form, &mut copy);

                let toml_str = toml::to_string_pretty(&copy).unwrap_or_default();
                return vec![Command::CreateProfile(toml_str)];
            }
        }
        KeyCode::Char('d') => {
            if let Some(profile) = model.profiles.selected_profile() {
                if profile.is_default {
                    model.profiles.status_message =
                        Some("Cannot delete built-in profiles".to_string());
                } else {
                    return vec![Command::DeleteProfile(profile.id.clone())];
                }
            }
        }
        KeyCode::Char('a') => {
            if let Some(profile) = model.profiles.selected_profile() {
                return vec![Command::SetActiveProfile {
                    id: profile.id.clone(),
                    state: "ac".to_string(),
                }];
            }
        }
        KeyCode::Char('b') => {
            if let Some(profile) = model.profiles.selected_profile() {
                return vec![Command::SetActiveProfile {
                    id: profile.id.clone(),
                    state: "battery".to_string(),
                }];
            }
        }
        _ => {}
    }
    vec![]
}

/// Keys in profile editor mode.
fn handle_profiles_editor_key(model: &mut Model, key: KeyEvent) -> Vec<Command> {
    match key.code {
        KeyCode::Esc => {
            model.profiles.mode = ProfilesMode::List;
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
                    return vec![];
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
                        return vec![Command::SaveProfile {
                            id: pid,
                            toml: toml_str,
                        }];
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
    vec![]
}

/// Generic key handler for form-backed tabs (Settings, Keyboard, Charging, Power, Display).
fn handle_form_tab_key(
    state: &mut FormTabState,
    key: KeyEvent,
    save_cmd: fn(String) -> Command,
) -> Vec<Command> {
    if !state.supported {
        return vec![];
    }
    state.status_message = None;
    match key.code {
        KeyCode::Up => state.form.select_prev(),
        KeyCode::Down => state.form.select_next(),
        KeyCode::Left => state.form.adjust(-1),
        KeyCode::Right => state.form.adjust(1),
        KeyCode::Char(' ') => state.form.toggle(),
        KeyCode::Esc => state.form.discard(),
        KeyCode::Char('s') => {
            if state.form.dirty {
                // Serialize form fields as a simple TOML table.
                let toml_str = serialize_form_to_toml(&state.form);
                return vec![save_cmd(toml_str)];
            }
        }
        _ => {}
    }
    vec![]
}

/// Webcam tab key handler: form controls + device switching.
fn handle_webcam_key(model: &mut Model, key: KeyEvent) -> Vec<Command> {
    if !model.webcam.form_tab.supported {
        return vec![];
    }
    model.webcam.form_tab.status_message = None;
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
                    .get(model.webcam.selected_device)
                    .cloned()
                    .unwrap_or_default();
                let toml_str = serialize_form_to_toml(&model.webcam.form_tab.form);
                return vec![Command::SaveWebcam {
                    device,
                    toml: toml_str,
                }];
            }
        }
        _ => {}
    }
    vec![]
}

/// Serialize form fields into a TOML string for D-Bus transmission.
fn serialize_form_to_toml(form: &crate::model::Form) -> String {
    use crate::model::FieldType;
    let mut table = toml::map::Map::new();
    for field in &form.fields {
        let key = if let Some(ref k) = field.key {
            k.clone()
        } else {
            field
                .label
                .to_lowercase()
                .replace(' ', "_")
                .replace("(%)", "percent")
        };
        let value = match &field.field_type {
            FieldType::Text(v) => toml::Value::String(v.clone()),
            FieldType::Number { value, .. } => toml::Value::Integer(*value),
            FieldType::Bool(v) => toml::Value::Boolean(*v),
            FieldType::Select { options, selected } => {
                toml::Value::String(options.get(*selected).cloned().unwrap_or_default())
            }
        };
        table.insert(key, value);
    }
    toml::to_string(&table).unwrap_or_default()
}

/// Copy current keyboard state from the Keyboard tab form into a profile.
fn apply_current_keyboard_to_profile(
    kb_form: &crate::model::Form,
    profile: &mut tux_core::profile::TuxProfile,
) {
    use crate::model::FieldType;
    for field in &kb_form.fields {
        let key = field.key.clone().unwrap_or_else(|| {
            field
                .label
                .to_lowercase()
                .replace(' ', "_")
                .replace("(%)", "percent")
        });
        match key.as_str() {
            "brightness" => {
                if let FieldType::Number { value, .. } = &field.field_type {
                    profile.keyboard.brightness = *value as u8;
                }
            }
            "color" => {
                if let FieldType::Text(v) = &field.field_type {
                    profile.keyboard.color = v.clone();
                }
            }
            "mode" => {
                if let FieldType::Text(v) = &field.field_type {
                    profile.keyboard.mode = v.clone();
                } else if let FieldType::Select { options, selected } = &field.field_type
                    && let Some(m) = options.get(*selected)
                {
                    profile.keyboard.mode = m.clone();
                }
            }
            _ => {}
        }
    }
}

/// Copy current display brightness from the Display tab form into a profile.
fn apply_current_display_to_profile(
    display_form: &crate::model::Form,
    profile: &mut tux_core::profile::TuxProfile,
) {
    use crate::model::FieldType;
    for field in &display_form.fields {
        let key = field.key.clone().unwrap_or_else(|| {
            field
                .label
                .to_lowercase()
                .replace(' ', "_")
                .replace("(%)", "percent")
        });
        if key == "brightness"
            && let FieldType::Number { value, .. } = &field.field_type
        {
            let v = *value as u8;
            profile.display.brightness = if v > 0 { Some(v) } else { None };
        }
    }
}

/// Handle a D-Bus data update.
pub fn handle_data(model: &mut Model, update: DbusUpdate) {
    model.needs_render = true;
    match update {
        DbusUpdate::ConnectionStatus(status) => {
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
        }
        DbusUpdate::FanHealth(toml_str) => {
            if let Ok(health) = toml::from_str::<tux_core::dbus_types::FanHealthResponse>(&toml_str)
            {
                model.dashboard.fan_health = if health.status == "ok" {
                    None
                } else {
                    Some(health.status)
                };
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
                        if field.label == "Color" || field.label == "Mode" {
                            field.enabled = false;
                        }
                    }
                }
                // Update keyboard mode options from hardware capabilities.
                if !caps.keyboard_modes.is_empty() {
                    for field in &mut model.keyboard.form.fields {
                        if field.label == "Mode"
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
                    match field.key.as_deref() {
                        Some("start_threshold") | Some("end_threshold") => {
                            field.enabled = caps.charging_thresholds;
                        }
                        Some("profile") | Some("priority") => {
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
            model.fan_curve.load_curve(points);
        }
        DbusUpdate::FanCurveSaved => {
            // Mark current points as the new baseline.
            model.fan_curve.original_points = model.fan_curve.points.clone();
            model.fan_curve.dirty = false;
        }
        DbusUpdate::ProfileList(profiles) => {
            model.profiles.profiles = profiles;
            // Clamp selection index.
            if model.profiles.selected_index >= model.profiles.profiles.len()
                && !model.profiles.profiles.is_empty()
            {
                model.profiles.selected_index = model.profiles.profiles.len() - 1;
            }
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
            // After successful operations, return to list and the caller will refetch.
            if let ProfilesMode::Editor { form, .. } = &mut model.profiles.mode {
                form.mark_saved();
            }
        }
        DbusUpdate::ProfileOperationError(msg) => {
            model.profiles.status_message = Some(format!("Error: {msg}"));
        }
        DbusUpdate::SettingsData(toml_str) => {
            load_form_from_toml(&mut model.settings.form, &toml_str);
        }
        DbusUpdate::KeyboardData(toml_str) => {
            load_form_from_toml(&mut model.keyboard.form, &toml_str);
            // Check if keyboard is supported (from capabilities).
            if model.info.keyboard_type == "None" || model.info.keyboard_type.is_empty() {
                model.keyboard.supported = false;
            }
        }
        DbusUpdate::ChargingData(toml_str) => {
            load_form_from_toml(&mut model.charging.form, &toml_str);
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
            load_form_from_toml(&mut model.power.form_tab.form, &toml_str);
        }
        DbusUpdate::DisplayData(toml_str) => {
            load_form_from_toml(&mut model.display.form, &toml_str);
            model.display.supported = true; // Backend responded
        }
        DbusUpdate::WebcamDevices(devices) => {
            model.webcam.devices = devices;
            if !model.webcam.devices.is_empty() {
                model.webcam.form_tab.supported = true; // Backend has devices
            }
            if model.webcam.selected_device >= model.webcam.devices.len()
                && !model.webcam.devices.is_empty()
            {
                model.webcam.selected_device = model.webcam.devices.len() - 1;
            }
        }
        DbusUpdate::WebcamData(toml_str) => {
            load_form_from_toml(&mut model.webcam.form_tab.form, &toml_str);
            model.webcam.form_tab.supported = true; // Backend responded
        }
        DbusUpdate::FormSaved(tab_name) => match tab_name.as_str() {
            "settings" => {
                model.settings.form.mark_saved();
                model.settings.status_message = Some("Settings saved".into());
            }
            "keyboard" => {
                model.keyboard.form.mark_saved();
                model.keyboard.status_message = Some("Keyboard settings saved".into());
            }
            "charging" => {
                model.charging.form.mark_saved();
                model.charging.status_message = Some("Charging settings saved".into());
            }
            "power" => {
                model.power.form_tab.form.mark_saved();
                model.power.form_tab.status_message = Some("Power settings saved".into());
            }
            "display" => {
                model.display.form.mark_saved();
                model.display.status_message = Some("Display settings saved".into());
            }
            "webcam" => {
                model.webcam.form_tab.form.mark_saved();
                model.webcam.form_tab.status_message = Some("Webcam settings saved".into());
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
        let key = field.key.clone().unwrap_or_else(|| {
            field
                .label
                .to_lowercase()
                .replace(' ', "_")
                .replace("(%)", "percent")
        });
        if let Some(value) = table.get(&key) {
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
        assert_eq!(model.current_tab, Tab::Info);
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
        handle_data(
            &mut model,
            DbusUpdate::ConnectionStatus(crate::model::ConnectionStatus::Connected),
        );
        assert_eq!(
            model.connection_status,
            crate::model::ConnectionStatus::Connected
        );
    }

    #[test]
    fn dashboard_telemetry_updates_model() {
        let mut model = Model::new();
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
    }

    #[test]
    fn cpu_core_count_updates_model() {
        let mut model = Model::new();
        handle_data(&mut model, DbusUpdate::CpuCoreCount(16));
        assert_eq!(model.dashboard.core_count, Some(16));
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
        assert_eq!(model.fan_curve.selected_index, 0);

        handle_key(&mut model, key(KeyCode::Right));
        assert_eq!(model.fan_curve.selected_index, 1);

        handle_key(&mut model, key(KeyCode::Left));
        assert_eq!(model.fan_curve.selected_index, 0);
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
        assert_eq!(model.profiles.selected_index, 1);
        handle_key(&mut model, key(KeyCode::Up));
        assert_eq!(model.profiles.selected_index, 0);
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
        assert!(cmds.iter().any(|c| matches!(c, Command::CreateProfile(_))));
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
        model.profiles.selected_index = 3; // Select last (index 3 of 4 profiles).

        // Simulate profile list shrinking to 3.
        handle_data(
            &mut model,
            DbusUpdate::ProfileList(tux_core::profile::builtin_profiles()[0..3].to_vec()),
        );
        assert_eq!(model.profiles.selected_index, 2);
    }

    #[test]
    fn profiles_empty_list_then_populate() {
        let mut ps = ProfilesState::new();
        assert!(ps.selected_profile().is_none());

        ps.select_next(); // Should be safe (noop).
        assert_eq!(ps.selected_index, 0);

        ps.profiles = tux_core::profile::builtin_profiles();
        assert!(ps.selected_profile().is_some());
        assert_eq!(ps.selected_index, 0);
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
        assert_eq!(model.webcam.selected_device, 0);

        let shift_right = KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        handle_key(&mut model, shift_right);
        assert_eq!(model.webcam.selected_device, 1);
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
        model.webcam.selected_device = 5;
        handle_data(
            &mut model,
            DbusUpdate::WebcamDevices(vec!["Cam1".into(), "Cam2".into()]),
        );
        assert_eq!(model.webcam.selected_device, 1); // Clamped to last.
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
            match field.key.as_deref() {
                Some("profile") | Some("priority") => {
                    assert!(field.enabled, "field '{}' should be enabled", field.label);
                }
                Some("start_threshold") | Some("end_threshold") => {
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
}
