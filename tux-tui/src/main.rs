//! tux-tui: Terminal UI for TUXEDO laptop control.

mod cli;
mod command;
mod dbus_task;
mod event;
mod model;
mod update;
mod view;
mod views;
mod widgets;

use std::io::{self, stdout};
use std::time::Duration;

use crossterm::ExecutableCommand;
use crossterm::event::{Event, EventStream};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures_util::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

use event::AppEvent;
use model::Model;

/// Commands sent to the D-Bus executor task.
enum DbusCommand {
    SaveFanCurve(Vec<tux_core::fan_curve::FanCurvePoint>),
    FetchFanCurve,
    FetchProfiles,
    CopyProfile(String),
    CreateProfile(String),
    DeleteProfile(String),
    SaveProfile { id: String, toml: String },
    SetActiveProfile { id: String, state: String },
    SaveSettings(String),
    SaveKeyboard(String),
    SaveCharging(String),
    SavePower(String),
    SaveDisplay(String),
    SaveWebcam { device: String, toml: String },
}

/// CLI arguments.
struct Args {
    /// Connect to session bus instead of system bus (for development).
    session_bus: bool,
    /// Optional starting tab.
    initial_tab: Option<model::Tab>,
}

fn parse_args() -> Args {
    let mut session_bus = false;
    let mut initial_tab = None;
    let mut args = std::env::args().skip(1).peekable();

    while let Some(arg) = args.next() {
        if arg == "--session" {
            session_bus = true;
        } else if let Some(val) = arg.strip_prefix("--tab=") {
            initial_tab = parse_tab_arg(val);
        } else if (arg == "-t" || arg == "--tab")
            && let Some(val) = args.next()
        {
            initial_tab = parse_tab_arg(&val);
        }
    }
    Args {
        session_bus,
        initial_tab,
    }
}

fn parse_tab_arg(s: &str) -> Option<model::Tab> {
    match s.to_lowercase().replace('-', "").as_str() {
        "dashboard" => Some(model::Tab::Dashboard),
        "profiles" => Some(model::Tab::Profiles),
        "fancurve" => Some(model::Tab::FanCurve),
        "settings" => Some(model::Tab::Settings),
        "keyboard" => Some(model::Tab::Keyboard),
        "charging" => Some(model::Tab::Charging),
        "power" => Some(model::Tab::Power),
        "display" => Some(model::Tab::Display),
        "webcam" => Some(model::Tab::Webcam),
        "info" => Some(model::Tab::Info),
        _ => {
            eprintln!("Warning: unknown tab '{}', defaulting to Dashboard", s);
            None
        }
    }
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let args = parse_args();

    // CLI mode: dump state and exit without opening a terminal.
    let all_args: Vec<String> = std::env::args().collect();
    if let Some(cli_cmd) = cli::parse_cli_command(&all_args) {
        match cli::run_cli(cli_cmd, args.session_bus).await {
            Ok(output) => {
                println!("{output}");
                return Ok(());
            }
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }

    let mut terminal = setup_terminal()?;
    let result = run_app(&mut terminal, args).await;
    restore_terminal(&mut terminal);

    result
}

fn setup_terminal() -> color_eyre::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    // Install a panic hook that restores the terminal before printing the backtrace.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        default_hook(info);
    }));

    let backend = CrosstermBackend::new(stdout());
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Best-effort terminal restore; never fails the exit.
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) {
    let _ = disable_raw_mode();
    let _ = terminal.backend_mut().execute(LeaveAlternateScreen);
    let _ = terminal.show_cursor();
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    args: Args,
) -> color_eyre::Result<()> {
    let mut model = Model::new();
    if let Some(tab) = args.initial_tab {
        model.current_tab = tab;
    }

    // Event channel.
    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(64);

    // Command channel for D-Bus operations.
    let (cmd_tx, cmd_rx) = mpsc::channel::<DbusCommand>(16);

    // Spawn D-Bus connection + polling task.
    let dbus_tx = event_tx.clone();
    let session_bus = args.session_bus;
    let dbus_handle = tokio::spawn(async move {
        dbus_task::run_dbus_task(session_bus, dbus_tx, cmd_rx).await;
    });

    // Spawn crossterm event reader.
    let input_tx = event_tx.clone();
    let input_handle = tokio::spawn(async move {
        let mut reader = EventStream::new();
        while let Some(Ok(event)) = reader.next().await {
            let app_event = match event {
                Event::Key(key) => {
                    // Only handle Press events (avoid duplicate Release on some terminals).
                    if key.kind != crossterm::event::KeyEventKind::Press {
                        continue;
                    }
                    AppEvent::Key(key)
                }
                Event::Resize(w, h) => AppEvent::Resize(w, h),
                _ => continue,
            };
            if input_tx.send(app_event).await.is_err() {
                break;
            }
        }
    });

    // Spawn tick timer (frame clock for telemetry-driven renders).
    let tick_tx = event_tx;
    let tick_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            if tick_tx.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    });

    // Initial render.
    terminal.draw(|frame| view::render(frame, &model))?;

    // Main event loop: recv → drain → process all → render if needed.
    loop {
        // Block until at least one event arrives.
        let first = match event_rx.recv().await {
            Some(ev) => ev,
            None => break,
        };

        // Drain all remaining queued events to batch processing.
        let mut events = vec![first];
        while let Ok(ev) = event_rx.try_recv() {
            events.push(ev);
        }

        // Process all collected events; track whether any are interactive (key/resize)
        // and whether the periodic render clock ticked.
        let mut has_interactive = false;
        let mut has_render_tick = false;
        for event in events {
            match event {
                AppEvent::Key(key) => {
                    has_interactive = true;
                    let commands = update::handle_key(&mut model, key);
                    for cmd in commands {
                        dispatch_command(&cmd_tx, cmd).await;
                    }
                }
                AppEvent::DbusData(data) => {
                    update::handle_data(&mut model, data);
                }
                AppEvent::Resize(w, h) => {
                    has_interactive = true;
                    model.needs_render = true;
                    model.terminal_size = (w, h);
                }
                AppEvent::Tick => {
                    // Tick is the frame clock for periodic repaint.
                    has_render_tick = true;
                }
            }
        }

        // Render immediately on interaction/data changes and also on each periodic tick.
        if has_interactive || has_render_tick || model.needs_render {
            terminal.draw(|frame| view::render(frame, &model))?;
            model.needs_render = false;
        }

        if model.should_quit {
            break;
        }
    }

    // Clean shutdown: drop the receiver so all sender tasks detect closure and exit.
    drop(event_rx);
    dbus_handle.abort();
    input_handle.abort();
    tick_handle.abort();

    Ok(())
}

