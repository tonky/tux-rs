//! Application state for the TUI.

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

use tux_core::fan_curve::FanCurvePoint;
use tux_core::profile::TuxProfile;

/// Which tab is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Profiles,
    FanCurve,
    Settings,
    Keyboard,
    Charging,
    Power,
    Display,
    Webcam,
    Info,
    EventLog,
}

impl Tab {
    /// All tabs in display order.
    pub const ALL: [Tab; 11] = [
        Tab::Dashboard,
        Tab::Profiles,
        Tab::FanCurve,
        Tab::Settings,
        Tab::Keyboard,
        Tab::Charging,
        Tab::Power,
        Tab::Display,
        Tab::Webcam,
        Tab::Info,
        Tab::EventLog,
    ];

    /// Display label for the tab bar.
    pub fn label(self) -> &'static str {
        match self {
            Tab::Dashboard => "1:Dashboard",
            Tab::Profiles => "2:Profiles",
            Tab::FanCurve => "3:Fan Curve",
            Tab::Settings => "4:Settings",
            Tab::Keyboard => "5:Keyboard",
            Tab::Charging => "6:Charging",
            Tab::Power => "7:Power",
            Tab::Display => "8:Display",
            Tab::Webcam => "9:Webcam",
            Tab::Info => "0:Info",
            Tab::EventLog => "L:Event Log",
        }
    }

    /// Index in the ALL array.
    fn index(self) -> usize {
        // Safety: every variant is listed in ALL; this is a const array.
        Tab::ALL.iter().position(|&t| t == self).unwrap_or(0)
    }

    /// Next tab (wraps around).
    pub fn next(self) -> Tab {
        Tab::ALL[(self.index() + 1) % Tab::ALL.len()]
    }

    /// Previous tab (wraps around).
    pub fn prev(self) -> Tab {
        Tab::ALL[(self.index() + Tab::ALL.len() - 1) % Tab::ALL.len()]
    }
}

/// D-Bus connection status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Connecting,
}

/// Top-level application state.
pub struct Model {
    pub current_tab: Tab,
    pub should_quit: bool,
    pub show_help: bool,
    pub connection_status: ConnectionStatus,
    pub terminal_size: (u16, u16),
    /// Whether the model has been mutated since the last render.
    pub needs_render: bool,
    pub dashboard: DashboardState,
    pub info: InfoState,
    pub fan_curve: FanCurveState,
    pub profiles: ProfilesState,
    pub settings: FormTabState,
    pub keyboard: FormTabState,
    pub charging: FormTabState,
    pub power: PowerState,
    pub display: FormTabState,
    pub webcam: WebcamState,
    pub event_log: EventLogState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventSource {
    User,
    Daemon,
    System,
}

#[derive(Debug, Clone)]
pub struct EventLogEntry {
    pub ts_unix_ms: u128,
    pub source: EventSource,
    pub summary: String,
    pub detail: Option<String>,
    pub debug: bool,
}

pub struct EventLogState {
    pub entries: VecDeque<EventLogEntry>,
    pub show_debug_events: bool,
    max_entries: usize,
}

impl EventLogState {
    const DEFAULT_MAX_ENTRIES: usize = 400;

    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            show_debug_events: false,
            max_entries: Self::DEFAULT_MAX_ENTRIES,
        }
    }

    #[cfg(test)]
    pub fn with_max_entries(max_entries: usize) -> Self {
        let mut s = Self::new();
        s.max_entries = max_entries.max(1);
        s
    }

    pub fn push(
        &mut self,
        source: EventSource,
        summary: impl Into<String>,
        detail: Option<String>,
    ) {
        self.push_with_level(source, summary, detail, false);
    }

    pub fn push_debug(
        &mut self,
        source: EventSource,
        summary: impl Into<String>,
        detail: Option<String>,
    ) {
        self.push_with_level(source, summary, detail, true);
    }

    fn push_with_level(
        &mut self,
        source: EventSource,
        summary: impl Into<String>,
        detail: Option<String>,
        debug: bool,
    ) {
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(EventLogEntry {
            ts_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            source,
            summary: summary.into(),
            detail,
            debug,
        });
    }

    pub fn toggle_debug_filter(&mut self) {
        self.show_debug_events = !self.show_debug_events;
    }
}

/// Dashboard tab: real-time telemetry.
pub struct DashboardState {
    pub fan_data: Vec<FanData>,
    pub cpu_temp: Option<f32>,
    pub power_state: String,
    pub temp_history: VecDeque<f32>,
    pub speed_history: VecDeque<f32>,
    pub load_history: VecDeque<f32>,
    pub num_fans: u8,
    pub max_rpm: u32,
    pub cpu_freq_mhz: Option<u32>,
    pub core_count: Option<u32>,
    pub active_profile: Option<String>,
    pub cpu_load_overall: Option<f32>,
    pub cpu_load_per_core: Vec<f32>,
    pub cpu_freq_per_core: Vec<u32>,
    /// Fan engine health: "ok", "degraded", or "failed".
    pub fan_health: Option<String>,
}

/// Per-fan live data.
#[derive(Debug, Clone, Default)]
pub struct FanData {
    pub rpm: u32,
    pub speed_percent: u8,
    /// PWM duty cycle (0–255); authoritative speed indicator.
    pub duty_percent: u8,
    /// `true` when RPM comes from a real hardware tachometer.
    pub rpm_available: bool,
}

