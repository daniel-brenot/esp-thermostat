use ds18b20::{Ds18b20, Resolution};
use esp_idf_svc::hal::delay::Ets;
use esp_idf_svc::hal::gpio::{Gpio2, Gpio3, Gpio4, Gpio21, InputOutput, Output, PinDriver};
use one_wire_bus::OneWire;

/// Used to interface with the relays and thermostat sensor.
pub struct Controller {
    is_cooling: bool,
    is_heating: bool,
    is_fan: bool,
    one_wire: OneWire<PinDriver<'static, Gpio21, InputOutput>>,
    sensor: Option<Ds18b20>,
    last_temperature_c: Option<f32>,
    /// GPIO 2 - Heat relay control
    heat_pin: PinDriver<'static, Gpio2, Output>,
    /// GPIO 3 - Cool relay control
    cool_pin: PinDriver<'static, Gpio3, Output>,
    /// GPIO 4 - Fan relay control
    fan_pin: PinDriver<'static, Gpio4, Output>,
}

impl Controller {
    /// Create a new controller with:
    /// - DS18B20 temperature sensor on GPIO 21
    /// - Heat relay on GPIO 2
    /// - Cool relay on GPIO 3
    /// - Fan relay on GPIO 4
    pub fn new(
        temp_pin: Gpio21,
        heat_pin: Gpio2,
        cool_pin: Gpio3,
        fan_pin: Gpio4,
    ) -> Result<Self, esp_idf_svc::sys::EspError> {
        // Configure the temperature sensor pin as open-drain for 1-Wire communication
        let pin_driver = PinDriver::input_output_od(temp_pin)?;
        let mut one_wire = OneWire::new(pin_driver).map_err(|_| {
            esp_idf_svc::sys::EspError::from_infallible::<{ esp_idf_svc::sys::ESP_ERR_INVALID_STATE }>()
        })?;

        // Search for DS18B20 sensor on the bus
        let mut delay = Ets;
        let sensor = Self::find_ds18b20_sensor(&mut one_wire, &mut delay);

        if sensor.is_none() {
            log::warn!("No DS18B20 sensor found on GPIO 21");
        } else {
            log::info!("DS18B20 sensor found on GPIO 21");
        }

        // Configure relay control pins as outputs (active low - start with relays off)
        let mut heat_pin = PinDriver::output(heat_pin)?;
        let mut cool_pin = PinDriver::output(cool_pin)?;
        let mut fan_pin = PinDriver::output(fan_pin)?;

        // Initialize all relays to off (low = off for active-high relays)
        heat_pin.set_low()?;
        cool_pin.set_low()?;
        fan_pin.set_low()?;

        log::info!("Controller initialized: Heat=GPIO2, Cool=GPIO3, Fan=GPIO4");

        Ok(Self {
            is_cooling: false,
            is_heating: false,
            is_fan: false,
            one_wire,
            sensor,
            last_temperature_c: None,
            heat_pin,
            cool_pin,
            fan_pin,
        })
    }

    /// Search for a DS18B20 sensor on the 1-Wire bus.
    fn find_ds18b20_sensor(
        one_wire: &mut OneWire<PinDriver<'static, Gpio21, InputOutput>>,
        delay: &mut Ets,
    ) -> Option<Ds18b20> {
        let mut search_state = None;

        // Search for devices on the bus
        loop {
            match one_wire.device_search(search_state.as_ref(), false, delay) {
                Ok(Some((device_address, state))) => {
                    search_state = Some(state);
                    // Check if this is a DS18B20 (family code 0x28)
                    if device_address.family_code() == ds18b20::FAMILY_CODE {
                        log::info!("Found DS18B20 at address: {:?}", device_address);
                        return Some(Ds18b20::new::<()>(device_address).ok()?);
                    }
                }
                Ok(None) => {
                    // No more devices
                    break;
                }
                Err(_) => {
                    log::error!("Error searching for devices on 1-Wire bus");
                    break;
                }
            }
        }
        None
    }

    /// Read the temperature from the DS18B20 sensor and update the cached value.
    /// Returns the temperature in Celsius if successful.
    fn read_temperature(&mut self) -> Option<f32> {
        let sensor = self.sensor.as_ref()?;
        let mut delay = Ets;

        // Start temperature measurement
        if sensor.start_temp_measurement(&mut self.one_wire, &mut delay).is_err() {
            log::error!("Failed to start temperature measurement");
            return self.last_temperature_c;
        }

        // Wait for conversion to complete (750ms for 12-bit resolution)
        Resolution::Bits12.delay_for_measurement_time(&mut delay);

        // Read the temperature
        match sensor.read_data(&mut self.one_wire, &mut delay) {
            Ok(data) => {
                let temp_c = data.temperature;
                self.last_temperature_c = Some(temp_c);
                log::debug!("Temperature read: {:.2}°C", temp_c);
                Some(temp_c)
            }
            Err(_) => {
                log::error!("Failed to read temperature from DS18B20");
                self.last_temperature_c
            }
        }
    }

    /// Get the current temperature from the sensor in Celsius (base unit).
    /// This will trigger a new reading from the sensor.
    pub fn get_temperature_c(&mut self) -> f32 {
        self.read_temperature().unwrap_or(25.0) // Default to 25°C if no reading
    }

    /// Get the current temperature from the sensor in Fahrenheit.
    /// This converts from the base Celsius reading.
    pub fn get_temperature_f(&mut self) -> f32 {
        Self::celsius_to_fahrenheit(self.get_temperature_c())
    }

    /// Convert Celsius to Fahrenheit: F = C * 9/5 + 32
    pub fn celsius_to_fahrenheit(celsius: f32) -> f32 {
        celsius * 9.0 / 5.0 + 32.0
    }

    /// Convert Fahrenheit to Celsius: C = (F - 32) * 5/9
    pub fn fahrenheit_to_celsius(fahrenheit: f32) -> f32 {
        (fahrenheit - 32.0) * 5.0 / 9.0
    }

    /// Control the cooling relay on GPIO 3.
    /// Active high: high = relay on, low = relay off
    pub fn set_cooling(&mut self, enabled: bool) {
        if self.is_cooling == enabled {
            return;
        }
        self.is_cooling = enabled;
        if enabled {
            log::info!("Cooling ON");
            let _ = self.cool_pin.set_high();
        } else {
            log::info!("Cooling OFF");
            let _ = self.cool_pin.set_low();
        }
    }

    /// Control the heating relay on GPIO 2.
    /// Active high: high = relay on, low = relay off
    pub fn set_heating(&mut self, enabled: bool) {
        if self.is_heating == enabled {
            return;
        }
        self.is_heating = enabled;
        if enabled {
            log::info!("Heating ON");
            let _ = self.heat_pin.set_high();
        } else {
            log::info!("Heating OFF");
            let _ = self.heat_pin.set_low();
        }
    }

    /// Control the fan relay on GPIO 4.
    /// Active high: high = relay on, low = relay off
    pub fn set_fan(&mut self, enabled: bool) {
        if self.is_fan == enabled {
            return;
        }
        self.is_fan = enabled;
        if enabled {
            log::info!("Fan ON");
            let _ = self.fan_pin.set_high();
        } else {
            log::info!("Fan OFF");
            let _ = self.fan_pin.set_low();
        }
    }
}
