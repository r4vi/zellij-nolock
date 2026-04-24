use std::collections::BTreeMap;
use zellij_tile::prelude::*;

const CAFFEINATE_TIMEOUT_SECS: u64 = 120;
const INACTIVITY_CHECK_SECS: f64 = 60.0;
const DEFAULT_INACTIVITY_TIMEOUT_SECS: f64 = 300.0;

#[derive(Default, Clone, Copy, PartialEq)]
enum Mode {
    Manual,
    #[default]
    Auto,
}

#[derive(Default)]
struct State {
    active: bool,
    mode: Mode,
    caffeinate_running: bool,
    generation: u64,
    timer_running: bool,
    last_activity_generation: u64,
    tick_generation: u64,
    inactivity_ticks: u64,
    last_error: Option<String>,
    permissions_granted: bool,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::RunCommands,
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);

        subscribe(&[
            EventType::Key,
            EventType::Timer,
            EventType::RunCommandResult,
            EventType::PermissionRequestResult,
        ]);

        set_selectable(false);

        if let Some(mode_str) = configuration.get("mode") {
            if mode_str == "manual" {
                self.mode = Mode::Manual;
            }
        }

        let inactivity_timeout = configuration
            .get("inactivity_timeout")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(DEFAULT_INACTIVITY_TIMEOUT_SECS);

        self.inactivity_ticks = (inactivity_timeout / INACTIVITY_CHECK_SECS).ceil() as u64;
        if self.inactivity_ticks == 0 {
            self.inactivity_ticks = 1;
        }
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(status) => {
                if status == PermissionStatus::Granted {
                    self.permissions_granted = true;
                    subscribe(&[EventType::PaneUpdate]);
                    if self.mode == Mode::Manual {
                        self.activate();
                    }
                } else {
                    self.last_error =
                        Some("Permissions denied - grant RunCommands to enable".to_string());
                }
                true
            }
            Event::Key(key) => self.handle_key(key),
            Event::Timer(_elapsed) => {
                self.handle_timer();
                true
            }
            Event::RunCommandResult(exit_code, _stdout, stderr, context) => {
                self.handle_command_result(exit_code, stderr, context)
            }
            Event::PaneUpdate(_pane_manifest) => {
                self.handle_pane_update();
                self.mode == Mode::Auto
            }
            _ => false,
        }
    }

    fn render(&mut self, _rows: usize, cols: usize) {
        let indicator = if !self.permissions_granted {
            "⏳"
        } else if self.active {
            "☕"
        } else {
            "💤"
        };

        let text = Text::new(indicator);
        print_text_with_coordinates(text, 0, cols.saturating_sub(2), None, None);
    }
}

impl State {
    fn activate(&mut self) {
        if self.active || !self.permissions_granted {
            return;
        }
        self.active = true;
        self.last_error = None;
        if !self.caffeinate_running {
            self.issue_caffeinate();
        }
        self.ensure_timer();
    }

    fn deactivate(&mut self) {
        self.active = false;
    }

    fn issue_caffeinate(&mut self) {
        self.generation += 1;
        self.caffeinate_running = true;
        let context = BTreeMap::from([
            ("op".to_string(), "caffeinate".to_string()),
            ("gen".to_string(), self.generation.to_string()),
        ]);
        let timeout_str = CAFFEINATE_TIMEOUT_SECS.to_string();
        run_command(&["caffeinate", "-d", "-t", &timeout_str], context);
    }

    fn ensure_timer(&mut self) {
        if !self.timer_running {
            self.timer_running = true;
            set_timeout(INACTIVITY_CHECK_SECS);
        }
    }

    fn handle_key(&mut self, key: KeyWithModifier) -> bool {
        match key.bare_key {
            BareKey::Char(' ') | BareKey::Enter if key.has_no_modifiers() => {
                if self.active {
                    self.deactivate();
                } else {
                    self.activate();
                }
                true
            }
            BareKey::Char('m') if key.has_no_modifiers() => {
                self.mode = match self.mode {
                    Mode::Manual => {
                        self.last_activity_generation = self.tick_generation;
                        Mode::Auto
                    }
                    Mode::Auto => Mode::Manual,
                };
                true
            }
            BareKey::Char('q') | BareKey::Esc if key.has_no_modifiers() => {
                self.deactivate();
                close_self();
                false
            }
            _ => false,
        }
    }

    fn handle_timer(&mut self) {
        self.timer_running = false;
        self.tick_generation += 1;

        if self.mode == Mode::Auto && self.active {
            let ticks_since_activity = self
                .tick_generation
                .saturating_sub(self.last_activity_generation);
            if ticks_since_activity >= self.inactivity_ticks {
                self.deactivate();
                return;
            }
        }

        if self.active || self.caffeinate_running {
            self.ensure_timer();
        }
    }

    fn handle_command_result(
        &mut self,
        exit_code: Option<i32>,
        stderr: Vec<u8>,
        context: BTreeMap<String, String>,
    ) -> bool {
        if context.get("op").map(String::as_str) != Some("caffeinate") {
            return false;
        }

        self.caffeinate_running = false;

        match exit_code {
            Some(0) => {
                self.last_error = None;
            }
            Some(code) => {
                let err_msg = String::from_utf8_lossy(&stderr);
                self.last_error = Some(format!("caffeinate exit {}: {}", code, err_msg.trim()));
            }
            None => {
                self.last_error = Some("caffeinate killed by signal".to_string());
            }
        }

        if self.active {
            self.issue_caffeinate();
        }

        true
    }

    fn handle_pane_update(&mut self) {
        if self.mode != Mode::Auto {
            return;
        }

        self.last_activity_generation = self.tick_generation;

        if !self.active {
            self.activate();
        }
    }
}
