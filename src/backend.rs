// Core logic for the thermostat to do all the things that
// the ui cant, like reading the temp and sending events to the ui.

use std::{time::{Duration, Instant}};
use std::{
    sync::mpsc::{self, Receiver, Sender, SyncSender},
    sync::Arc,
    thread,
};
use crate::{controller::Controller, events::{BackendEvent, DiffStatus, FanStatus, ModeStatus, RestStatus, UiEvent}};


const REST_DURATION_MINS: u64 = 30;

pub struct ThermostatState {
    ui_events_rx: Receiver<UiEvent>,
    actor_events_tx: Sender<BackendEvent>,
    /// Current temperature in Celsius (base unit)
    current_temp_c: f32,
    /// Target temperature in Celsius (base unit)
    target_temp_c: f32,
    mode: ModeStatus,
    diff_mode: DiffStatus,
    rest_mode: RestStatus,
    fan_mode: FanStatus,
    use_fahrenheit: bool,

    runtime_state: ThermostatRuntimeState,


    /// Used to track cumulative cooling duration since last resting
    total_cooling_duration: Duration,
    /// unused, just nice to have a counterpart
    total_heating_duration: Duration,
    
    last_resting_start_time: Instant,

    /// Used to debounce user interaction and prevent rapid changes in mode.
    last_user_interaction_time: Instant,

    /// Used to track time passed since last run was called. Can be appended to durations
    last_run_finished_time: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ThermostatRuntimeState {
    Waiting,
    Heating,
    Cooling,
    Resting,
    Idle,
}

impl ThermostatState {
    pub fn new(ui_events_rx: Receiver<UiEvent>, actor_events_tx: Sender<BackendEvent>) -> Self {
        Self {
            ui_events_rx,
            actor_events_tx,
            current_temp_c: 21.0,  // ~70°F
            target_temp_c: 21.0,   // ~70°F
            mode: ModeStatus::Off,
            diff_mode: DiffStatus::Normal,
            rest_mode: RestStatus::Off,
            fan_mode: FanStatus::Auto,
            use_fahrenheit: true,
            runtime_state: ThermostatRuntimeState::Waiting,
            total_cooling_duration: Duration::from_secs(0),
            total_heating_duration: Duration::from_secs(0),
            last_resting_start_time: Instant::now(),
            last_user_interaction_time: Instant::now(),
            last_run_finished_time: Instant::now(),
        }
    }

    /// Get target temp needed to transition from waiting mode to heating or cooling mode (in Celsius)
    pub fn get_waiting_target_temp(&self) -> f32 {
        match self.mode {
            ModeStatus::Heat => {
                match self.diff_mode {
                    // Differential offsets in Celsius
                    DiffStatus::Slow => self.target_temp_c - 1.0,    // ~1.9°F
                    DiffStatus::Normal => self.target_temp_c - 0.4, // ~0.75°F
                    DiffStatus::Fast => self.target_temp_c - 0.3    // ~0.5°F
                }
            },
            ModeStatus::Cool => {
                match self.diff_mode {
                    // Differential offsets in Celsius
                    DiffStatus::Slow => self.target_temp_c + 0.9,   // ~1.7°F
                    DiffStatus::Normal => self.target_temp_c + 0.7, // ~1.2°F
                    DiffStatus::Fast => self.target_temp_c + 0.5    // ~0.9°F
                }
            },
            ModeStatus::Off => self.current_temp_c,
        }
    }

    /// We need to rest for a while after cooling to prevent the compressor from freezing,
    /// since we don't have enough airflow to prevent it.
    pub fn should_rest(&self) -> bool {
        if let ModeStatus::Cool = self.mode {
            return match self.rest_mode {
                RestStatus::Short => self.total_cooling_duration > Duration::from_mins(60),
                RestStatus::Medium => self.total_cooling_duration > Duration::from_mins(90),
                RestStatus::Long => self.total_cooling_duration > Duration::from_mins(120),
                RestStatus::Off => false,
            }
        }
        false
    }

    /// Formats the temperature (base unit: Celsius) in the user's preferred unit
    pub fn format_temp(&self, temp_c: f32) -> String {
        if self.use_fahrenheit {
            format!("{:.1}°F", Controller::celsius_to_fahrenheit(temp_c))
        } else {
            format!("{:.1}°C", temp_c)
        }
    }

    pub fn format_time(duration: Duration) -> String {
        let minutes = duration.as_secs() / 60;
        let seconds = duration.as_secs() % 60;
        format!("{}m {}s", minutes, seconds)
    }

    pub fn get_waiting_temp_formatted(&self) -> String {
        return self.format_temp(self.get_waiting_target_temp());
    }

    pub fn get_remaining_resting_duration_formatted(&self) -> String {
        let elapsed = self.last_resting_start_time.elapsed();
        let remaining = Duration::from_mins(REST_DURATION_MINS) - elapsed;
        return Self::format_time(remaining);
    }

    pub fn get_status_message(&self) -> String {
        match self.runtime_state {
            ThermostatRuntimeState::Waiting => format!("Waiting for {}", self.get_waiting_temp_formatted()),
            ThermostatRuntimeState::Heating => "Heating".to_string(),
            ThermostatRuntimeState::Cooling => "Cooling".to_string(),
            ThermostatRuntimeState::Resting => format!("Defrosting for {}", self.get_remaining_resting_duration_formatted()),
            ThermostatRuntimeState::Idle => "Idling".to_string(),
        }
    }

