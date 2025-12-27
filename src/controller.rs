
/// Used to interface with the relays and thermostat sensor.
pub struct Controller {
    is_cooling: bool,
    is_heating: bool,
    is_fan: bool,
}

impl Controller {
    pub fn new() -> Self {
        Self {
            is_cooling: false,
            is_heating: false,
            is_fan: false,
        }
    }

    /// Get the last known temperature from the sensorin Fahrenheit.
    pub fn get_temperature_f(&self) -> f32 {
        todo!()
    }

    pub fn set_cooling(&mut self, enabled: bool) {
        if self.is_cooling == enabled {
            return;
        }
        self.is_cooling = enabled;
        todo!()
    }

    pub fn set_heating(&mut self, enabled: bool) {
        if self.is_heating == enabled {
            return;
        }
        self.is_heating = enabled;
        todo!()
    }

    pub fn set_fan(&mut self, enabled: bool) {
        if self.is_fan == enabled {
            return;
        }
        self.is_fan = enabled;
        todo!()
    }
}