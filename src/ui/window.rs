use anyhow::Result;
use esp_idf_svc::hal::i2c::I2cDriver;
use slint::{Color, SharedString, Weak};
use std::{
    collections::HashMap,
    sync::{
        mpsc::{Receiver, Sender},
        Arc,
    },
    time::Duration,
};

use crate::{bsp::slint_platform, events::{BackendEvent, DiffStatus, FanStatus, ModeStatus, RestStatus, UiEvent}};


slint::include_modules!();
pub struct Window;

impl Window {
    pub fn init(
        touch_i2c: I2cDriver<'static>,
        rx: Receiver<BackendEvent>,
        actor_tx: Sender<UiEvent>,
    ) -> Result<()> {
        slint_platform::init(touch_i2c);
        let window = MainWindow::new()
            .map_err(|e| anyhow::anyhow!("Failed to create main window: {}", e))?;

        install_callbacks(&window, actor_tx);
        let timer = regiser_event_receiver_timer(&window, rx);

        window
            .run()
            .map_err(|e| anyhow::anyhow!("Failed to run main window: {}", e))?;

        Ok(())
    }
}

fn install_callbacks(window: &MainWindow, actor_tx: Sender<UiEvent>) {
    let _ = window.as_weak();
    let diff_mode_tx = actor_tx.clone();
    let rest_mode_tx = actor_tx.clone();
    let fan_mode_tx = actor_tx.clone();
    let hvac_mode_tx = actor_tx.clone();
    let target_temp_tx = actor_tx.clone();
    window.on_diff_mode_changed(move |e| {
        diff_mode_tx.send(UiEvent::DiffUpdate(DiffStatus::try_from(e).unwrap())).unwrap();
    });
    window.on_rest_mode_changed(move |e| {
        rest_mode_tx.send(UiEvent::RestUpdate(RestStatus::try_from(e).unwrap())).unwrap();
    });
    window.on_fan_mode_changed(move |e| {
        fan_mode_tx.send(UiEvent::FanUpdate(FanStatus::try_from(e).unwrap())).unwrap();
    });
    window.on_hvac_mode_changed(move |e| {
        hvac_mode_tx.send(UiEvent::ModeUpdate(ModeStatus::try_from(e).unwrap())).unwrap();
    });
    window.on_target_temp_changed(move |e| {
        target_temp_tx.send(UiEvent::TargetTempUpdate(e)).unwrap();
    });
}

fn regiser_event_receiver_timer(window: &MainWindow, rx: Receiver<BackendEvent>) -> slint::Timer {
    let window_weak = window.as_weak();
    let timer = slint::Timer::default();
    let callback = move || {
        // On call, upgrade the weak reference to a strong reference.
        let window = window_weak.upgrade().unwrap();
        while let Ok(msg) = rx.try_recv() {
            match msg {
                BackendEvent::CurrentTempCUpdate(temp_c) => {
                    window.set_current_temp_c(temp_c);
                }
                BackendEvent::CurrentStateMessage(message) => {
                    window.set_thermostat_state(SharedString::from(message));
                }
            }
        }
    };
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_secs(1),
        callback
    );
    timer
}