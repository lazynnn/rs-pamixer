mod device;
mod pulseaudio;

use clap::Parser;
use anyhow::{Result, bail};

use crate::device::{Device, gamma_correction, percent_to_volume};
use crate::pulseaudio::PulseAudio;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(name = "rs-pamixer")]
#[command(about = "PulseAudio command line mixer", long_about = None)]
struct Args {
    /// Choose a different sink than the default
    #[arg(long)]
    sink: Option<String>,

    /// Choose a different source than the default
    #[arg(long)]
    source: Option<String>,

    /// Select the default source
    #[arg(long)]
    default_source: bool,

    /// Get the current volume
    #[arg(long)]
    get_volume: bool,

    /// Get the current volume percentage or "muted"
    #[arg(long)]
    get_volume_human: bool,

    /// Set the volume
    #[arg(long, value_name = "VALUE")]
    set_volume: Option<i32>,

    /// Increase the volume
    #[arg(short = 'i', long, value_name = "VALUE")]
    increase: Option<i32>,

    /// Decrease the volume
    #[arg(short = 'd', long, value_name = "VALUE")]
    decrease: Option<i32>,

    /// Switch between mute and unmute
    #[arg(short = 't', long)]
    toggle_mute: bool,

    /// Set mute
    #[arg(short = 'm', long)]
    mute: bool,

    /// Allow volume to go above 100%
    #[arg(long)]
    allow_boost: bool,

    /// Set a limit for the volume
    #[arg(long, value_name = "VALUE")]
    set_limit: Option<i32>,

    /// Increase/decrease using gamma correction
    #[arg(long, default_value = "1.0")]
    gamma: f64,

    /// Unset mute
    #[arg(short = 'u', long)]
    unmute: bool,

    /// Display true if the volume is mute, false otherwise
    #[arg(long)]
    get_mute: bool,

    /// List the sinks
    #[arg(long)]
    list_sinks: bool,

    /// List the sources
    #[arg(long)]
    list_sources: bool,

    /// Print the default sink
    #[arg(long)]
    get_default_sink: bool,

    /// List sink inputs (application audio streams)
    #[arg(long)]
    list_sink_inputs: bool,

    /// Move a sink input to a different sink (routing)
    #[arg(long, value_names = &["INPUT_INDEX", "SINK_INDEX"], num_args = 2)]
    move_sink_input: Option<Vec<u32>>,

    /// Mirror a sink input to multiple sinks (creates a combine sink)
    /// Usage: --mirror INPUT_INDEX SINK_INDEX1 SINK_INDEX2 ...
    #[arg(long, value_names = &["INPUT_INDEX", "SINK_INDEX"], num_args = 2..)]
    mirror: Option<Vec<u32>>,

    /// Unload a module by index (for cleanup)
    #[arg(long, value_name = "MODULE_INDEX")]
    unload_module: Option<u32>,

    /// Unload multiple modules (for mirror cleanup) - comma-separated indices
    #[arg(long, value_name = "MODULE_INDICES")]
    unload_mirror: Option<String>,

    /// List loaded modules
    #[arg(long)]
    list_modules: bool,

    /// Print version info
    #[arg(short = 'v', long)]
    version: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.version {
        println!("rs-pamixer {}", VERSION);
        return Ok(());
    }

    // Validate conflicting options
    validate_options(&args)?;

    // If just listing or getting info, we don't need device selection
    if args.list_sinks || args.list_sources || args.list_sink_inputs || args.get_default_sink || args.list_modules {
        let mut pulse = PulseAudio::new("rs-pamixer")?;

        if args.list_sinks {
            println!("Sinks:");
            for sink in pulse.get_sinks()? {
                println!("{} \"{}\" \"{}\" \"{}\"",
                    sink.index, sink.name, sink.state_string(), sink.description);
            }
        }

        if args.list_sources {
            println!("Sources:");
            for source in pulse.get_sources()? {
                println!("{} \"{}\" \"{}\" \"{}\"",
                    source.index, source.name, source.state_string(), source.description);
            }
        }

        if args.list_sink_inputs {
            println!("Sink Inputs:");
            for input in pulse.get_sink_inputs()? {
                println!("{} \"{}\" -> sink {}", input.index, input.name, input.sink_index);
            }
        }

        if args.list_modules {
            println!("Loaded Modules:");
            for module in pulse.get_modules()? {
                println!("{} \"{}\" \"{}\"", module.index, module.name, module.argument);
            }
        }

        if args.get_default_sink {
            let sink = pulse.get_default_sink()?;
            println!("Default sink:");
            println!("{} \"{}\" \"{}\"", sink.index, sink.name, sink.description);
        }

        return Ok(());
    }

