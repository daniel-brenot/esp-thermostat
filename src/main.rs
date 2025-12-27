use esp_idf_svc::hal::gpio::{Pin, PinDriver, Pull};
use esp_idf_svc::hal::i2c::I2cDriver;
use esp_idf_svc::sys::{self as idf_sys, gpio_set_level};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        prelude::*,
        task::{block_on, thread::ThreadSpawnConfiguration},
    },
    nvs::EspDefaultNvsPartition,
    timer::EspTaskTimerService,
};
use esp_thermostat::events::{BackendEvent, UiEvent};
use esp_thermostat::ui::window::Window;
use std::ffi::CString;
use std::{
    sync::mpsc::{self, Receiver, Sender, SyncSender},
    sync::Arc,
    thread,
};

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::info!("Booting up...");

    

    let touch_i2c = setup_display()?;

    // UI Updates Channel is used to send events to the UI thread.
    let (ui_updates_tx, ui_updates_rx): (Sender<BackendEvent>, Receiver<BackendEvent>) = mpsc::channel();
    // Actor would take action on events typically from the UI thread. (e.g. when a button is pressed)
    let (actor_tx, actor_rx): (Sender<UiEvent>, Receiver<UiEvent>) = mpsc::channel();
    
    // Need more stack space since we use stack based allocator
    ThreadSpawnConfiguration {
        stack_size: 4096,
        ..Default::default()
    }
    .set()?;

    if let Err(e) = ThreadSpawnConfiguration::default().set() {
        log::error!("Failed to set thread spawn configuration: {}", e);
    }

    let window_thread = thread::spawn(move || {
        Window::init(
            touch_i2c,
            ui_updates_rx,
            actor_tx,
        ).unwrap();
    });



    let _ = window_thread.join().unwrap();

    Ok(())
}

/// Sets up the touch display and returns the I2cDriver for it.
fn setup_display() -> Result<I2cDriver<'static>, anyhow::Error> {
    let peripherals = Peripherals::take()?;

    let mut touch_i2c = esp_idf_svc::hal::i2c::I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio8,
        peripherals.pins.gpio9,
        &esp_idf_svc::hal::i2c::config::Config::new().baudrate(400_000.Hz()),
    )?;
    
    // Reset touch screen before using it
    // DO NOT REMOVE THIS.
    let _ = touch_i2c.write(0x24, &[0x1], 1000);
    let mut exio_value = [0xC];
    let _ = touch_i2c.write(0x38, &exio_value, 1000);
    std::thread::sleep(std::time::Duration::from_millis(100));
    unsafe {
        gpio_set_level(peripherals.pins.gpio4.pin(), 0);
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    exio_value[0] = 0xE;
    let _ = touch_i2c.write(0x38, &exio_value, 1000);
    // Not sute why this is needed, probably to give the touch screen time to initialize
    std::thread::sleep(std::time::Duration::from_millis(200));
    Ok(touch_i2c)
}