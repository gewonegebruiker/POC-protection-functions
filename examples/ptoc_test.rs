/// Simple PTOC test example
use poc_protection_functions::{
    PtocConfig, Ptoc, ProtectionFunction,
};

fn main() {
    println!("PTOC Test Example\n");

    // Create PTOC with 100A pickup and 100ms delay
    let config = PtocConfig {
        iset: 100.0,
        tset: 100,
        enabled: true,
    };
    
    let mut ptoc = Ptoc::new(config);
    
    println!("Configuration:");
    println!("  Pickup (Iset): {} A", ptoc.iset());
    println!("  Time Delay (Tset): {} ms", ptoc.tset());
    println!();

    // Test scenario 1: Current below pickup
    println!("Test 1: Current below pickup (50A)");
    let result = ptoc.process(50.0, 0);
    println!("  Result: {:?}", result);
    println!("  State: {:?}", ptoc.state());
    println!();

    // Test scenario 2: Current exceeds pickup
    println!("Test 2: Current exceeds pickup (150A at t=0)");
    let result = ptoc.process(150.0, 0);
    println!("  Result: {:?}", result);
    println!("  State: {:?}", ptoc.state());
    println!();

    // Test scenario 3: Still overcurrent, but delay not expired
    println!("Test 3: Still overcurrent at t=50ms");
    let result = ptoc.process(150.0, 50_000); // 50ms = 50000 microseconds
    println!("  Result: {:?}", result);
    println!("  State: {:?}", ptoc.state());
    println!();

    // Test scenario 4: Delay expired, should trip
    println!("Test 4: Still overcurrent at t=100ms (delay expired)");
    let result = ptoc.process(150.0, 100_000); // 100ms = 100000 microseconds
    println!("  Result: {:?}", result);
    println!("  State: {:?}", ptoc.state());
    println!();

    // Test scenario 5: Reset and try again
    println!("Test 5: Reset and test with current drop");
    ptoc.reset();
    ptoc.process(150.0, 0); // Pickup at t=0
    println!("  Pickup at t=0");
    let result = ptoc.process(50.0, 50_000); // Current drops before delay
    println!("  Current drops to 50A at t=50ms");
    println!("  Result: {:?}", result);
    println!("  State: {:?}", ptoc.state());
    println!();

    println!("Test complete!");
}
