
#[derive(Debug, Clone)]
pub enum UiEvent {
    // Event from ui to backend to update the mode
    ModeUpdate(ModeStatus),
    // Event from frontend to backend to update the unit
    UseFahrenheitUpdate(bool),
    // Event from frontend to backend to update the diff mode
    DiffUpdate(DiffStatus),
    // Event from frontend to backend to update the rest mode
    RestUpdate(RestStatus),
    // Event from frontend to backend to update the fan mode
    FanUpdate(FanStatus),
    // Event from frontend to backend to update the target temp
    TargetTempUpdate(f32),
}

#[derive(Debug, Clone)]
pub enum BackendEvent {
    // Event from backend to ui to update the current temperature (in Celsius)
    CurrentTempCUpdate(f32),
    // Event from backend to ui to update message for current state
    // Should be one of "Heating", "Cooling", "Resting for <duration>", "Waiting for <target temp>"
    CurrentStateMessage(String),
}
#[derive(Debug, Clone)]
#[repr(i32)]
pub enum ModeStatus {
    Heat = 0,
    Cool = 1,
    Off = 2,
}

#[derive(Debug, Clone)]
#[repr(i32)]
pub enum DiffStatus {
    Slow,
    Normal,
    Fast,
}

#[derive(Debug, Clone)]
#[repr(i32)]
pub enum RestStatus {
    Short,
    Medium,
    Long,
    Off,
}

#[derive(Debug, Clone, PartialEq)]
#[repr(i32)]
pub enum FanStatus {
    Auto,
    On,
}


impl TryFrom<i32> for ModeStatus {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ModeStatus::Heat),
            1 => Ok(ModeStatus::Cool),
            2 => Ok(ModeStatus::Off),
            _ => Err(anyhow::anyhow!("Invalid mode status: {}", value)),
        }
    }
}

impl TryFrom<i32> for DiffStatus {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(DiffStatus::Slow),
            1 => Ok(DiffStatus::Normal),
            2 => Ok(DiffStatus::Fast),
            _ => Err(anyhow::anyhow!("Invalid diff status: {}", value)),
        }
    }
}

impl TryFrom<i32> for RestStatus {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(RestStatus::Short),
            1 => Ok(RestStatus::Medium),
            2 => Ok(RestStatus::Long),
            3 => Ok(RestStatus::Off),
            _ => Err(anyhow::anyhow!("Invalid rest status: {}", value)),
        }
    }
}

impl TryFrom<i32> for FanStatus {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(FanStatus::Auto),
            1 => Ok(FanStatus::On),
            _ => Err(anyhow::anyhow!("Invalid fan status: {}", value)),
        }
    }
}