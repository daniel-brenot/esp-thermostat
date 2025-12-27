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
    current_temp_f: f32,
    target_temp_f: f32,
    mode: ModeStatus,
    diff_mode: DiffStatus,
    rest_mode: RestStatus,
    fan_mode: FanStatus,
    use_fahrenheit: bool,

    runtime_state: ThermostatRuntimeState,


    total_cooling_duration: Duration,
    total_heating_duration: Duration,
    
    last_resting_start_time: Instant,

    /// Used to debounce user interaction and prevent rapid changes in mode.
    last_user_interaction_time: Instant
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
            current_temp_f: 70.0,
            target_temp_f: 70.0,
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
        }
    }

    /// Get target temp needed to transition from waiting mode to heating or cooling mode
    pub fn get_waiting_target_temp(&self) -> f32 {
        match self.mode {
            ModeStatus::Heat => {
                match self.diff_mode {
                    DiffStatus::Slow => self.target_temp_f - 1.9,
                    DiffStatus::Normal => self.target_temp_f - 0.75,
                    DiffStatus::Fast => self.target_temp_f - 0.5
                }
            },
            ModeStatus::Cool => {
                match self.diff_mode {
                    DiffStatus::Slow => self.target_temp_f + 1.7,
                    DiffStatus::Normal => self.target_temp_f + 1.2,
                    DiffStatus::Fast => self.target_temp_f + 0.9
                }
            },
            ModeStatus::Off => self.current_temp_f,
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

    /// Formats the temperature in fahrenheit or celcius and appends the unit
    pub fn format_temp(&self, temp: f32) -> String {
        if self.use_fahrenheit {
            format!("{}°F", temp)
        } else {
            format!("{}°C", (temp - 32.0) * 5.0 / 9.0)
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

    pub fn set_target_temp(&mut self, target_temp: f32) {
        self.target_temp_f = target_temp;
    }

    /// Receives events from the UI thread and updates the state accordingly.
    pub fn receive_events(&mut self) {
        while let Ok(event) = self.ui_events_rx.try_recv() {
            match event {
                UiEvent::ModeUpdate(mode) => self.mode = mode,
                UiEvent::UseFahrenheitUpdate(use_fahrenheit) => self.use_fahrenheit = use_fahrenheit,
                UiEvent::DiffUpdate(diff_mode) => self.diff_mode = diff_mode,
                UiEvent::RestUpdate(rest_mode) => self.rest_mode = rest_mode,
                UiEvent::FanUpdate(fan_mode) => self.fan_mode = fan_mode,
                UiEvent::TargetTempUpdate(target_temp) => self.target_temp_f = target_temp,
            }
        }
    }

    pub fn set_runtime_state(&mut self, runtime_state: ThermostatRuntimeState) {
        // We don't want to issue duplicate events to the controller
        if self.runtime_state == runtime_state {
            return;
        }
        match runtime_state {
            ThermostatRuntimeState::Waiting => {
                
            }
            ThermostatRuntimeState::Heating => {
                todo!()
            }
            ThermostatRuntimeState::Cooling => todo!(),
            ThermostatRuntimeState::Resting => todo!(),
            ThermostatRuntimeState::Idle => todo!(),
        }
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

    fn start_waiting(&mut self, controller: &mut Controller) {
        self.runtime_state = ThermostatRuntimeState::Waiting;
        controller.set_heating(false);
        controller.set_cooling(false);
        // Turn fan off if in auto mode. Will always be turned back on when in heating or cooling mode.
        if self.fan_mode == FanStatus::Auto {
            controller.set_fan(false);
        }
    }

    pub fn run(self: &mut ThermostatState, controller: &mut Controller) {
        match self.runtime_state {
            ThermostatRuntimeState::Waiting => {
                match self.mode {
                    ModeStatus::Heat => {
                        if self.current_temp_f < self.get_waiting_target_temp() {
                            self.start_heating(controller);
                        }
                    },
                    ModeStatus::Cool => {
                        if self.current_temp_f > self.get_waiting_target_temp() {
                            self.start_cooling(controller);
                        }
                    },
                    ModeStatus::Off => {
                        self.start_idle(controller);
                    }
                }
            },
            ThermostatRuntimeState::Heating => {
                if self.current_temp_f >= self.target_temp_f {
                    self.start_waiting(controller);
                }
            },
            ThermostatRuntimeState::Cooling => {
                if self.current_temp_f <= self.target_temp_f {
                    self.start_waiting(controller);
                }
            },
            ThermostatRuntimeState::Resting => {
                if self.last_resting_start_time.elapsed() > Duration::from_mins(REST_DURATION_MINS) {
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
    }
}