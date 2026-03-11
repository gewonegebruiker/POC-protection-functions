# Utility-Grade DSP Architecture for IEC 61850 Protection

## AI Context & Prompting Guide
**To the AI Assistant:** This document describes the architecture for upgrading an IEC 61850 protection relay (written in Rust) from a naive RMS calculation to a Utility-Grade Digital Signal Processing (DSP) pipeline. The goal is to implement Frequency Tracking, Dynamic Resampling, and a Recursive Discrete Fourier Transform (DFT). The code must execute within a strict 250µs real-time loop. Heap allocations (`Box`, `Vec::push`) are strictly forbidden in the hot path.

---

## 1. Problem Statement
The current implementation relies on a mathematical sliding-window RMS assuming a perfect 50.00 Hz grid frequency and perfectly sinusoidal waveforms. 
*   **DC Offset Vulnerability:** Fault currents contain exponentially decaying DC components. Naive RMS includes this DC energy, causing over-reaching (false trips).
*   **Frequency Drift (Leakage):** If the grid drops to 49.0 Hz, an 80-sample window (at 4000 Sa/s) no longer covers exactly one cycle. This causes the RMS value to oscillate.

## 2. Solution: The "Utility-Grade" Approach
To solve this, we decouple the physical sample rate (4000 Hz) from the logical sample rate (80 samples per actual cycle).
1.  **Frequency Tracker:** Measures the actual grid frequency using zero-crossings.
2.  **Software Resampler:** Interpolates the 4000 Hz fixed-rate SV stream into a variable-rate stream so we always output exactly 80 samples per fundamental cycle, regardless of grid frequency.
3.  **Recursive O(1) DFT:** Acts as a band-pass filter to extract only the fundamental (50Hz) magnitude, entirely rejecting DC offsets and harmonics. Because the Resampler guarantees exactly 80 samples per cycle, the DFT weights can remain perfectly static and hardcoded.

### System Data Flow
```text
┌──────────────┐   ┌───────────────┐   ┌───────────────┐   ┌──────────────┐   ┌────────┐
│  SV Stream   │──▶│ Frequency     │──▶│   Resampler   │──▶│ O(1) Sliding │──▶│ PTOC / │
│ (Fixed 4kHz) │   │ Tracker       │   │ (Interpolate) │   │ Window DFT   │   │ PIOC   │
└──────────────┘   └───────────────┘   └───────────────┘   └──────────────┘   └────────┘
```

---

## 3. Module Specifications & Interfaces

### 3.1. `src/measurement/frequency.rs`
**Purpose:** Detect true grid frequency by measuring the time between zero-crossings.
*   **Logic:** Look for a sign change (negative to positive). Use linear interpolation to find the exact fractional timestamp of the zero-crossing. Apply a low-pass filter (e.g., Exponential Moving Average) to the calculated frequency to prevent jitter.
*   **Fallback:** If no zero-crossings occur (e.g., line is dead), default back to the nominal frequency (50.0 Hz).

```rust
pub struct FrequencyTracker {
    nominal_freq: f64,
    current_freq: f64,
    last_sample: f64,
    samples_since_cross: f64,
}

impl FrequencyTracker {
    pub fn new(nominal_freq: f64) -> Self;
    
    /// Feeds a raw SV sample. Returns the currently tracked frequency.
    pub fn update(&mut self, sample: f64) -> f64;
}
```

### 3.2. `src/measurement/resampler.rs`
**Purpose:** Convert the fixed 4000 Hz input into a stream that provides exactly `N` (80) samples per tracked grid cycle.
*   **Logic:** 
    *   Maintain a `phase_accumulator`. 
    *   Every raw SV sample advances time by `1.0 / 4000.0` seconds.
    *   The "virtual" sample interval is `1.0 / (80.0 * current_freq)`.
    *   When the accumulated time exceeds the virtual interval, emit an interpolated sample.
    *   *Note:* Because the grid frequency might be > 50Hz, one physical SV sample might trigger 0, 1, or (rarely) 2 virtual samples. Return a `Vec` or (better for RT) an `ArrayVec` / fixed-size array of emitted samples.

```rust
pub struct Resampler {
    nominal_sample_rate: f64,
    samples_per_cycle: usize,
    accumulated_time: f64,
    last_sample: f64,
}

impl Resampler {
    pub fn new(nominal_sample_rate: f64, samples_per_cycle: usize) -> Self;

    /// Takes a raw SV sample and the current tracked frequency.
    /// Returns 0, 1, or 2 interpolated virtual samples.
    pub fn process_sample(&mut self, raw_sample: f64, current_freq: f64) -> Option<f64>;
}
```

### 3.3. `src/measurement/dft.rs`
**Purpose:** Extract the fundamental AC magnitude using an O(1) sliding window Discrete Fourier Transform.
*   **Logic:**
    *   Normally, DFT requires $O(N)$ operations per sample. 
    *   **Recursive O(1) optimization:** 
        $X_{new} = X_{old} + (x_{new} - x_{oldest}) \cdot (\cos(\theta) - j\sin(\theta))$
    *   Pre-calculate cosine and sine arrays for $k=1$ (fundamental frequency).
    *   Maintain a running sum of `real` and `imag` components.
    *   Magnitude = $\sqrt{real^2 + imag^2} \times \frac{\sqrt{2}}{N}$ (to convert peak to RMS).

```rust
pub struct RecursiveDft {
    window_size: usize,
    buffer: Vec<f64>,      // Pre-allocate to window_size! No growing.
    index: usize,
    cos_weights: Vec<f64>, // Pre-calculated
    sin_weights: Vec<f64>, // Pre-calculated
    sum_real: f64,
    sum_imag: f64,
}

impl RecursiveDft {
    pub fn new(window_size: usize) -> Self;

    /// Feed a virtual sample from the resampler.
    /// Returns the fundamental RMS magnitude.
    pub fn update(&mut self, virtual_sample: f64) -> f64;
}
```

---

## 4. Integration into the Hot Path (`src/main.rs`)

When replacing `rms.rs` with this pipeline, the event loop structure shifts. 

**Old Loop:**
```rust
let sample = sv.receive_sample()?;
let primary = adc_to_primary(sample);
let rms = rms_calculator.update(primary);
ptoc.process(rms);
```

**New Loop (Target Implementation):**
```rust
let sample = sv.receive_sample()?;
let primary = adc_to_primary(sample);

// 1. Track physical frequency
let freq = freq_tracker.update(primary);

// 2. Resample (might yield 0 or 1 samples depending on phase accumulator)
if let Some(virtual_sample) = resampler.process_sample(primary, freq) {
    // 3. Calculate DFT ONLY when a virtual sample is emitted
    let fundamental_rms = dft.update(virtual_sample);
    
    // 4. Run protection logic using the pure, filtered magnitude
    ptoc.process(fundamental_rms);
}

// 5. GOOSE processing continues as normal
```

## 5. Implementation Rules for AI
1. **Strict Real-Time:** Do not use `std::sync::Mutex`, `Box::new()`, or dynamically sized `Vec` in `update()` methods. Allocate all buffers in the `new()` constructors.
2. **Math Efficiency:** Cache `sqrt(2) / N` as a constant multiplier.
3. **Data Types:** Use `f64` for all DSP calculations to prevent precision loss in the recursive DFT state over long uptimes.
4. **Phase Alignment:** The phase of the first sample does not matter, as the DFT calculates the absolute magnitude $\sqrt{Re^2 + Im^2}$.