    pub fn set_mode(&mut self, mode: ModeStatus) {
        self.mode = mode;
    }

    pub fn set_rest_mode(&mut self, rest_mode: RestStatus) {
        self.rest_mode = rest_mode;
    }
    
    pub fn set_fan_mode(&mut self, fan_mode: FanStatus) {
        self.fan_mode = fan_mode;
    }

    /// Set target temperature in Celsius
    pub fn set_target_temp(&mut self, target_temp_c: f32) {
        self.target_temp_c = target_temp_c;
    }

    /// Receives events from the UI thread and updates the state accordingly.
    pub fn receive_events(&mut self) {
        if !(self.last_user_interaction_time.elapsed() > Duration::from_secs(5)) {
            return;
        }

        while let Ok(event) = self.ui_events_rx.try_recv() {
            match event {
                UiEvent::ModeUpdate(mode) => self.mode = mode,
                UiEvent::UseFahrenheitUpdate(use_fahrenheit) => self.use_fahrenheit = use_fahrenheit,
                UiEvent::DiffUpdate(diff_mode) => self.diff_mode = diff_mode,
                UiEvent::RestUpdate(rest_mode) => self.rest_mode = rest_mode,
                UiEvent::FanUpdate(fan_mode) => self.fan_mode = fan_mode,
                UiEvent::TargetTempUpdate(target_temp_c) => self.target_temp_c = target_temp_c,
            }
        }
        self.last_user_interaction_time = Instant::now();
    }

    fn start_heating(&mut self, controller: &mut Controller) {
        self.runtime_state = ThermostatRuntimeState::Heating;
        controller.set_heating(true);
        controller.set_cooling(false);
        controller.set_fan(true);
    }

    fn start_cooling(&mut self, controller: &mut Controller) {
        self.runtime_state = ThermostatRuntimeState::Cooling;
        controller.set_cooling(true);
        controller.set_heating(false);
        controller.set_fan(true);
    }

    fn start_idle(&mut self, controller: &mut Controller) {
        self.runtime_state = ThermostatRuntimeState::Idle;
        controller.set_heating(false);
        controller.set_cooling(false);
        // Turn fan off if in auto mode. Will always be turned back on when in heating or cooling mode.
        if self.fan_mode == FanStatus::Auto {
            controller.set_fan(false);
        }
    }

    fn start_resting(&mut self, controller: &mut Controller) {
        self.runtime_state = ThermostatRuntimeState::Resting;
        self.last_resting_start_time = Instant::now();
        controller.set_heating(false);
        controller.set_cooling(false);
        // Fan is always on during resting to make sure compressor thaws
        controller.set_fan(true);
    }

    fn start_waiting(&mut self, controller: &mut Controller) {
        self.runtime_state = ThermostatRuntimeState::Waiting;
        self.last_resting_start_time = Instant::now();

        controller.set_heating(false);
        controller.set_cooling(false);
        // Turn fan off if in auto mode. Will always be turned back on when in heating or cooling mode.
        if self.fan_mode == FanStatus::Auto {
            controller.set_fan(false);
        }
    }

    pub fn run(self: &mut ThermostatState, controller: &mut Controller) {
        self.receive_events();
        match self.runtime_state {
            ThermostatRuntimeState::Waiting => {
                // Waiting isn't for resting, but if it happens to have rested long enough we don't need to rest again
                if self.last_resting_start_time.elapsed() > Duration::from_mins(REST_DURATION_MINS) {
                    self.total_cooling_duration = Duration::from_secs(0);
                }
                match self.mode {
                    ModeStatus::Heat => {
                        if self.current_temp_c < self.get_waiting_target_temp() {
                            self.start_heating(controller);
                        }
                    },
                    ModeStatus::Cool => {
                        if self.current_temp_c > self.get_waiting_target_temp() {
                            self.start_cooling(controller);
                        }
                    },
                    ModeStatus::Off => {
                        self.start_idle(controller);
                    }
                }
            },
            ThermostatRuntimeState::Heating => {
                self.total_heating_duration += self.last_run_finished_time.elapsed();
                if self.current_temp_c >= self.target_temp_c {
                    self.start_waiting(controller);
                }
            },
            ThermostatRuntimeState::Cooling => {
                self.total_cooling_duration += self.last_run_finished_time.elapsed();
                if self.should_rest() {
                    self.start_resting(controller);
                } else if self.current_temp_c <= self.target_temp_c {
                    self.start_waiting(controller);
                }
            },
            ThermostatRuntimeState::Resting => {
                if self.last_resting_start_time.elapsed() > Duration::from_mins(REST_DURATION_MINS) {
                    self.total_cooling_duration = Duration::from_secs(0);
                    match self.mode {
                        ModeStatus::Heat => self.start_heating(controller),
                        ModeStatus::Cool => self.start_cooling(controller),
                        ModeStatus::Off => self.start_idle(controller)
                    }
                }
            },
            ThermostatRuntimeState::Idle => {
                match self.mode {
                    ModeStatus::Heat => self.start_heating(controller),
                    ModeStatus::Cool => self.start_cooling(controller),
                    ModeStatus::Off => self.start_idle(controller)
                }
            }
        }
        // Update status message to the UI
        self.actor_events_tx.send(BackendEvent::CurrentStateMessage(self.get_status_message()));
        self.last_run_finished_time = Instant::now();
    }
}