/// Info tab: static system information.
#[derive(Default)]
pub struct InfoState {
    pub device_name: String,
    pub platform: String,
    pub daemon_version: String,
    pub hostname: String,
    pub kernel: String,
    pub fan_control: bool,
    pub fan_count: u8,
    pub keyboard_type: String,
    pub charging_control: bool,
    pub battery: tux_core::dbus_types::BatteryInfoResponse,
}

/// Profile assignments: which profile is active for AC and battery.
#[derive(Debug, Clone, Default)]
pub struct ProfileAssignments {
    pub ac_profile: String,
    pub battery_profile: String,
}

/// Profiles tab state.
pub struct ProfilesState {
    pub profiles: Vec<TuxProfile>,
    pub selected_index: usize,
    pub assignments: ProfileAssignments,
    pub mode: ProfilesMode,
    /// Status message shown briefly after operations.
    pub status_message: Option<String>,
}

/// Whether we're in list view or editing a profile.
pub enum ProfilesMode {
    List,
    Editor { form: Form, profile_id: String },
}

impl ProfilesState {
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
            selected_index: 0,
            assignments: ProfileAssignments::default(),
            mode: ProfilesMode::List,
            status_message: None,
        }
    }

    /// Select the next profile in list.
    pub fn select_next(&mut self) {
        if !self.profiles.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.profiles.len();
        }
    }

    /// Select the previous profile in list.
    pub fn select_prev(&mut self) {
        if !self.profiles.is_empty() {
            self.selected_index =
                (self.selected_index + self.profiles.len() - 1) % self.profiles.len();
        }
    }

    /// Get the currently selected profile.
    pub fn selected_profile(&self) -> Option<&TuxProfile> {
        self.profiles.get(self.selected_index)
    }

    /// Build a Form from a TuxProfile for editing.
    pub fn build_editor_form(profile: &TuxProfile) -> Form {
        let read_only = profile.is_default;
        let fields = vec![
            FormField {
                label: "Name".into(),
                key: None,
                field_type: FieldType::Text(profile.name.clone()),
                enabled: !read_only,
            },
            FormField {
                label: "Description".into(),
                key: None,
                field_type: FieldType::Text(profile.description.clone()),
                enabled: !read_only,
            },
            // ── Fan Settings ──
            FormField {
                label: "Fan Control".into(),
                key: None,
                field_type: FieldType::Bool(profile.fan.enabled),
                enabled: !read_only,
            },
            FormField {
                label: "Fan Mode".into(),
                key: None,
                field_type: FieldType::Select {
                    options: vec!["Auto".into(), "CustomCurve".into(), "Manual".into()],
                    selected: match profile.fan.mode {
                        tux_core::fan_curve::FanMode::Auto => 0,
                        tux_core::fan_curve::FanMode::CustomCurve => 1,
                        tux_core::fan_curve::FanMode::Manual => 2,
                    },
                },
                enabled: !read_only,
            },
            FormField {
                label: "Min Speed (%)".into(),
                key: None,
                field_type: FieldType::Number {
                    value: profile.fan.min_speed_percent as i64,
                    min: 0,
                    max: 100,
                    step: 5,
                },
                enabled: !read_only,
            },
            FormField {
                label: "Max Speed (%)".into(),
                key: None,
                field_type: FieldType::Number {
                    value: profile.fan.max_speed_percent as i64,
                    min: 0,
                    max: 100,
                    step: 5,
                },
                enabled: !read_only,
            },
            // ── CPU Settings ──
            FormField {
                label: "Governor".into(),
                key: None,
                field_type: FieldType::Select {
                    options: vec!["powersave".into(), "schedutil".into(), "performance".into()],
                    selected: match profile.cpu.governor.as_str() {
                        "schedutil" => 1,
                        "performance" => 2,
                        _ => 0,
                    },
                },
                enabled: !read_only,
            },
            FormField {
                label: "Energy Pref".into(),
                key: None,
                field_type: FieldType::Select {
                    options: vec![
                        "power".into(),
                        "balance_power".into(),
                        "balance_performance".into(),
                        "performance".into(),
                    ],
                    selected: match profile
                        .cpu
                        .energy_performance_preference
                        .as_deref()
                        .unwrap_or("balance_power")
                    {
                        "power" => 0,
                        "balance_performance" => 2,
                        "performance" => 3,
                        _ => 1,
                    },
                },
                enabled: !read_only,
            },
            FormField {
                label: "No Turbo".into(),
                key: None,
                field_type: FieldType::Bool(profile.cpu.no_turbo),
                enabled: !read_only,
            },
            // ── Keyboard ──
            FormField {
                label: "KB Brightness (%)".into(),
                key: None,
                field_type: FieldType::Number {
                    value: profile.keyboard.brightness as i64,
                    min: 0,
                    max: 100,
                    step: 10,
                },
                enabled: !read_only,
            },
            FormField {
                label: "KB Color".into(),
                key: None,
                field_type: FieldType::Text(profile.keyboard.color.clone()),
                enabled: !read_only,
            },
            FormField {
                label: "KB Mode".into(),
                key: None,
                field_type: FieldType::Select {
                    options: vec![
                        "static".into(),
                        "breathe".into(),
                        "cycle".into(),
                        "wave".into(),
                    ],
                    selected: match profile.keyboard.mode.as_str() {
                        "breathe" => 1,
                        "cycle" => 2,
                        "wave" => 3,
                        _ => 0,
                    },
                },
                enabled: !read_only,
            },
            // ── Display ──
            FormField {
                label: "Display Brightness (%)".into(),
                key: None,
                field_type: FieldType::Number {
                    value: profile.display.brightness.unwrap_or(0) as i64,
                    min: 0,
                    max: 100,
                    step: 5,
                },
                enabled: !read_only,
            },
        ];
        Form::new(fields)
    }

    /// Apply the form values back to a TuxProfile for saving.
    pub fn apply_form_to_profile(form: &Form, base: &TuxProfile) -> TuxProfile {
        let mut profile = base.clone();

        for field in &form.fields {
            match field.label.as_str() {
                "Name" => {
                    if let FieldType::Text(v) = &field.field_type {
                        profile.name = v.clone();
                    }
                }
                "Description" => {
                    if let FieldType::Text(v) = &field.field_type {
                        profile.description = v.clone();
                    }
                }
                "Fan Control" => {
                    if let FieldType::Bool(v) = &field.field_type {
                        profile.fan.enabled = *v;
                    }
                }
                "Fan Mode" => {
                    if let FieldType::Select { options, selected } = &field.field_type
                        && let Some(mode_str) = options.get(*selected)
                    {
                        profile.fan.mode = match mode_str.as_str() {
                            "CustomCurve" => tux_core::fan_curve::FanMode::CustomCurve,
                            "Manual" => tux_core::fan_curve::FanMode::Manual,
                            _ => tux_core::fan_curve::FanMode::Auto,
                        };
                    }
                }
                "Min Speed (%)" => {
                    if let FieldType::Number { value, .. } = &field.field_type {
                        profile.fan.min_speed_percent = *value as u8;
                    }
                }
                "Max Speed (%)" => {
                    if let FieldType::Number { value, .. } = &field.field_type {
                        profile.fan.max_speed_percent = *value as u8;
                    }
                }
                "Governor" => {
                    if let FieldType::Select { options, selected } = &field.field_type
                        && let Some(gov) = options.get(*selected)
                    {
                        profile.cpu.governor = gov.clone();
                    }
                }
                "Energy Pref" => {
                    if let FieldType::Select { options, selected } = &field.field_type
                        && let Some(pref) = options.get(*selected)
                    {
                        profile.cpu.energy_performance_preference = Some(pref.clone());
                    }
                }
                "No Turbo" => {
                    if let FieldType::Bool(v) = &field.field_type {
                        profile.cpu.no_turbo = *v;
                    }
                }
                "KB Brightness (%)" => {
                    if let FieldType::Number { value, .. } = &field.field_type {
                        profile.keyboard.brightness = *value as u8;
                    }
                }
                "KB Color" => {
                    if let FieldType::Text(v) = &field.field_type {
                        profile.keyboard.color = v.clone();
                    }
                }
                "KB Mode" => {
                    if let FieldType::Select { options, selected } = &field.field_type
                        && let Some(mode) = options.get(*selected)
                    {
                        profile.keyboard.mode = mode.clone();
                    }
                }
                "Display Brightness (%)" => {
                    if let FieldType::Number { value, .. } = &field.field_type {
                        let v = *value as u8;
                        profile.display.brightness = if v > 0 { Some(v) } else { None };
                    }
                }
                _ => {}
            }
        }

        profile
    }
}

