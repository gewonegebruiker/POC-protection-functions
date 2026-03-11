# Utility-Grade DSP Architecture for IEC 61850 Protection

## AI Context & Prompting Guide

**To the AI Assistant:** This document describes the architecture for upgrading an IEC 61850 protection relay (written in Rust) from a naive RMS calculation to a Utility-Grade Digital Signal Processing (DSP) pipeline. The goal is to implement DFT-based Frequency Tracking, Dynamic Resampling, and a Multi-Harmonic Recursive Discrete Fourier Transform (DFT). The code must execute within a strict 250 µs real-time loop. Heap allocations (`Box`, `Vec::push`) are strictly forbidden in the hot path.

**Document purpose:** This document serves two roles simultaneously:

1. **Technical architecture specification** — canonical reference for all DSP modules, their APIs, data flow, and integration requirements.
2. **AI prompting guide** — sufficient context for an AI assistant to implement any module without needing to read the rest of the codebase.

**Key constraints to keep in mind at all times:**
- Physical sample rate: 4000 Sa/s (80 samples per 50 Hz cycle)
- Logical virtual rate: exactly 80 samples per actual grid cycle (maintained by the resampler)
- Real-time budget per sample: ≤ 250 µs total (all phases, all modules)
- No heap allocation in any hot-path method (`update`, `process_sample`, etc.)
- PIOC is an instantaneous protection function and **bypasses the entire DSP pipeline**
- The DFT feeds the frequency tracker (feedback loop); the frequency tracker feeds the resampler

---

## 1. Problem Statement

The current implementation relies on a sliding-window RMS calculation that assumes a perfect 50.00 Hz grid frequency and a perfectly sinusoidal waveform.

- **DC Offset Vulnerability:** Fault currents contain exponentially decaying DC components. Naive RMS includes this DC energy, causing over-reaching (false trips).
- **Frequency Drift (Spectral Leakage):** If the grid drifts to 49.0 Hz, an 80-sample window at 4000 Sa/s no longer covers exactly one cycle. This causes the RMS value to oscillate with a beat frequency.
- **No Harmonic Separation:** The naive RMS cannot distinguish the fundamental from harmonic distortion. Future functions (transformer inrush blocking, CT saturation detection) require individual harmonic magnitudes.

---

## 2. Solution: The "Utility-Grade" Approach

To solve this, we decouple the physical sample rate (4000 Hz) from the logical sample rate (80 samples per actual cycle) and use the DFT phase output to drive frequency tracking.

1. **DFT-Based Frequency Tracker:** Derives the actual grid frequency from the phase angle of the fundamental DFT bin — not from zero-crossings. This is robust during faults (DC offset, CT saturation) and at light load (noise-immune).
2. **Software Resampler:** Interpolates the 4000 Hz fixed-rate SV stream into a variable-rate stream that always delivers exactly 80 samples per fundamental cycle, regardless of grid frequency.
3. **Multi-Harmonic Recursive DFT:** Extracts the fundamental magnitude (for PTOC) and up to 7 additional harmonic bins at O(1) per bin per sample. Acts as a set of narrow band-pass filters, entirely rejecting DC and all non-tracked harmonics.
4. **PIOC Bypass:** PIOC operates directly on the raw scaled sample, completely bypassing the DSP pipeline, guaranteeing sub-3 ms response.

### System Data Flow

```text
                         ┌─────────────────────────────────────────────┐
                         │              Frequency Feedback              │
                         ▼                                             │
┌──────────────┐   ┌───────────────┐   ┌──────────────┐   ┌──────────┴───┐
│  SV Stream   │──▶│   Resampler   │──▶│ Multi-Bin    │──▶│  Frequency   │
│ (Fixed 4kHz) │   │ (Interpolate) │   │ Sliding DFT  │   │  Tracker     │
└──────┬───────┘   └───────────────┘   └──────┬───────┘   └──────────────┘
       │                                      │
       │                                      ├──▶ Fundamental RMS → PTOC
       │                                      ├──▶ Harmonics → Future use
       └──────────────────────────────────────┴──▶ Raw sample → PIOC (bypass)
```

