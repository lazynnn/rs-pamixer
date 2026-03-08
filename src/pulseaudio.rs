use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;
use std::time::Duration;
use anyhow::{Result, bail};

use libpulse_binding::mainloop::threaded::Mainloop;
use libpulse_binding::context::{Context, State as ContextState, FlagSet as ContextFlagSet};
use libpulse_binding::operation::Operation;
use libpulse_binding::proplist::Proplist;
use libpulse_binding::volume::Volume;
use libpulse_binding::callbacks::ListResult;

use crate::device::{Device, DeviceType, SinkInput};

/// Server information (default sink/source names)
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub default_sink_name: String,
    pub default_source_name: String,
}

/// PulseAudio connection wrapper using ThreadedMainloop
pub struct PulseAudio {
    mainloop: Mainloop,
    context: Context,
}

impl PulseAudio {
    /// Create a new PulseAudio connection
    pub fn new(client_name: &str) -> Result<Self> {
        let mainloop = Mainloop::new()
            .ok_or_else(|| anyhow::anyhow!("Failed to create mainloop"))?;

        let proplist = Proplist::new()
            .ok_or_else(|| anyhow::anyhow!("Failed to create proplist"))?;

        let context = Context::new_with_proplist(&mainloop, client_name, &proplist)
            .ok_or_else(|| anyhow::anyhow!("Failed to create context"))?;

        let mut pa = PulseAudio {
            mainloop,
            context,
        };

        // Connect to PulseAudio server
        pa.connect()?;

        Ok(pa)
    }

    /// Connect to the PulseAudio server
    fn connect(&mut self) -> Result<()> {
        // We need to signal the mainloop when the state changes
        let mainloop_ptr = &mut self.mainloop as *mut Mainloop;

        // Set up state callback to signal when state changes
        self.context.set_state_callback(Some(Box::new(move || {
            unsafe {
                (*mainloop_ptr).signal(false);
            }
        })));

        // Start the mainloop thread
        self.mainloop.start()
            .map_err(|_| anyhow::anyhow!("Failed to start mainloop"))?;

        // Lock and connect
        self.mainloop.lock();

        if self.context.connect(None, ContextFlagSet::NOFLAGS, None).is_err() {
            self.mainloop.unlock();
            bail!("Failed to initiate connection to PulseAudio");
        }

        // Wait for connection to be ready
        loop {
            let state = self.context.get_state();
            match state {
                ContextState::Ready => break,
                ContextState::Failed => {
                    self.mainloop.unlock();
                    bail!("Failed to connect to PulseAudio");
                }
                ContextState::Terminated => {
                    self.mainloop.unlock();
                    bail!("Connection to PulseAudio terminated");
                }
                _ => {
                    self.mainloop.wait();
                }
            }
        }

        self.mainloop.unlock();

        Ok(())
    }

