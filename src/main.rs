//! Real-time IEC 61850 protection IED event loop.
//!
//! Loads configuration, sets up RT scheduling on Linux, then processes
//! Sampled Values and publishes GOOSE trip messages.

use poc_protection_functions::{
    SystemConfig, PtocSlidingWindow, Pioc, ProtectionFunction, ProtectionResult,
    SvSampleBuffer, SvSubscriber, GoosePublisher, adc_to_primary,
    diagnostics::LatencyTracker,
};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

fn get_timestamp_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

// ---------------------------------------------------------------------------
// Linux real-time setup
// ---------------------------------------------------------------------------

/// Pin the calling thread to its assigned CPU cores (read from cgroup cpuset),
/// set `SCHED_FIFO` priority 80 and lock memory.
///
/// No-op on non-Linux platforms.
#[cfg(target_os = "linux")]
fn setup_realtime() {
    use libc::{
        mlockall, sched_param, sched_setscheduler, MCL_CURRENT, MCL_FUTURE, SCHED_FIFO,
    };

    // Lock all current and future memory pages to prevent page faults
    // SAFETY: mlockall is a well-defined POSIX call.
    let ret = unsafe { mlockall(MCL_CURRENT | MCL_FUTURE) };
    if ret != 0 {
        log::warn!("mlockall failed (errno {}); continuing without memory lock", ret);
    }

    // Set SCHED_FIFO priority 80
    let param = sched_param { sched_priority: 80 };
    // SAFETY: sched_setscheduler with a valid param is safe.
    let ret = unsafe { sched_setscheduler(0, SCHED_FIFO, &param) };
    if ret != 0 {
        log::warn!(
            "sched_setscheduler SCHED_FIFO failed (errno {}); continuing without RT priority",
            ret
        );
    } else {
        log::info!("RT scheduling: SCHED_FIFO priority 80");
    }
}

#[cfg(not(target_os = "linux"))]
fn setup_realtime() {
    log::info!("RT scheduling not configured (non-Linux platform)");
}