The feedback loop is: **DFT phase angle → FrequencyTracker → Resampler → DFT**. During startup, `nominal_freq` is used until the DFT has completed its first full window.

---

## 3. Module Specifications & Interfaces

### 3.1. `src/measurement/frequency.rs` — DFT-Based Frequency Tracker

**Purpose:** Derive the true grid frequency from the phase angle of the fundamental DFT bin after each complete DFT window.

**Why DFT-based (not zero-crossing):** Voltage is not always available in this architecture (only current SV streams). Zero-crossing on current is unreliable during faults (DC offset, CT saturation, and harmonics shift the crossing point) and at light load (noise causes jitter).

**Algorithm:**
- After each complete DFT window, receive `(sum_real, sum_imag)` from the DFT.
- Compute phase angle: `φ = atan2(imag, real)`.
- Compute phase change: `Δφ = φ − φ_prev` (unwrap to `(−π, π]`).
- Convert to frequency: `f_tracked = f_nominal + Δφ / (2π · Δt)`, where `Δt` is the time between consecutive DFT windows (= `samples_per_cycle / sample_rate`).
- Apply EMA smoothing: `f_ema = α · f_tracked + (1 − α) · f_ema_prev`.
- **Fallback:** If DFT magnitude is below `noise_threshold` (dead line), output `nominal_freq`.
- **Startup:** Output `nominal_freq` until at least 2 full DFT windows have been processed (needed for valid Δφ).

**EMA smoothing constant:** α = 0.1 (default, configurable). This gives an effective window of ~10 DFT windows ≈ 200 ms settling time.
- α too small → slow tracking during real frequency events (generator islanding)
- α too large → frequency estimate jitters from DFT magnitude noise

**Tracking range:** 47–53 Hz (outside this range, clamp to nearest bound and raise a diagnostic).

```rust
pub struct FrequencyTracker {
    nominal_freq: f64,
    current_freq: f64,
    last_phase: f64,
    last_phase_valid: bool,
    ema_alpha: f64,           // Smoothing constant (default 0.1)
    noise_threshold: f64,     // Minimum DFT magnitude to trust phase
    samples_per_cycle: usize, // To compute Δt from window period
    sample_rate: f64,         // Physical sample rate (4000.0)
}

impl FrequencyTracker {
    pub fn new(nominal_freq: f64, sample_rate: f64, samples_per_cycle: usize) -> Self;

    /// Called once per DFT window with the DFT's real and imaginary sums
    /// and the DFT magnitude. Returns the tracked frequency.
    pub fn update(&mut self, dft_real: f64, dft_imag: f64, dft_magnitude: f64) -> f64;
}
```

---

### 3.2. `src/measurement/resampler.rs` — Variable-Rate Resampler

**Purpose:** Convert the fixed 4000 Hz SV input into a virtual stream that provides exactly 80 samples per tracked grid cycle, regardless of grid frequency.

**Algorithm:**
- Maintain `accumulated_time`: the elapsed physical time since the last virtual sample was emitted.
- Each raw SV sample advances time by `1.0 / sample_rate` seconds.
- The virtual sample interval is `1.0 / (samples_per_cycle × current_freq)` seconds.
- When `accumulated_time ≥ virtual_interval`, emit a linearly interpolated sample and subtract `virtual_interval` from `accumulated_time`.
- If the remaining `accumulated_time` is still ≥ `virtual_interval` (possible when `current_freq > nominal_freq`), emit a second sample. Hence the return type is `ArrayVec<f64, 2>`.
- Use linear interpolation between `last_sample` and `raw_sample` at the fractional time position.
- **Startup:** Use `nominal_freq` until the frequency tracker produces its first valid output.

