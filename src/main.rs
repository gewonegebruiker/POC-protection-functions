/// Example application demonstrating PTOC protection function
use poc_protection_functions::{
    SystemConfig, Ptoc, ProtectionFunction, ProtectionResult,
    CurrentScaler, SvSampleBuffer,
};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

fn get_timestamp_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("POC Protection Functions - PTOC Example");
    log::info!("Version: {}", poc_protection_functions::VERSION);

    // Load or create configuration
    let config = SystemConfig::default();
    
    log::info!("Configuration:");
    log::info!("  PTOC Iset: {} A", config.ptoc.iset);
    log::info!("  PTOC Tset: {} ms", config.ptoc.tset);
    log::info!("  CT Ratio: {}/{}", config.ct.primary, config.ct.secondary);
    log::info!("  ADC Scale: {}", config.adc.scale_factor);
    log::info!("  Samples/cycle: {}", config.sv.samples_per_cycle);

    // Save example configuration
    config.to_json_file("ptoc_config.json")?;
    log::info!("Saved example configuration to ptoc_config.json");

    // Initialize components
    let mut ptoc = Ptoc::new(config.ptoc.clone());
    let scaler = CurrentScaler::new(config.adc.clone(), config.ct.clone());
    let mut sample_buffer = SvSampleBuffer::new(config.sv.samples_per_cycle);

    log::info!("\nSimulating overcurrent condition...");

    // Simulate receiving samples over time
    // Generate one cycle of samples with overcurrent (150A primary)
    let base_time = get_timestamp_micros();
    let sample_period_us = 250; // 4000 samples/sec = 250 microseconds per sample

    for cycle in 0..3 {
        log::info!("\n--- Cycle {} ---", cycle + 1);
        
        sample_buffer.clear();

        // Simulate 80 samples per cycle
        for sample_num in 0..config.sv.samples_per_cycle {
            // Simulate ADC value for sine wave with overcurrent
            // 150A primary = 0.375A secondary (with 400/1 CT)
            // 0.375A secondary = 375 ADC counts (with 0.001 scale factor)
            // Peak = 375 * sqrt(2) ≈ 530 counts
            let angle = 2.0 * std::f64::consts::PI * sample_num as f64 / config.sv.samples_per_cycle as f64;
            let peak_adc = 530.0;
            let adc_value = (peak_adc * angle.sin()) as i32;
            
            sample_buffer.add_sample(adc_value);
        }

        // Calculate RMS from accumulated samples
        if sample_buffer.is_full() {
            // Convert ADC samples to primary current and calculate RMS
            let primary_samples = scaler.scale_samples_to_primary(sample_buffer.samples());
            let rms_current = poc_protection_functions::calculate_rms(&primary_samples);
            
            log::info!("RMS Current: {:.2} A (primary)", rms_current);

            // Process through PTOC
            let timestamp = base_time + ((cycle + 1) * config.sv.samples_per_cycle) as u64 * sample_period_us;
            let result = ptoc.process(rms_current, timestamp);

            match result {
                ProtectionResult::NoTrip => {
                    log::info!("Status: No Trip");
                }
                ProtectionResult::TripPending(delay) => {
                    log::info!("Status: Trip Pending (remaining: {} ms)", delay.as_millis());
                }
                ProtectionResult::Trip => {
                    log::warn!("Status: TRIP!");
                    // In a real system, this would trigger GOOSE message
                }
                ProtectionResult::Disabled => {
                    log::info!("Status: Disabled");
                }
            }

            log::info!("PTOC State: {:?}", ptoc.state());
        }

        // Simulate time delay between cycles (20ms per cycle at 50Hz)
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    log::info!("\nSimulation complete!");
    log::info!("In a real deployment:");
    log::info!("  - SV subscriber would receive samples from network");
    log::info!("  - GOOSE publisher would send trip messages");
    log::info!("  - Integration with Omicron test equipment");

    Ok(())
}