// ---------------------------------------------------------------------------
// Main event loop
// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging — suppress per-sample noise by defaulting to Info
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .init();

    log::info!("POC Protection Functions — RT IED");
    log::info!("Version: {}", poc_protection_functions::VERSION);

    // --- Load configuration ---
    let config_path = std::env::var("IED_CONFIG")
        .unwrap_or_else(|_| "config/ied.json".to_string());

    let config = if std::path::Path::new(&config_path).exists() {
        log::info!("Loading configuration from {}", config_path);
        SystemConfig::from_json_file(&config_path)?
    } else {
        log::info!("Config file not found at {}; using defaults", config_path);
        SystemConfig::default()
    };

    log::info!("  PTOC Iset: {} A  Tset: {} ms", config.ptoc.iset, config.ptoc.tset);
    log::info!("  PIOC Iset: {} A", config.pioc.iset);
    log::info!("  CT ratio: {}/{}", config.ct.primary, config.ct.secondary);
    log::info!("  Samples/cycle: {}", config.sv.samples_per_cycle);

    // --- Linux RT setup ---
    setup_realtime();

    // --- Initialise components ---
    let mut sliding_ptoc = PtocSlidingWindow::new(
        config.ptoc.clone(),
        config.sv.samples_per_cycle,
    );
    let mut pioc = Pioc::new(config.pioc.clone());
    let mut sample_buffer = SvSampleBuffer::new(config.sv.samples_per_cycle);

    let mut latency = LatencyTracker::new(4000); // 1 second at 4 kSa/s
    let mut last_trip = ProtectionResult::NoTrip;
    let mut sample_count = 0u64;
    let log_interval = config.sv.samples_per_cycle * 50; // log every ~50 cycles

    let live_mode = std::env::var("IED_LIVE").map(|v| v == "1").unwrap_or(false);

    if live_mode {
        // ---------------------------------------------------------------
        // Live I/O mode: SV network input → protection → GOOSE output
        // Requires CAP_NET_RAW (Linux) or root.  Set IED_LIVE=1 to enable.
        // ---------------------------------------------------------------
        log::info!("Entering live I/O mode (SV input → GOOSE output)");
        let mut sv = SvSubscriber::new(config.sv.clone());
        sv.init()?;
        let mut goose = GoosePublisher::new(config.goose.clone());
        goose.init()?;

        loop {
            let now = get_timestamp_micros();

            // Send GOOSE retransmission / heartbeat if one is due
            goose.tick(now)?;

            // Non-blocking: skip if no SV packet is available yet
            match sv.receive_sample() {
                Ok(sample) => {
                    let primary = adc_to_primary(sample.current_adc, &config.adc, &config.ct);
                    let t = sample.timestamp;
                    let start = LatencyTracker::start();

                    let ptoc_result = sliding_ptoc.process_sample(primary, t);
                    let pioc_result = pioc.process(primary, t);

                    latency.stop(start);

                    let trip = pioc_result == ProtectionResult::Trip
                        || ptoc_result == ProtectionResult::Trip;

                    if trip != goose.last_trip_state() {
                        goose.publish_trip(trip, t)?;
                        if trip {
                            log::warn!("TRIP! primary={:.1}A t={}µs", primary, t);
                        } else {
                            log::info!("CLEAR t={}µs", t);
                        }
                    }

                    sample_count += 1;
                    if sample_count % log_interval as u64 == 0 {
                        if let Some(stats) = latency.stats() {
                            log::info!("sample={} {}", sample_count, stats);
                        }
                    }
                }
                // Non-blocking socket returned WouldBlock — spin
                Err(e) if e.to_string().contains("No data available") => {}
                Err(e) => {
                    log::error!("SV receive error: {}", e);
                    break;
                }
            }
        }

        return Ok(());
    }

    // ---------------------------------------------------------------
    // Simulation mode (default — no network hardware required)
    // ---------------------------------------------------------------
    log::info!("Entering simulation mode (set IED_LIVE=1 for live SV input)");

    // --- Simulation loop (replaces live SV subscriber for portability) ---
    let base_time = get_timestamp_micros();
    let sample_period_us: u64 = 250; // 4 kSa/s

    // Simulate 10 cycles of overcurrent then 5 cycles of normal
    let total_cycles = 15;
    let overcurrent_cycles = 10;

    for cycle in 0..total_cycles {
        let is_overcurrent = cycle < overcurrent_cycles;
        // Peak = Iset * 1.5 * √2 for overcurrent; 0.5 * Iset * √2 for normal
        let peak_primary = if is_overcurrent {
            config.ptoc.iset * 1.5 * std::f64::consts::SQRT_2
        } else {
            config.ptoc.iset * 0.5 * std::f64::consts::SQRT_2
        };

        sample_buffer.clear();

        for s in 0..config.sv.samples_per_cycle {
            let angle = 2.0 * std::f64::consts::PI * s as f64
                / config.sv.samples_per_cycle as f64;
            let primary_sample = peak_primary * angle.sin();

            let t = base_time + sample_count * sample_period_us;
            let start = LatencyTracker::start();

            // --- Sliding-window PTOC (evaluates every sample) ---
            let ptoc_result = sliding_ptoc.process_sample(primary_sample, t);

            // --- PIOC (instantaneous) ---
            let pioc_result = pioc.process(primary_sample.abs(), t);

            latency.stop(start);

            sample_buffer.add_sample(
                (primary_sample / (config.adc.scale_factor * config.ct.ratio())) as i32,
            );
            sample_count += 1;

            // Detect trip-state change → would publish GOOSE here
            let current_trip = if pioc_result == ProtectionResult::Trip {
                pioc_result
            } else {
                ptoc_result
            };

            if current_trip != last_trip {
                match &current_trip {
                    ProtectionResult::Trip => {
                        log::warn!(
                            "TRIP! cycle={} sample={} t={}µs",
                            cycle,
                            s,
                            t
                        );
                        // In production: GoosePublisher::send_trip(…)
                    }
                    ProtectionResult::NoTrip if last_trip == ProtectionResult::Trip => {
                        // This won't happen until reset; here just for completeness
                    }
                    _ => {}
                }
                last_trip = current_trip;
            }
        }

        // Periodic logging (~every 50 cycles)
        if sample_count.is_multiple_of(log_interval as u64) {
            if let Some(stats) = latency.stats() {
                log::info!("Cycle {}: {}", cycle, stats);
            }
        }

        // Break early once tripped for this simulation
        if last_trip == ProtectionResult::Trip {
            log::info!("Trip detected — would issue GOOSE retransmissions");
            // Continue processing remaining cycles for simulation completeness
        }
    }

    log::info!(
        "Simulation complete. {} samples processed.",
        sample_count
    );
    if let Some(stats) = latency.stats() {
        log::info!("Final latency stats: {}", stats);
    }

    Ok(())
}