    // Handle audio routing - move sink input
    if let Some(ref indices) = args.move_sink_input {
        let input_index = indices[0];
        let sink_index = indices[1];
        let mut pulse = PulseAudio::new("rs-pamixer")?;
        pulse.move_sink_input(input_index, sink_index)?;
        println!("Moved sink input {} to sink {}", input_index, sink_index);
        return Ok(());
    }

    // Handle mirror - create combine sink and route input to it
    if let Some(ref indices) = args.mirror {
        if indices.len() < 2 {
            bail!("--mirror requires at least 2 arguments: INPUT_INDEX SINK_INDEX [SINK_INDEX...]");
        }
        let input_index = indices[0];
        let sink_indices: Vec<u32> = indices[1..].to_vec();

        let mut pulse = PulseAudio::new("rs-pamixer")?;

        // Get sink names from indices
        let mut sink_names = Vec::new();
        for sink_idx in &sink_indices {
            let sink = pulse.get_sink_by_index(*sink_idx)?;
            sink_names.push(sink.name.clone());
        }

        // Create mirror sink
        let mirror_sink_name = format!("rs_mirror_{}", input_index);
        let sink_name_refs: Vec<&str> = sink_names.iter().map(|s| s.as_str()).collect();
        let (module_indices, _sink_index) = pulse.create_mirror_sink(&sink_name_refs, &mirror_sink_name)?;

        // Get the newly created mirror sink
        let mirror_sink = pulse.get_sink_by_name(&mirror_sink_name)?;

        // Move the input to the mirror sink
        pulse.move_sink_input(input_index, mirror_sink.index)?;

        let module_indices_str: Vec<String> = module_indices.iter().map(|i| i.to_string()).collect();
        println!("Created mirror sink '{}' routing to sinks: {:?}", mirror_sink_name, sink_indices);
        println!("Module indices: {} (use --unload-mirror with these indices to cleanup)", module_indices_str.join(","));
        return Ok(());
    }

    // Handle unload-mirror - unload multiple modules
    if let Some(ref modules_str) = args.unload_mirror {
        let mut pulse = PulseAudio::new("rs-pamixer")?;
        let module_indices: Vec<u32> = modules_str
            .split(',')
            .filter_map(|s| s.trim().parse::<u32>().ok())
            .collect();
        
        for module_index in &module_indices {
            pulse.unload_module(*module_index)?;
        }
        println!("Unloaded modules: {:?}", module_indices);
        return Ok(());
    }

    // Handle unload-module
    if let Some(module_index) = args.unload_module {
        let mut pulse = PulseAudio::new("rs-pamixer")?;
        pulse.unload_module(module_index)?;
        println!("Unloaded module {}", module_index);
        return Ok(());
    }

    // For all other operations, we need a device
    let mut pulse = PulseAudio::new("rs-pamixer")?;
    let device = get_selected_device(&mut pulse, &args)?;

    // Handle volume operations
    if args.set_volume.is_some() || args.increase.is_some() || args.decrease.is_some() {
        let mut new_volume = if let Some(value) = args.set_volume {
            let value = value.max(0);
            percent_to_volume(value)
        } else if let Some(value) = args.increase {
            let value = value.max(0);
            gamma_correction(device.volume_avg, args.gamma, value)
        } else if let Some(value) = args.decrease {
            let value = value.max(0);
            gamma_correction(device.volume_avg, args.gamma, -value)
        } else {
            unreachable!()
        };

        if !args.allow_boost {
            const PA_VOLUME_NORM: u32 = 0x10000;
            if new_volume.0 > PA_VOLUME_NORM {
                new_volume = Volume(PA_VOLUME_NORM);
            }
        }

        pulse.set_volume(&device, new_volume)?;
        return Ok(());
    }

    // Handle set limit
    if let Some(limit_value) = args.set_limit {
        let limit_value = limit_value.max(0);
        let limit = percent_to_volume(limit_value);

        if device.volume_avg.0 > limit.0 {
            pulse.set_volume(&device, limit)?;
        }
        return Ok(());
    }

    // Handle mute operations
    if args.toggle_mute || args.mute || args.unmute {
        let new_mute = if args.toggle_mute {
            !device.mute
        } else {
            args.mute
        };

        pulse.set_mute(&device, new_mute)?;
        return Ok(());
    }

