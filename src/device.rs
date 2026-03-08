use libpulse_binding::volume::{Volume, ChannelVolumes};
use libpulse_binding::def::{SinkState, SourceState};

/// Device type: Sink (output) or Source (input)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    Sink,
    Source,
}

/// Device state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    Running,
    Idle,
    Suspended,
    Invalid,
}

/// Represents a PulseAudio device (Sink or Source)
#[derive(Debug, Clone)]
pub struct Device {
    pub index: u32,
    pub device_type: DeviceType,
    pub name: String,
    pub description: String,
    pub state: DeviceState,
    pub volume: ChannelVolumes,
    pub volume_avg: Volume,
    pub volume_percent: i32,
    pub mute: bool,
}

impl Device {
    /// Create a Device from sink info
    pub fn from_sink_info(info: &libpulse_binding::context::introspect::SinkInfo<'_>) -> Self {
        let volume = info.volume;
        let volume_avg = volume.avg();
        
        Device {
            index: info.index,
            device_type: DeviceType::Sink,
            name: info.name.as_ref().map(|s| s.to_string()).unwrap_or_default(),
            description: info.description.as_ref().map(|s| s.to_string()).unwrap_or_default(),
            state: convert_sink_state(info.state),
            volume,
            volume_avg,
            volume_percent: volume_to_percent(volume_avg),
            mute: info.mute,
        }
    }

    /// Create a Device from source info
    pub fn from_source_info(info: &libpulse_binding::context::introspect::SourceInfo<'_>) -> Self {
        let volume = info.volume;
        let volume_avg = volume.avg();
        
        Device {
            index: info.index,
            device_type: DeviceType::Source,
            name: info.name.as_ref().map(|s| s.to_string()).unwrap_or_default(),
            description: info.description.as_ref().map(|s| s.to_string()).unwrap_or_default(),
            state: convert_source_state(info.state),
            volume,
            volume_avg,
            volume_percent: volume_to_percent(volume_avg),
            mute: info.mute,
        }
    }

    /// Get state as human-readable string
    pub fn state_string(&self) -> &'static str {
        match self.state {
            DeviceState::Running => "Running",
            DeviceState::Idle => "Idle",
            DeviceState::Suspended => "Suspended",
            DeviceState::Invalid => "Invalid state",
        }
    }
}

/// Represents a Sink Input (application audio stream)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SinkInput {
    pub index: u32,
    pub name: String,
    pub sink_index: u32,
    pub mute: bool,
}

impl SinkInput {
    /// Create a SinkInput from sink input info
    pub fn from_sink_input_info(info: &libpulse_binding::context::introspect::SinkInputInfo<'_>) -> Self {
        SinkInput {
            index: info.index,
            name: info.name.as_ref().map(|s| s.to_string()).unwrap_or_default(),
            sink_index: info.sink,
            mute: info.mute,
        }
    }
}

/// Convert sink state to our DeviceState
fn convert_sink_state(state: SinkState) -> DeviceState {
    match state {
        SinkState::Running => DeviceState::Running,
        SinkState::Idle => DeviceState::Idle,
        SinkState::Suspended => DeviceState::Suspended,
        _ => DeviceState::Invalid,
    }
}

/// Convert source state to our DeviceState
fn convert_source_state(state: SourceState) -> DeviceState {
    match state {
        SourceState::Running => DeviceState::Running,
        SourceState::Idle => DeviceState::Idle,
        SourceState::Suspended => DeviceState::Suspended,
        _ => DeviceState::Invalid,
    }
}

/// Convert volume to percentage (PA_VOLUME_NORM = 65536 = 100%)
fn volume_to_percent(vol: Volume) -> i32 {
    // PA_VOLUME_NORM is the normal volume (100%)
    const PA_VOLUME_NORM: u32 = 0x10000; // 65536
    
    // Volume is a newtype wrapper around u32
    let vol_raw: u32 = vol.0;
    ((vol_raw as i64 * 100 / PA_VOLUME_NORM as i64)) as i32
}

/// Convert percentage to volume
pub fn percent_to_volume(percent: i32) -> Volume {
    const PA_VOLUME_NORM: u32 = 0x10000;
    
    let raw = (percent as i64 * PA_VOLUME_NORM as i64 / 100) as u32;
    Volume(raw)
}

/// Apply gamma correction to volume for perceptual scaling
pub fn gamma_correction(current_vol: Volume, gamma: f64, delta_percent: i32) -> Volume {
    const PA_VOLUME_NORM: u32 = 0x10000;
    
    let vol_raw: u32 = current_vol.0;
    let mut j = vol_raw as f64 / PA_VOLUME_NORM as f64;
    
    // Apply inverse gamma to linearize
    j = j.powf(1.0 / gamma);
    
    // Apply delta
    let rel_delta = delta_percent as f64 / 100.0;
    j = j + rel_delta;
    if j < 0.0 {
        j = 0.0;
    }
    
    // Apply gamma to compress back
    j = j.powf(gamma);
    let new_vol = (j * PA_VOLUME_NORM as f64).round() as u32;
    
    Volume(new_vol)
}