// ── Form-Backed Tab States ──────────────────────────────────

/// A generic form-backed tab: has a form, a capability flag, and a status message.
pub struct FormTabState {
    pub form: Form,
    /// Whether the hardware supports this feature.
    pub supported: bool,
    /// Status message shown briefly after operations.
    pub status_message: Option<String>,
}

impl FormTabState {
    pub fn new(fields: Vec<FormField>) -> Self {
        Self {
            form: Form::new(fields),
            supported: true,
            status_message: None,
        }
    }

    #[allow(dead_code)]
    pub fn unsupported() -> Self {
        Self {
            form: Form::new(vec![]),
            supported: false,
            status_message: None,
        }
    }
}

/// Build the Settings tab form.
pub fn settings_form() -> FormTabState {
    FormTabState::new(vec![
        FormField {
            label: "Temperature Unit".into(),
            key: None,
            field_type: FieldType::Select {
                options: vec!["Celsius".into(), "Fahrenheit".into()],
                selected: 0,
            },
            enabled: true,
        },
        FormField {
            label: "Fan Control Enabled".into(),
            key: None,
            field_type: FieldType::Bool(true),
            enabled: true,
        },
        FormField {
            label: "CPU Settings Enabled".into(),
            key: None,
            field_type: FieldType::Bool(true),
            enabled: true,
        },
    ])
}