    // Handle get volume / get mute
    if args.get_volume && args.get_mute {
        println!("{} {}", device.mute, device.volume_percent);
    } else if args.get_volume {
        println!("{}", device.volume_percent);
    } else if args.get_volume_human {
        if device.mute {
            println!("muted");
        } else {
            println!("{}%", device.volume_percent);
        }
    } else if args.get_mute {
        println!("{}", device.mute);
    }

    Ok(())
}

use libpulse_binding::volume::Volume;

fn validate_options(args: &Args) -> Result<()> {
    let has_volume_set = args.set_volume.is_some();
    let has_volume_inc = args.increase.is_some();
    let has_volume_dec = args.decrease.is_some();
    let has_toggle_mute = args.toggle_mute;
    let has_mute = args.mute;
    let has_unmute = args.unmute;
    let has_sink = args.sink.is_some();
    let has_source = args.source.is_some();
    let has_default_source = args.default_source;
    let has_get_volume = args.get_volume;
    let has_get_volume_human = args.get_volume_human;
    let has_get_mute = args.get_mute;
    let has_list_sinks = args.list_sinks;
    let has_list_sources = args.list_sources;
    let has_get_default_sink = args.get_default_sink;

    // Check conflicting volume options
    if has_volume_set && has_volume_inc {
        bail!("Conflicting options 'set-volume' and 'increase'");
    }
    if has_volume_set && has_volume_dec {
        bail!("Conflicting options 'set-volume' and 'decrease'");
    }
    if has_volume_dec && has_volume_inc {
        bail!("Conflicting options 'decrease' and 'increase'");
    }

    // Check conflicting mute options
    if has_toggle_mute && has_mute {
        bail!("Conflicting options 'toggle-mute' and 'mute'");
    }
    if has_toggle_mute && has_unmute {
        bail!("Conflicting options 'toggle-mute' and 'unmute'");
    }
    if has_unmute && has_mute {
        bail!("Conflicting options 'unmute' and 'mute'");
    }

    // Check conflicting device selection
    if has_sink && has_source {
        bail!("Conflicting options 'sink' and 'source'");
    }
    if has_sink && has_default_source {
        bail!("Conflicting options 'sink' and 'default-source'");
    }

    // Check conflicting output options
    if has_get_volume && has_list_sinks {
        bail!("Conflicting options 'get-volume' and 'list-sinks'");
    }
    if has_get_volume && has_list_sources {
        bail!("Conflicting options 'get-volume' and 'list-sources'");
    }
    if has_get_volume && has_get_volume_human {
        bail!("Conflicting options 'get-volume' and 'get-volume-human'");
    }
    if has_get_volume && has_get_default_sink {
        bail!("Conflicting options 'get-volume' and 'get-default-sink'");
    }
    if has_get_volume_human && has_list_sinks {
        bail!("Conflicting options 'get-volume-human' and 'list-sinks'");
    }
    if has_get_volume_human && has_list_sources {
        bail!("Conflicting options 'get-volume-human' and 'list-sources'");
    }
    if has_get_volume_human && has_get_mute {
        bail!("Conflicting options 'get-volume-human' and 'get-mute'");
    }
    if has_get_volume_human && has_get_default_sink {
        bail!("Conflicting options 'get-volume-human' and 'get-default-sink'");
    }
    if has_get_mute && has_list_sinks {
        bail!("Conflicting options 'get-mute' and 'list-sinks'");
    }
    if has_get_mute && has_list_sources {
        bail!("Conflicting options 'get-mute' and 'list-sources'");
    }
    if has_get_mute && has_get_default_sink {
        bail!("Conflicting options 'get-mute' and 'get-default-sink'");
    }

    Ok(())
}

fn get_selected_device(pulse: &mut PulseAudio, args: &Args) -> Result<Device> {
    if let Some(ref sink_name) = args.sink {
        // Try parsing as index first, then as name
        if let Ok(index) = sink_name.parse::<u32>() {
            return pulse.get_sink_by_index(index);
        }
        pulse.get_sink_by_name(sink_name)
    } else if args.default_source {
        pulse.get_default_source()
    } else if let Some(ref source_name) = args.source {
        // Try parsing as index first, then as name
        if let Ok(index) = source_name.parse::<u32>() {
            return pulse.get_source_by_index(index);
        }
        pulse.get_source_by_name(source_name)
    } else {
        pulse.get_default_sink()
    }
}