/// Send a TUI command to the D-Bus executor task.
async fn dispatch_command(cmd_tx: &mpsc::Sender<DbusCommand>, cmd: command::Command) {
    match cmd {
        command::Command::Quit | command::Command::None => {}
        command::Command::SaveFanCurve(points) => {
            let _ = cmd_tx.send(DbusCommand::SaveFanCurve(points)).await;
        }
        command::Command::FetchFanCurve => {
            let _ = cmd_tx.send(DbusCommand::FetchFanCurve).await;
        }
        command::Command::FetchProfiles => {
            let _ = cmd_tx.send(DbusCommand::FetchProfiles).await;
        }
        command::Command::CopyProfile(id) => {
            let _ = cmd_tx.send(DbusCommand::CopyProfile(id)).await;
        }
        command::Command::CreateProfile(toml) => {
            let _ = cmd_tx.send(DbusCommand::CreateProfile(toml)).await;
        }
        command::Command::DeleteProfile(id) => {
            let _ = cmd_tx.send(DbusCommand::DeleteProfile(id)).await;
        }
        command::Command::SaveProfile { id, toml } => {
            let _ = cmd_tx.send(DbusCommand::SaveProfile { id, toml }).await;
        }
        command::Command::SetActiveProfile { id, state } => {
            let _ = cmd_tx
                .send(DbusCommand::SetActiveProfile { id, state })
                .await;
        }
        command::Command::SaveSettings(toml) => {
            let _ = cmd_tx.send(DbusCommand::SaveSettings(toml)).await;
        }
        command::Command::SaveKeyboard(toml) => {
            let _ = cmd_tx.send(DbusCommand::SaveKeyboard(toml)).await;
        }
        command::Command::SaveCharging(toml) => {
            let _ = cmd_tx.send(DbusCommand::SaveCharging(toml)).await;
        }
        command::Command::SavePower(toml) => {
            let _ = cmd_tx.send(DbusCommand::SavePower(toml)).await;
        }
        command::Command::SaveDisplay(toml) => {
            let _ = cmd_tx.send(DbusCommand::SaveDisplay(toml)).await;
        }
        command::Command::SaveWebcam { device, toml } => {
            let _ = cmd_tx.send(DbusCommand::SaveWebcam { device, toml }).await;
        }
    }
}