/// Build the Keyboard tab form.
pub fn keyboard_form() -> FormTabState {
    FormTabState::new(vec![
        FormField {
            label: "Brightness".into(),
            key: None,
            field_type: FieldType::Number {
                value: 50,
                min: 0,
                max: 100,
                step: 5,
            },
            enabled: true,
        },
        FormField {
            label: "Color".into(),
            key: None,
            field_type: FieldType::Text("#ffffff".into()),
            enabled: true,
        },
        FormField {
            label: "Mode".into(),
            key: None,
            field_type: FieldType::Select {
                options: vec!["static".into()],
                selected: 0,
            },
            enabled: true,
        },
    ])
}

/// Build the Charging tab form.
pub fn charging_form() -> FormTabState {
    FormTabState::new(vec![
        FormField {
            label: "Charging Profile".into(),
            key: Some("profile".into()),
            field_type: FieldType::Select {
                options: vec![
                    "high_capacity".into(),
                    "balanced".into(),
                    "stationary".into(),
                ],
                selected: 0,
            },
            enabled: true,
        },
        FormField {
            label: "Charging Priority".into(),
            key: Some("priority".into()),
            field_type: FieldType::Select {
                options: vec!["charge_battery".into(), "performance".into()],
                selected: 0,
            },
            enabled: true,
        },
        FormField {
            label: "Start Threshold (%)".into(),
            key: Some("start_threshold".into()),
            field_type: FieldType::Number {
                value: 0,
                min: 0,
                max: 100,
                step: 5,
            },
            enabled: true,
        },
        FormField {
            label: "End Threshold (%)".into(),
            key: Some("end_threshold".into()),
            field_type: FieldType::Number {
                value: 100,
                min: 0,
                max: 100,
                step: 5,
            },
            enabled: true,
        },
    ])
}

/// Build the Display tab form.
pub fn display_form() -> FormTabState {
    let mut state = FormTabState::new(vec![FormField {
        label: "Brightness (%)".into(),
        key: Some("brightness".into()),
        field_type: FieldType::Number {
            value: 50,
            min: 0,
            max: 100,
            step: 5,
        },
        enabled: true,
    }]);
    state.supported = false; // enabled when daemon reports display_brightness capability
    state
}

/// Power tab state: info block + settings form.
pub struct PowerState {
    pub form_tab: FormTabState,
    pub dgpu_name: String,
    pub dgpu_temp: Option<f32>,
    pub dgpu_usage: Option<u8>,
    pub dgpu_power: Option<f32>,
    pub igpu_name: String,
    pub igpu_usage: Option<u8>,
}

impl PowerState {
    pub fn new() -> Self {
        Self {
            form_tab: FormTabState::new(vec![FormField {
                label: "TGP Offset".into(),
                key: None,
                field_type: FieldType::Number {
                    value: 0,
                    min: -15,
                    max: 15,
                    step: 1,
                },
                enabled: true,
            }]),
            dgpu_name: String::new(),
            dgpu_temp: None,
            dgpu_usage: None,
            dgpu_power: None,
            igpu_name: String::new(),
            igpu_usage: None,
        }
    }
}

/// Webcam tab state: multiple devices with per-device controls.
pub struct WebcamState {
    pub form_tab: FormTabState,
    pub devices: Vec<String>,
    pub selected_device: usize,
}

impl WebcamState {
    pub fn new() -> Self {
        Self {
            form_tab: webcam_form(),
            devices: Vec::new(),
            selected_device: 0,
        }
    }

    pub fn select_next_device(&mut self) {
        if !self.devices.is_empty() {
            self.selected_device = (self.selected_device + 1) % self.devices.len();
        }
    }

    pub fn select_prev_device(&mut self) {
        if !self.devices.is_empty() {
            self.selected_device =
                (self.selected_device + self.devices.len() - 1) % self.devices.len();
        }
    }
}

/// Build the Webcam form fields.
fn webcam_form() -> FormTabState {
    let mut state = FormTabState::new(vec![
        FormField {
            label: "Brightness".into(),
            key: None,
            field_type: FieldType::Number {
                value: 50,
                min: 0,
                max: 100,
                step: 5,
            },
            enabled: true,
        },
        FormField {
            label: "Contrast".into(),
            key: None,
            field_type: FieldType::Number {
                value: 50,
                min: 0,
                max: 100,
                step: 5,
            },
            enabled: true,
        },
        FormField {
            label: "Exposure".into(),
            key: None,
            field_type: FieldType::Number {
                value: 30,
                min: 0,
                max: 100,
                step: 5,
            },
            enabled: true,
        },
        FormField {
            label: "Auto Exposure".into(),
            key: None,
            field_type: FieldType::Bool(true),
            enabled: true,
        },
    ]);
    state.supported = false; // No daemon backend yet
    state
}

/// Fan curve editor state.
pub struct FanCurveState {
    /// Editable copy of curve points.
    pub points: Vec<FanCurvePoint>,
    /// Index of the currently selected point.
    pub selected_index: usize,
    /// Live CPU temperature from daemon (°C).
    pub current_temp: Option<u8>,
    /// Live fan speed from daemon (%).
    pub current_speed: Option<u8>,
    /// Whether the curve has unsaved changes.
    pub dirty: bool,
    /// Original points for reset.
    pub original_points: Vec<FanCurvePoint>,
}

impl FanCurveState {
    pub fn new() -> Self {
        let default_points = tux_core::fan_curve::FanConfig::default().curve;
        Self {
            original_points: default_points.clone(),
            points: default_points,
            selected_index: 0,
            current_temp: None,
            current_speed: None,
            dirty: false,
        }
    }