    /// Get list of all sinks
    pub fn get_sinks(&mut self) -> Result<Vec<Device>> {
        let devices: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new()));
        let devices_clone = Arc::clone(&devices);

        self.mainloop.lock();

        let introspector = self.context.introspect();
        let op = introspector.get_sink_info_list(move |result| {
            if let ListResult::Item(info) = result {
                let mut devs = devices_clone.lock().unwrap();
                devs.push(Device::from_sink_info(info));
            }
        });

        self.wait_operation(op);
        self.mainloop.unlock();

        let devices = Arc::try_unwrap(devices)
            .map_err(|_| anyhow::anyhow!("Failed to unwrap devices"))?
            .into_inner()
            .map_err(|_| anyhow::anyhow!("Failed to lock devices"))?;

        Ok(devices)
    }

    /// Get list of all sources
    pub fn get_sources(&mut self) -> Result<Vec<Device>> {
        let devices: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new()));
        let devices_clone = Arc::clone(&devices);

        self.mainloop.lock();

        let introspector = self.context.introspect();
        let op = introspector.get_source_info_list(move |result| {
            if let ListResult::Item(info) = result {
                let mut devs = devices_clone.lock().unwrap();
                devs.push(Device::from_source_info(info));
            }
        });

        self.wait_operation(op);
        self.mainloop.unlock();

        let devices = Arc::try_unwrap(devices)
            .map_err(|_| anyhow::anyhow!("Failed to unwrap devices"))?
            .into_inner()
            .map_err(|_| anyhow::anyhow!("Failed to lock devices"))?;

        Ok(devices)
    }

    /// Get a sink by index
    pub fn get_sink_by_index(&mut self, index: u32) -> Result<Device> {
        let devices: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new()));
        let devices_clone = Arc::clone(&devices);

        self.mainloop.lock();

        let introspector = self.context.introspect();
        let op = introspector.get_sink_info_by_index(index, move |result| {
            if let ListResult::Item(info) = result {
                let mut devs = devices_clone.lock().unwrap();
                devs.push(Device::from_sink_info(info));
            }
        });

        self.wait_operation(op);
        self.mainloop.unlock();

        let devices = devices.lock().map_err(|_| anyhow::anyhow!("Failed to lock devices"))?;

        devices.first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Sink {} not found", index))
    }

    /// Get a sink by name
    pub fn get_sink_by_name(&mut self, name: &str) -> Result<Device> {
        let devices: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new()));
        let devices_clone = Arc::clone(&devices);
        let name = name.to_string();

        self.mainloop.lock();

        let introspector = self.context.introspect();
        let op = introspector.get_sink_info_by_name(&name, move |result| {
            if let ListResult::Item(info) = result {
                let mut devs = devices_clone.lock().unwrap();
                devs.push(Device::from_sink_info(info));
            }
        });

        self.wait_operation(op);
        self.mainloop.unlock();

        let devices = devices.lock().map_err(|_| anyhow::anyhow!("Failed to lock devices"))?;

        devices.first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Sink '{}' not found", name))
    }

    /// Get a source by index
    pub fn get_source_by_index(&mut self, index: u32) -> Result<Device> {
        let devices: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new()));
        let devices_clone = Arc::clone(&devices);

        self.mainloop.lock();

        let introspector = self.context.introspect();
        let op = introspector.get_source_info_by_index(index, move |result| {
            if let ListResult::Item(info) = result {
                let mut devs = devices_clone.lock().unwrap();
                devs.push(Device::from_source_info(info));
            }
        });

        self.wait_operation(op);
        self.mainloop.unlock();

        let devices = devices.lock().map_err(|_| anyhow::anyhow!("Failed to lock devices"))?;

        devices.first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Source {} not found", index))
    }

    /// Get a source by name
    pub fn get_source_by_name(&mut self, name: &str) -> Result<Device> {
        let devices: Arc<Mutex<Vec<Device>>> = Arc::new(Mutex::new(Vec::new()));
        let devices_clone = Arc::clone(&devices);
        let name = name.to_string();

        self.mainloop.lock();

        let introspector = self.context.introspect();
        let op = introspector.get_source_info_by_name(&name, move |result| {
            if let ListResult::Item(info) = result {
                let mut devs = devices_clone.lock().unwrap();
                devs.push(Device::from_source_info(info));
            }
        });

        self.wait_operation(op);
        self.mainloop.unlock();

        let devices = devices.lock().map_err(|_| anyhow::anyhow!("Failed to lock devices"))?;

        devices.first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Source '{}' not found", name))
    }

    /// Get server info (default sink/source names)
    pub fn get_server_info(&mut self) -> Result<ServerInfo> {
        let info: Arc<Mutex<Option<ServerInfo>>> = Arc::new(Mutex::new(None));
        let info_clone = Arc::clone(&info);

        self.mainloop.lock();

        let introspector = self.context.introspect();
        let op = introspector.get_server_info(move |server_info| {
            let mut i = info_clone.lock().unwrap();
            *i = Some(ServerInfo {
                default_sink_name: server_info.default_sink_name.as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                default_source_name: server_info.default_source_name.as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
            });
        });

        self.wait_operation(op);
        self.mainloop.unlock();

        let info_guard = info.lock().map_err(|_| anyhow::anyhow!("Failed to lock info"))?;

        info_guard.clone()
            .ok_or_else(|| anyhow::anyhow!("Failed to get server info"))
    }

    /// Get the default sink
    pub fn get_default_sink(&mut self) -> Result<Device> {
        let server_info = self.get_server_info()?;
        if server_info.default_sink_name.is_empty() {
            bail!("No default sink found");
        }
        self.get_sink_by_name(&server_info.default_sink_name)
    }

    /// Get the default source
    pub fn get_default_source(&mut self) -> Result<Device> {
        let server_info = self.get_server_info()?;
        if server_info.default_source_name.is_empty() {
            bail!("No default source found");
        }
        self.get_source_by_name(&server_info.default_source_name)
    }

    /// Set volume for a device
    pub fn set_volume(&mut self, device: &Device, new_volume: Volume) -> Result<()> {
        const PA_VOLUME_MAX: u32 = 0x7FFFFFFF;

        let vol = if new_volume.0 > PA_VOLUME_MAX {
            Volume(PA_VOLUME_MAX)
        } else {
            new_volume
        };

        let mut cvolume = device.volume.clone();
        cvolume.set(cvolume.len(), vol);

        self.mainloop.lock();

        let (tx, rx): (Sender<Result<()>>, Receiver<Result<()>>) = mpsc::channel();
        let tx = Arc::new(Mutex::new(Some(tx)));

        let mut introspector = self.context.introspect();
        let op = match device.device_type {
            DeviceType::Sink => {
                let tx_clone = Arc::clone(&tx);
                introspector.set_sink_volume_by_index(device.index, &cvolume, Some(Box::new(move |success| {
                    if let Some(tx) = tx_clone.lock().unwrap().take() {
                        let _ = tx.send(if success { Ok(()) } else { Err(anyhow::anyhow!("Failed to set sink volume")) });
                    }
                })))
            }
            DeviceType::Source => {
                let tx_clone = Arc::clone(&tx);
                introspector.set_source_volume_by_index(device.index, &cvolume, Some(Box::new(move |success| {
                    if let Some(tx) = tx_clone.lock().unwrap().take() {
                        let _ = tx.send(if success { Ok(()) } else { Err(anyhow::anyhow!("Failed to set source volume")) });
                    }
                })))
            }
        };

        self.wait_operation(op);
        self.mainloop.unlock();

        rx.recv()?.map_err(|e| anyhow::anyhow!("Failed to set volume: {}", e))
    }

    /// Set mute state for a device
    pub fn set_mute(&mut self, device: &Device, mute: bool) -> Result<()> {
        self.mainloop.lock();

        let (tx, rx): (Sender<Result<()>>, Receiver<Result<()>>) = mpsc::channel();
        let tx = Arc::new(Mutex::new(Some(tx)));

        let mut introspector = self.context.introspect();
        let op = match device.device_type {
            DeviceType::Sink => {
                let tx_clone = Arc::clone(&tx);
                introspector.set_sink_mute_by_index(device.index, mute, Some(Box::new(move |success| {
                    if let Some(tx) = tx_clone.lock().unwrap().take() {
                        let _ = tx.send(if success { Ok(()) } else { Err(anyhow::anyhow!("Failed to set sink mute")) });
                    }
                })))
            }
            DeviceType::Source => {
                let tx_clone = Arc::clone(&tx);
                introspector.set_source_mute_by_index(device.index, mute, Some(Box::new(move |success| {
                    if let Some(tx) = tx_clone.lock().unwrap().take() {
                        let _ = tx.send(if success { Ok(()) } else { Err(anyhow::anyhow!("Failed to set source mute")) });
                    }
                })))
            }
        };

        self.wait_operation(op);
        self.mainloop.unlock();

        rx.recv()?.map_err(|e| anyhow::anyhow!("Failed to set mute: {}", e))
    }

    /// Get all sink inputs (application audio streams)
    pub fn get_sink_inputs(&mut self) -> Result<Vec<SinkInput>> {
        let inputs: Arc<Mutex<Vec<SinkInput>>> = Arc::new(Mutex::new(Vec::new()));
        let inputs_clone = Arc::clone(&inputs);

        self.mainloop.lock();

        let introspector = self.context.introspect();
        let op = introspector.get_sink_input_info_list(move |result| {
            if let ListResult::Item(info) = result {
                let mut ins = inputs_clone.lock().unwrap();
                ins.push(SinkInput::from_sink_input_info(info));
            }
        });

        self.wait_operation(op);
        self.mainloop.unlock();

        let inputs = Arc::try_unwrap(inputs)
            .map_err(|_| anyhow::anyhow!("Failed to unwrap inputs"))?
            .into_inner()
            .map_err(|_| anyhow::anyhow!("Failed to lock inputs"))?;

        Ok(inputs)
    }

    /// Move a sink input to a different sink (audio routing)
    pub fn move_sink_input(&mut self, input_index: u32, sink_index: u32) -> Result<()> {
        self.mainloop.lock();

        let (tx, rx): (Sender<Result<()>>, Receiver<Result<()>>) = mpsc::channel();
        let tx = Arc::new(Mutex::new(Some(tx)));

        let mut introspector = self.context.introspect();
        let op = introspector.move_sink_input_by_index(input_index, sink_index, Some(Box::new(move |success| {
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(if success { Ok(()) } else { Err(anyhow::anyhow!("Failed to move sink input")) });
            }
        })));

        self.wait_operation(op);
        self.mainloop.unlock();

        rx.recv()?.map_err(|e| anyhow::anyhow!("Failed to move sink input: {}", e))
    }

    /// Wait for an operation to complete
    fn wait_operation<T: ?Sized>(&mut self, op: Operation<T>) {
        while op.get_state() == libpulse_binding::operation::State::Running {
            // Release the lock, wait a bit, then check again
            // This is not ideal but avoids the signaling complexity
            self.mainloop.unlock();
            thread::sleep(Duration::from_millis(10));
            self.mainloop.lock();
        }
    }
}

impl Drop for PulseAudio {
    fn drop(&mut self) {
        // Properly cleanup: disconnect context before stopping mainloop
        // Check if context is still connected before disconnecting
        let state = self.context.get_state();
        if state == ContextState::Ready {
            self.mainloop.lock();
            self.context.disconnect();
            self.mainloop.unlock();
        }
        // Stop the mainloop thread
        self.mainloop.stop();
    }
}