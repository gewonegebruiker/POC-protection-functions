# Expert Review Panel Feedback

## 1. The PAC Engineer
*(Protection, Automation, and Control Expert)*

> "The logic works for ideal signals, but in a real substation, bad data is as common as faults. This relay might misoperate during maintenance or clock shifts."

### Critical: Missing Quality Check
*   **Issue:** In `src/io/sv_input.rs`, the code extracts `sample.value` but completely ignores the Quality (`q`) field from the Sampled Values stream.
*   **Risk:** If a Merging Unit (MU) signals "Invalid" or "Test" mode, this relay will treat the data as valid zero or frozen values, potentially causing a false trip or failure to trip.
*   **Suggestion:** Check the `q` attribute. Block protection if validity is not `Good`.

### Unsafe Time Reference
*   **Issue:** The protection logic relies on `SystemTime::now()` (Wall Clock).
*   **Risk:** If NTP steps the clock backward (or forward), inverse-time curves will calculate incorrect delays.
*   **Suggestion:** Use `std::time::Instant` (monotonic clock) for relative delta calculations (timers) and only use PTP-disciplined absolute time for event timestamping.

### Simplistic Reset Characteristic
*   **Issue:** `src/protection/ptoc.rs` immediately resets to `Idle` when current drops below the dropout threshold.
*   **Gap:** Electromechanical relays have a "cooling" time (Electromechanical Reset). For intermittent faults, the thermal accumulation should decay slowly, not instantly reset.

### Missing Phases
*   **Gap:** The code processes single-phase logic well, but real PTOC/PIOC usually involves Ground/Neutral (calculated 3I0) elements which are currently missing.

## 2. The Solution Architect
*(Systems Integration & SEAPATH Expert)*

> "The application assumes it owns the whole CPU. In a virtualized SEAPATH environment, we need better resource citizenship."

### Busy-Wait Loop
*   **Issue:** In `src/main.rs`, the code performs a busy-wait spin loop when `sv.receive_sample()` returns `WouldBlock`.
*   **Impact:** This pins a CPU core to 100% usage even if network traffic is low. In a virtualized environment (SEAPATH), this starves other VMs (like HMI or Gateway).
*   **Suggestion:** Use `epoll` (via `mio` or `tokio`) or at least `std::thread::yield_now()` to yield the timeslice when no data is available.

### Scalability
*   **Issue:** The architecture is single-threaded. Receiving SV, processing logic, and sending GOOSE happen sequentially.
*   **Risk:** As more bays or protection functions (Distance, Differential) are added, the cycle time will exceed the 250µs (4kHz) budget.
*   **Suggestion:** Decouple I/O from Logic. Use a ring buffer (like `crossbeam-channel` or a lock-free queue) to pass samples from the Network Thread to a dedicated Protection Thread.

## 3. The Software Architect
*(Rust & Performance Expert)*

> "The code is clean and idiomatic Rust, but the error handling strategy in the hot loop is risky."

### Error Handling in Hot Path
*   **Issue:** The `sv.receive_sample()` method returns `Result<T, Box<dyn Error>>`.
*   **Impact:** Allocating a `Box<dyn Error>` on the heap for every receive error (even minor ones) causes non-deterministic latency spikes (GC/Allocator pressure) in a real-time path.
*   **Suggestion:** Return a lightweight `enum` for expected runtime errors (`WouldBlock`, `PacketTooSmall`, `InvalidHeader`) to keep the stack purely allocated.

### Raw Sockets & Security
*   **Issue:** The app requires `CAP_NET_RAW` or root because it builds its own Ethernet headers.
*   **Suggestion:** This is necessary for GOOSE/SV, but consider dropping privileges after the socket `bind()` call to improve security posture.

### Logging Blocking I/O
*   **Issue:** `log::info!` is used in the simulation loop.
*   **Impact:** Standard logging (writing to stdout/disk) is a blocking I/O operation. If done inside the protection loop, it will violate real-time constraints.
*   **Suggestion:** Use a non-blocking logging facade or a ring-buffer logger that writes to disk in a separate low-priority thread.