    /// Load curve points from daemon data, resetting edit state.
    pub fn load_curve(&mut self, points: Vec<FanCurvePoint>) {
        self.original_points = points.clone();
        self.points = points;
        self.selected_index = 0;
        self.dirty = false;
    }

    /// Move selection to next point.
    pub fn select_next(&mut self) {
        if !self.points.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.points.len();
        }
    }

    /// Move selection to previous point.
    pub fn select_prev(&mut self) {
        if !self.points.is_empty() {
            self.selected_index = (self.selected_index + self.points.len() - 1) % self.points.len();
        }
    }

    /// Increase selected point's speed by 5%, capped at 100.
    pub fn increase_speed(&mut self) {
        if let Some(p) = self.points.get_mut(self.selected_index) {
            p.speed = (p.speed + 5).min(100);
            self.dirty = true;
        }
    }

    /// Decrease selected point's speed by 5%, floored at 0.
    pub fn decrease_speed(&mut self) {
        if let Some(p) = self.points.get_mut(self.selected_index) {
            p.speed = p.speed.saturating_sub(5);
            self.dirty = true;
        }
    }

    /// Insert a point between selected and next, at their midpoint.
    pub fn insert_point(&mut self) {
        if self.points.len() >= 20 {
            return; // Reasonable cap.
        }
        let idx = self.selected_index;
        if idx + 1 < self.points.len() {
            let a = &self.points[idx];
            let b = &self.points[idx + 1];
            let mid = FanCurvePoint {
                temp: (a.temp / 2) + (b.temp / 2) + (a.temp % 2 + b.temp % 2) / 2,
                speed: (a.speed / 2) + (b.speed / 2) + (a.speed % 2 + b.speed % 2) / 2,
            };
            self.points.insert(idx + 1, mid);
            self.selected_index = idx + 1;
            self.dirty = true;
        }
    }

    /// Delete the selected point (minimum 2 points).
    pub fn delete_point(&mut self) -> bool {
        if self.points.len() <= 2 {
            return false;
        }
        self.points.remove(self.selected_index);
        if self.selected_index >= self.points.len() {
            self.selected_index = self.points.len() - 1;
        }
        self.dirty = true;
        true
    }

    /// Reset to default fan curve (5 points, 0–100°C).
    pub fn reset(&mut self) {
        let defaults = tux_core::fan_curve::FanConfig::default().curve;
        self.points = defaults;
        self.selected_index = 0;
        self.dirty = true;
    }

    /// Revert to the last-loaded points (undo unsaved changes).
    pub fn revert(&mut self) {
        self.points = self.original_points.clone();
        self.selected_index = 0;
        self.dirty = false;
    }
}

// ── Generic Form Widget State ───────────────────────────────

/// A reusable form with labelled, typed fields.
#[allow(dead_code)]
pub struct Form {
    pub fields: Vec<FormField>,
    pub selected_index: usize,
    pub dirty: bool,
    /// Snapshot for reset on Esc.
    original_values: Vec<FieldType>,
}

/// A single field in a form.
#[allow(dead_code)]
pub struct FormField {
    pub label: String,
    /// TOML key used for D-Bus serialization. If `None`, derived from label.
    pub key: Option<String>,
    pub field_type: FieldType,
    pub enabled: bool,
}

/// Type-safe field value.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum FieldType {
    Text(String),
    Number {
        value: i64,
        min: i64,
        max: i64,
        step: i64,
    },
    Bool(bool),
    Select {
        options: Vec<String>,
        selected: usize,
    },
}

#[allow(dead_code)]
impl Form {
    pub fn new(fields: Vec<FormField>) -> Self {
        let original_values: Vec<FieldType> = fields.iter().map(|f| f.field_type.clone()).collect();
        Self {
            fields,
            selected_index: 0,
            dirty: false,
            original_values,
        }
    }

    /// Move to next enabled field.
    pub fn select_next(&mut self) {
        let len = self.fields.len();
        if len == 0 {
            return;
        }
        for i in 1..=len {
            let idx = (self.selected_index + i) % len;
            if self.fields[idx].enabled {
                self.selected_index = idx;
                return;
            }
        }
    }

    /// Move to previous enabled field.
    pub fn select_prev(&mut self) {
        let len = self.fields.len();
        if len == 0 {
            return;
        }
        for i in 1..=len {
            let idx = (self.selected_index + len - i) % len;
            if self.fields[idx].enabled {
                self.selected_index = idx;
                return;
            }
        }
    }

    /// Adjust the selected field value (← / →).
    pub fn adjust(&mut self, delta: i64) {
        if let Some(field) = self.fields.get_mut(self.selected_index) {
            if !field.enabled {
                return;
            }
            match &mut field.field_type {
                FieldType::Number {
                    value,
                    min,
                    max,
                    step,
                } => {
                    let increment = delta.saturating_mul(*step);
                    *value = value.saturating_add(increment).clamp(*min, *max);
                    self.dirty = true;
                }
                FieldType::Bool(b) => {
                    *b = !*b;
                    self.dirty = true;
                }
                FieldType::Select { options, selected } => {
                    if !options.is_empty() {
                        let len = options.len() as i64;
                        *selected = ((*selected as i64 + delta).rem_euclid(len)) as usize;
                        self.dirty = true;
                    }
                }
                FieldType::Text(_) => {} // Text editing via Enter (not arrow keys).
            }
        }
    }