**Return type:** `ArrayVec<f64, 2>` — zero heap allocation, returns 0, 1, or 2 virtual samples per call. `Option<f64>` is insufficient: at frequencies above 50 Hz (e.g. 52 Hz → virtual rate 4160 Sa/s > physical 4000 Sa/s), two virtual samples may need to be emitted for a single physical sample, and returning only `Option<f64>` would silently drop one.

```rust
use arrayvec::ArrayVec;

pub struct Resampler {
    nominal_sample_rate: f64,
    samples_per_cycle: usize,
    accumulated_time: f64,
    last_sample: f64,
    last_time: f64,
}

impl Resampler {
    pub fn new(nominal_sample_rate: f64, samples_per_cycle: usize) -> Self;

    /// Takes a raw SV sample and the current tracked frequency.
    /// Returns 0, 1, or 2 interpolated virtual samples (no heap allocation).
    pub fn process_sample(&mut self, raw_sample: f64, current_freq: f64) -> ArrayVec<f64, 2>;
}
```

---

### 3.3. `src/measurement/dft.rs` — Multi-Harmonic Recursive DFT

**Purpose:** Extract the fundamental AC magnitude and multiple harmonic magnitudes using an O(1) sliding-window DFT per harmonic bin.

**Recursive O(1) update formula (per harmonic bin `k`):**

```
X_k[new] = X_k[old] + (x_new − x_oldest) × (cos(2π·k·n/N) − j·sin(2π·k·n/N))
```

Pre-calculate all cosine and sine weights at construction time. No trigonometric calls in `update()`.

**Harmonic bins:**

| Bin | Harmonic | Use case | When needed |
|-----|----------|----------|-------------|
| k=0 | DC component | DC offset monitoring, CT saturation detection | Phase 2 |
| k=1 | Fundamental (50/60 Hz) | PTOC magnitude, frequency tracking | **Now** |
| k=2 | 2nd harmonic | Transformer inrush blocking (I₂/I₁ > 15% → block trip) | Phase 2 (PDIF) |
| k=3 | 3rd harmonic | Zero-sequence harmonic analysis | Future |
| k=4 | 4th harmonic | Reserved | Future |
| k=5 | 5th harmonic | Capacitor switching restraint, CT saturation detection | Phase 2 |
| k=6 | 6th harmonic | Reserved | Future |
| k=7 | 7th harmonic | Power quality, harmonic distortion reporting | Future |

Extracting all 8 bins costs `8 × 4 = 32` floating-point operations per sample — well within the 250 µs budget.

**Periodic recalibration:** The recursive accumulation of `sum_real` and `sum_imag` allows f64 rounding errors to compound over hours/days of operation. Every `recalibration_interval` windows (default: 100 = 2 seconds), recompute `sum_real` and `sum_imag` from scratch using the full O(N) batch formula over the current ring buffer contents. This costs 80 multiply-adds every 2 seconds — negligible.

**Const generics:** The buffer size `N` is a const generic parameter. This guarantees at compile time that no heap allocation occurs and that the buffer size cannot be accidentally changed at runtime.

```rust
pub const MAX_HARMONICS: usize = 8;

pub struct DftResult {
    /// Fundamental (k=1) RMS magnitude
    pub fundamental_rms: f64,
    /// Fundamental real component (for frequency tracking)
    pub fundamental_real: f64,
    /// Fundamental imaginary component (for frequency tracking)
    pub fundamental_imag: f64,
    /// Whether this update completed a full window
    pub window_complete: bool,
    /// Harmonic RMS magnitudes [k=0 (DC), k=1 (fund), k=2 (2nd), ..., k=7 (7th)]
    /// Valid only after window_complete has been true at least once.
    pub harmonic_rms: [f64; MAX_HARMONICS],
}

pub struct RecursiveDft<const N: usize> {
    buffer: [f64; N],
    index: usize,
    count: usize,
    cos_weights: [[f64; N]; MAX_HARMONICS],  // Pre-calculated per harmonic
    sin_weights: [[f64; N]; MAX_HARMONICS],  // Pre-calculated per harmonic
    sum_real: [f64; MAX_HARMONICS],
    sum_imag: [f64; MAX_HARMONICS],
    rms_scale: f64,                          // Cached sqrt(2) / N
    recalibration_counter: usize,
    recalibration_interval: usize,           // Default: 100 windows
}

impl<const N: usize> RecursiveDft<N> {
    pub fn new(recalibration_interval: usize) -> Self;

    /// Feed a virtual sample from the resampler.
    /// Returns DftResult with fundamental + harmonic magnitudes.
    pub fn update(&mut self, virtual_sample: f64) -> DftResult;
}
```