    /// Toggle the selected field (Space).
    pub fn toggle(&mut self) {
        if let Some(field) = self.fields.get_mut(self.selected_index) {
            if !field.enabled {
                return;
            }
            match &mut field.field_type {
                FieldType::Bool(b) => {
                    *b = !*b;
                    self.dirty = true;
                }
                FieldType::Select { options, selected } => {
                    if !options.is_empty() {
                        *selected = (*selected + 1) % options.len();
                        self.dirty = true;
                    }
                }
                _ => {}
            }
        }
    }

    /// Discard changes, restore original values.
    pub fn discard(&mut self) {
        for (field, orig) in self.fields.iter_mut().zip(self.original_values.iter()) {
            field.field_type = orig.clone();
        }
        self.dirty = false;
    }

    /// Mark as saved: current values become the new original.
    pub fn mark_saved(&mut self) {
        self.original_values = self.fields.iter().map(|f| f.field_type.clone()).collect();
        self.dirty = false;
    }
}

const HISTORY_LEN: usize = 60;

impl DashboardState {
    /// Fallback max RPM when FanInfo hasn't been received yet.
    pub const DEFAULT_MAX_RPM: u32 = 6000;

    pub fn new() -> Self {
        Self {
            fan_data: Vec::new(),
            cpu_temp: None,
            power_state: "unknown".to_string(),
            temp_history: VecDeque::with_capacity(HISTORY_LEN),
            speed_history: VecDeque::with_capacity(HISTORY_LEN),
            load_history: VecDeque::with_capacity(HISTORY_LEN),
            num_fans: 0,
            max_rpm: Self::DEFAULT_MAX_RPM,
            cpu_freq_mhz: None,
            core_count: None,
            active_profile: None,
            cpu_load_overall: None,
            cpu_load_per_core: Vec::new(),
            cpu_freq_per_core: Vec::new(),
            fan_health: None,
        }
    }

    /// Push a temperature reading, keeping history at HISTORY_LEN.
    pub fn push_temp(&mut self, temp: f32) {
        if self.temp_history.len() >= HISTORY_LEN {
            self.temp_history.pop_front();
        }
        self.temp_history.push_back(temp);
    }

    /// Push average fan speed percentage, keeping history at HISTORY_LEN.
    pub fn push_speed(&mut self, pct: f32) {
        if self.speed_history.len() >= HISTORY_LEN {
            self.speed_history.pop_front();
        }
        self.speed_history.push_back(pct);
    }

    /// Push overall CPU load percentage, keeping history at HISTORY_LEN.
    pub fn push_load(&mut self, pct: f32) {
        if self.load_history.len() >= HISTORY_LEN {
            self.load_history.pop_front();
        }
        self.load_history.push_back(pct);
    }
}

impl Model {
    pub fn new() -> Self {
        let mut model = Self {
            current_tab: Tab::Dashboard,
            should_quit: false,
            show_help: false,
            connection_status: ConnectionStatus::Connecting,
            terminal_size: (80, 24),
            needs_render: true,
            dashboard: DashboardState::new(),
            info: InfoState::default(),
            fan_curve: FanCurveState::new(),
            profiles: ProfilesState::new(),
            settings: settings_form(),
            keyboard: keyboard_form(),
            charging: charging_form(),
            power: PowerState::new(),
            display: display_form(),
            webcam: WebcamState::new(),
            event_log: EventLogState::new(),
        };
        model.log_event(
            EventSource::System,
            "tux-tui started",
            Some("event log initialized".to_string()),
        );
        model
    }

    pub fn log_event(
        &mut self,
        source: EventSource,
        summary: impl Into<String>,
        detail: Option<String>,
    ) {
        self.event_log.push(source, summary, detail);
    }