---

## 4. Integration into the Hot Path (`src/main.rs`)

When replacing `rms.rs` with this pipeline, the event loop structure shifts to two separate paths.

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

// === PIOC: instantaneous path (bypasses DSP pipeline entirely) ===
let pioc_result = pioc.process(primary.abs(), t);

// === PTOC: DSP pipeline path ===
for virtual_sample in resampler.process_sample(primary, current_freq) {
    let dft_result = dft.update(virtual_sample);
    let fundamental_rms = dft_result.fundamental_rms;

    // Update frequency tracker from DFT phase (once per completed window)
    if dft_result.window_complete {
        current_freq = freq_tracker.update(
            dft_result.fundamental_real,
            dft_result.fundamental_imag,
            fundamental_rms,
        );
    }

    ptoc.process(fundamental_rms, t);
}

// === GOOSE: publish if trip state changed ===
```

**Why PIOC bypasses the DSP pipeline:** PIOC is an instantaneous protection function (P1 class, response ≤ 3 ms). The DFT requires a full window (80 samples = 20 ms) before producing a valid output. PIOC MUST operate on the raw instantaneous scaled sample to meet its timing requirement.

---

## 5. Implementation Rules for AI

1. **Strict Real-Time:** Do not use `std::sync::Mutex`, `Box::new()`, or dynamically sized `Vec` in any `update()` or `process_sample()` method. Allocate all buffers in the `new()` constructors.
2. **Const Generics for Buffers:** Use `[f64; N]` (const generic) for all fixed-size arrays, not `Vec<f64>`. This enforces no-heap-allocation at compile time.
3. **Math Efficiency:** Cache `sqrt(2) / N` as `rms_scale`. Cache all trigonometric values in `cos_weights` and `sin_weights` during `new()`.
4. **Data Types:** Use `f64` for all DSP calculations to prevent precision accumulation errors in the recursive DFT state over long uptimes.
5. **Phase Alignment:** The phase of the first sample does not matter. The DFT computes the absolute magnitude `sqrt(Re² + Im²)`.
6. **Feedback Loop Initialization:** During startup, use `nominal_freq` in both the resampler and the frequency tracker until the DFT has completed its first full window.
7. **Recalibration:** Implement the periodic full O(N) recalibration in `RecursiveDft` — do not omit it. It prevents long-uptime drift.
8. **EMA Alpha:** Make `ema_alpha` configurable in `FrequencyTracker::new()`. Default to 0.1. Document in comments that this gives ~200 ms settling time.

---

## 6. Startup & Warm-Up Behavior

The DSP pipeline has a defined warm-up sequence. Protection functions must respect these states.

| Module | Warm-Up Condition | Behavior During Warm-Up |
|--------|-------------------|------------------------|
| **Resampler** | Uses `nominal_freq` until `FrequencyTracker` produces its first valid update | Outputs virtual samples at nominal rate; functionally correct, slightly inaccurate |
| **RecursiveDft** | `count < N` (fewer than 80 virtual samples received) | `DftResult::fundamental_rms = 0.0`, `window_complete = false`; mark output as not valid |
| **FrequencyTracker** | Fewer than 2 complete DFT windows processed | Outputs `nominal_freq`; `last_phase_valid = false` |
| **PTOC** | DFT warm-up not complete | Remains in `Idle` state; no trip possible |
| **PIOC** | None | Active immediately on first sample; no warm-up |

**Maximum warm-up duration:** 2 cycles (40 ms at 50 Hz, covering the first complete DFT window and one additional window for Δφ). If the DFT has not produced a valid output after 100 ms, raise a diagnostic alarm (`DSP_WARMUP_TIMEOUT`).

---

## 7. Three-Phase Instantiation

The DSP pipeline must be instantiated independently per phase. The `FrequencyTracker` is shared.

**Rationale:**
- During asymmetric faults, individual phases carry different distortion levels. Independent `Resampler` + `DFT` chains allow each phase to be processed independently.
- The `FrequencyTracker` is fed from all three phase DFT outputs (use the phase with the highest DFT magnitude as the authoritative source, or use the average of valid phases).
- A single shared frequency estimate is sufficient because grid frequency is a system-wide quantity.

```text
Phase A SV ──▶ Resampler_A ──▶ DFT_A ──▶ PTOC_A
Phase B SV ──▶ Resampler_B ──▶ DFT_B ──▶ PTOC_B    ←── shared FrequencyTracker
Phase C SV ──▶ Resampler_C ──▶ DFT_C ──▶ PTOC_C
                                          │
                                   I₀ = I_a + I_b + I_c (from DFT fundamentals)
                                          │
                                          ▼
                                       PTOC_N (neutral/ground)
```

**Residual current (I₀):** The neutral/ground current can be computed directly from the three fundamental DFT outputs: `I0_phasor = DFT_A_phasor + DFT_B_phasor + DFT_C_phasor`. This avoids a fourth physical SV channel while still providing ground-fault protection.

**Memory footprint (per phase, N=80):**
- `RecursiveDft<80>`: `8 × 80 × 2 × 8` bytes (cos/sin weights) + `80 × 8` bytes (buffer) = ~11 KB per phase
- Three phases = ~33 KB total — well within embedded Linux constraints

---

## 8. Validation Criteria

Acceptance criteria for the DSP pipeline. All tests should use a synthetic sine wave generator at known frequency, amplitude, and harmonic content, comparing DFT output against a reference batch FFT.

| Parameter | Requirement |
|-----------|-------------|
| Magnitude error (steady state, pure sine) | < 0.1% vs batch DFT |
| Magnitude error (steady state, with 5% THD) | < 0.5% vs batch DFT |
| Magnitude settling time (step fault to 90%) | < 1 cycle (20 ms) |
| DC offset rejection | > 40 dB (< 1% of DC appears in fundamental) |
| Frequency tracking range | 47–53 Hz |
| Frequency tracking step response (to 90%) | < 5 cycles (100 ms) |
| Per-sample computation time (all 8 harmonic bins) | < 5 µs |
| Recalibration overhead (every 100 windows) | < 50 µs |

---

## 9. Known Limitations

The following limitations are documented for transparency. They do not affect the POC scope but must be considered before production deployment.

**CT Saturation:**
During heavy faults, CT cores can saturate, clipping the current waveform. The DFT will report a reduced fundamental magnitude, potentially causing under-reach (failure to trip on a fault within the protection zone). CT saturation detection using 2nd/5th harmonic ratios is planned for Phase 2 but is not implemented in this POC.

**Linear Interpolation in Resampler:**
The resampler uses linear interpolation between adjacent samples. This introduces approximately 0.08% amplitude error at peaks for a pure sine wave. With harmonics present, this error rises to 0.5–1%. This is acceptable for PTOC (which has generous pickup margins). If Phase-2 functions (differential protection PDIF, distance protection PDIS) are added, consider upgrading to cubic or Hermite interpolation for improved accuracy.

**Single-Frequency Tracking Assumption:**
The `FrequencyTracker` assumes a single dominant fundamental frequency component. During inter-area power oscillations or sub-synchronous resonance events, the assumption may not hold and the tracker may report an unstable frequency. This condition is out of scope for this POC and should be documented as a known gap in production deployment reviews.