    pub fn log_debug_event(
        &mut self,
        source: EventSource,
        summary: impl Into<String>,
        detail: Option<String>,
    ) {
        self.event_log.push_debug(source, summary, detail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_next_wraps() {
        assert_eq!(Tab::Info.next(), Tab::EventLog);
        assert_eq!(Tab::EventLog.next(), Tab::Dashboard);
        assert_eq!(Tab::Dashboard.next(), Tab::Profiles);
    }

    #[test]
    fn tab_prev_wraps() {
        assert_eq!(Tab::Dashboard.prev(), Tab::EventLog);
        assert_eq!(Tab::EventLog.prev(), Tab::Info);
        assert_eq!(Tab::Profiles.prev(), Tab::Dashboard);
    }

    #[test]
    fn all_tabs_have_labels() {
        for tab in Tab::ALL {
            assert!(!tab.label().is_empty());
        }
    }

    #[test]
    fn model_defaults() {
        let m = Model::new();
        assert_eq!(m.current_tab, Tab::Dashboard);
        assert!(!m.should_quit);
        assert!(!m.show_help);
        assert_eq!(m.connection_status, ConnectionStatus::Connecting);
        assert!(!m.event_log.entries.is_empty());
        assert!(!m.event_log.show_debug_events);
    }

    #[test]
    fn event_log_retention_is_bounded() {
        let mut state = EventLogState::with_max_entries(3);
        state.push(EventSource::System, "e1", None);
        state.push(EventSource::System, "e2", None);
        state.push(EventSource::System, "e3", None);
        state.push(EventSource::System, "e4", None);

        assert_eq!(state.entries.len(), 3);
        assert_eq!(
            state.entries.front().map(|e| e.summary.as_str()),
            Some("e2")
        );
        assert_eq!(state.entries.back().map(|e| e.summary.as_str()), Some("e4"));
    }

    #[test]
    fn history_wraps_at_60() {
        let mut ds = DashboardState::new();
        for i in 0..70 {
            ds.push_temp(i as f32);
        }
        assert_eq!(ds.temp_history.len(), HISTORY_LEN);
        // First value should be 10 (the oldest after wrapping past 60).
        assert_eq!(ds.temp_history[0] as u32, 10);
    }

    #[test]
    fn speed_history_wraps() {
        let mut ds = DashboardState::new();
        for i in 0..65 {
            ds.push_speed(i as f32);
        }
        assert_eq!(ds.speed_history.len(), HISTORY_LEN);
        assert_eq!(ds.speed_history[0] as u32, 5);
    }

    #[test]
    fn fan_data_default() {
        let fd = FanData::default();
        assert_eq!(fd.rpm, 0);
        assert_eq!(fd.speed_percent, 0);
    }

    // ── FanCurveState tests ──

    #[test]
    fn fan_curve_increase_speed_caps_at_100() {
        let mut fc = FanCurveState::new();
        fc.points = vec![FanCurvePoint {
            temp: 90,
            speed: 98,
        }];
        fc.selected_index = 0;
        fc.increase_speed();
        assert_eq!(fc.points[0].speed, 100);
        assert!(fc.dirty);
    }

    #[test]
    fn fan_curve_decrease_speed_floors_at_0() {
        let mut fc = FanCurveState::new();
        fc.points = vec![FanCurvePoint { temp: 40, speed: 3 }];
        fc.selected_index = 0;
        fc.decrease_speed();
        assert_eq!(fc.points[0].speed, 0);
        assert!(fc.dirty);
    }

    #[test]
    fn fan_curve_insert_creates_midpoint() {
        let mut fc = FanCurveState::new();
        fc.points = vec![
            FanCurvePoint { temp: 40, speed: 0 },
            FanCurvePoint {
                temp: 80,
                speed: 80,
            },
        ];
        fc.selected_index = 0;
        fc.insert_point();
        assert_eq!(fc.points.len(), 3);
        assert_eq!(fc.points[1].temp, 60);
        assert_eq!(fc.points[1].speed, 40);
        assert_eq!(fc.selected_index, 1);
        assert!(fc.dirty);
    }

    #[test]
    fn fan_curve_delete_with_min_points_fails() {
        let mut fc = FanCurveState::new();
        fc.points = vec![
            FanCurvePoint { temp: 40, speed: 0 },
            FanCurvePoint {
                temp: 90,
                speed: 100,
            },
        ];
        assert!(!fc.delete_point());
        assert_eq!(fc.points.len(), 2);
    }

    #[test]
    fn fan_curve_revert_restores_original() {
        let mut fc = FanCurveState::new();
        let orig_len = fc.points.len();
        fc.increase_speed();
        fc.insert_point();
        assert!(fc.dirty);
        fc.revert();
        assert!(!fc.dirty);
        assert_eq!(fc.points.len(), orig_len);
        assert_eq!(fc.selected_index, 0);
    }

    #[test]
    fn fan_curve_save_marks_not_dirty() {
        let mut fc = FanCurveState::new();
        fc.increase_speed();
        assert!(fc.dirty);
        // Simulate save: update originals.
        fc.original_points = fc.points.clone();
        fc.dirty = false;
        assert!(!fc.dirty);
    }

    // ── Form tests ──

    #[test]
    fn form_number_respects_bounds() {
        let mut form = Form::new(vec![FormField {
            label: "Speed".into(),
            key: None,
            field_type: FieldType::Number {
                value: 95,
                min: 0,
                max: 100,
                step: 10,
            },
            enabled: true,
        }]);
        form.adjust(1); // +10 → 105 → clamped to 100
        assert_eq!(
            form.fields[0].field_type,
            FieldType::Number {
                value: 100,
                min: 0,
                max: 100,
                step: 10
            }
        );
    }

    #[test]
    fn form_select_wraps_around() {
        let mut form = Form::new(vec![FormField {
            label: "Unit".into(),
            key: None,
            field_type: FieldType::Select {
                options: vec!["Celsius".into(), "Fahrenheit".into()],
                selected: 1,
            },
            enabled: true,
        }]);
        form.adjust(1); // wraps to 0
        match &form.fields[0].field_type {
            FieldType::Select { selected, .. } => assert_eq!(*selected, 0),
            _ => panic!("expected Select"),
        }
    }

    #[test]
    fn form_bool_toggles_on_space() {
        let mut form = Form::new(vec![FormField {
            label: "Enabled".into(),
            key: None,
            field_type: FieldType::Bool(false),
            enabled: true,
        }]);
        form.toggle();
        assert_eq!(form.fields[0].field_type, FieldType::Bool(true));
        assert!(form.dirty);
    }

    #[test]
    fn form_dirty_on_change() {
        let mut form = Form::new(vec![FormField {
            label: "Val".into(),
            key: None,
            field_type: FieldType::Number {
                value: 50,
                min: 0,
                max: 100,
                step: 1,
            },
            enabled: true,
        }]);
        assert!(!form.dirty);
        form.adjust(1);
        assert!(form.dirty);
    }

    #[test]
    fn form_esc_resets_to_original() {
        let mut form = Form::new(vec![FormField {
            label: "Val".into(),
            key: None,
            field_type: FieldType::Number {
                value: 50,
                min: 0,
                max: 100,
                step: 1,
            },
            enabled: true,
        }]);
        form.adjust(5);
        assert!(form.dirty);
        form.discard();
        assert!(!form.dirty);
        assert_eq!(
            form.fields[0].field_type,
            FieldType::Number {
                value: 50,
                min: 0,
                max: 100,
                step: 1
            }
        );
    }

    // ── Edge case tests ──

    #[test]
    fn fan_curve_insert_at_max_points_is_noop() {
        let mut fc = FanCurveState::new();
        fc.points = (0..20)
            .map(|i| FanCurvePoint {
                temp: i * 5,
                speed: i * 5,
            })
            .collect();
        fc.selected_index = 0;
        fc.insert_point();
        assert_eq!(fc.points.len(), 20);
        assert!(!fc.dirty);
    }

    #[test]
    fn fan_curve_insert_at_last_point_is_noop() {
        let mut fc = FanCurveState::new();
        fc.points = vec![
            FanCurvePoint { temp: 40, speed: 0 },
            FanCurvePoint {
                temp: 90,
                speed: 100,
            },
        ];
        fc.selected_index = 1; // Last point.
        fc.insert_point();
        assert_eq!(fc.points.len(), 2);
        assert!(!fc.dirty);
    }

    #[test]
    fn form_select_next_all_disabled() {
        let mut form = Form::new(vec![
            FormField {
                label: "A".into(),
                key: None,
                field_type: FieldType::Bool(true),
                enabled: false,
            },
            FormField {
                label: "B".into(),
                key: None,
                field_type: FieldType::Bool(false),
                enabled: false,
            },
        ]);
        let before = form.selected_index;
        form.select_next();
        // Should not advance when all disabled.
        assert_eq!(form.selected_index, before);
    }

    #[test]
    fn form_adjust_disabled_field_is_noop() {
        let mut form = Form::new(vec![FormField {
            label: "Val".into(),
            key: None,
            field_type: FieldType::Number {
                value: 50,
                min: 0,
                max: 100,
                step: 1,
            },
            enabled: false,
        }]);
        form.adjust(5);
        assert!(!form.dirty);
        assert_eq!(
            form.fields[0].field_type,
            FieldType::Number {
                value: 50,
                min: 0,
                max: 100,
                step: 1
            }
        );
    }

    // ── ProfilesState tests ──

    #[test]
    fn profiles_select_wraps() {
        let mut ps = ProfilesState::new();
        ps.profiles = tux_core::profile::builtin_profiles();
        assert_eq!(ps.selected_index, 0);
        ps.select_next();
        assert_eq!(ps.selected_index, 1);
        ps.select_prev();
        assert_eq!(ps.selected_index, 0);
        ps.select_prev(); // wraps
        assert_eq!(ps.selected_index, 3);
    }

    #[test]
    fn profiles_select_empty_is_noop() {
        let mut ps = ProfilesState::new();
        ps.select_next();
        assert_eq!(ps.selected_index, 0);
        ps.select_prev();
        assert_eq!(ps.selected_index, 0);
    }

    #[test]
    fn profiles_selected_profile_returns_correct() {
        let mut ps = ProfilesState::new();
        ps.profiles = tux_core::profile::builtin_profiles();
        ps.selected_index = 2;
        assert_eq!(ps.selected_profile().unwrap().id, "__office__");
    }

    #[test]
    fn profiles_build_editor_form_default_readonly() {
        let profiles = tux_core::profile::builtin_profiles();
        let form = ProfilesState::build_editor_form(&profiles[0]);
        // All fields disabled for default profiles.
        for field in &form.fields {
            assert!(!field.enabled, "field '{}' should be disabled", field.label);
        }
    }

    #[test]
    fn profiles_build_editor_form_custom_editable() {
        let mut profile = tux_core::profile::builtin_profiles()[0].clone();
        profile.is_default = false;
        let form = ProfilesState::build_editor_form(&profile);
        for field in &form.fields {
            assert!(field.enabled, "field '{}' should be enabled", field.label);
        }
    }

    #[test]
    fn profiles_apply_form_roundtrip() {
        let profiles = tux_core::profile::builtin_profiles();
        let profile = &profiles[2]; // Office
        let form = ProfilesState::build_editor_form(profile);
        let result = ProfilesState::apply_form_to_profile(&form, profile);
        assert_eq!(result.name, profile.name);
        assert_eq!(result.fan.enabled, profile.fan.enabled);
        assert_eq!(result.cpu.governor, profile.cpu.governor);
    }
}
