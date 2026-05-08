# axonos-swarm

**Distributed real-time coordination for AxonOS mesh nodes.**

Extends the single-node timing guarantees of [RFC-0004](https://github.com/AxonOS-org/axonos-rfcs/blob/main/rfcs/0004-dual-core-real-time-contract.md) to N nodes on a wireless mesh. Implements the Swarm Real-Time Contract (SC0–SC6) documented in [AxonOS Article #36](https://medium.com/@AxonOS/axonos-article-36-swarm-real-time-distributed-hard-deadlines-across-the-axon-protocol-mesh-4e30d730c2ad).

[![Crate](https://img.shields.io/badge/crate-axonos--swarm-orange)](https://github.com/AxonOS-org/axonos-swarm)
[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue)](#license)
[![no_std](https://img.shields.io/badge/no__std-yes-green)](https://doc.rust-lang.org/reference/names/preludes.html#the-no_std-attribute)

---

## What this crate provides

| Module | Type | Purpose |
|--------|------|---------|
| `neural_ptp` | `NeuralPtpKalman` | Kalman-filtered IEEE 1588 PTP — converges to ±18 µs (3σ) on BLE mesh |
| `swarm` | `SwarmScheduler` | Epoch start synchronisation — SC2 skew ≤ 100 µs |
| `fault` | `SwarmFaultDetector` | Silence / degradation / desync / Byzantine detection — SC4 |

---

## Swarm Real-Time Contract

| Clause | Guarantee | Bound |
|--------|-----------|-------|
| SC0 | Clock sync | ≤ 50 µs (3σ) between any two nodes |
| SC1 | Local pipeline (per RFC-0004) | WCRT ≤ 972 µs |
| SC2 | Synchronised epoch start | Skew ≤ 2 × σ_sync ≤ 100 µs |
| SC3 | Intent co-availability | All nodes' intents within ≤ 500 µs |
| SC4 | Fault detection | Silence ≤ 8 ms · Desync ≤ 100 ms |
| SC5 | Graceful degradation | N−1 nodes continue under SC1 |
| SC6 | Probabilistic cross-node delivery | P(T ≤ 14.5 ms) ≥ 0.999 |

**Evidence level:** `L1` — analytically derived from Kalman filter theory and empirical BLE mesh latency distribution. `L3` oscilloscope validation pending Q2 2026 hardware fixture.

---

## Quick start

```toml
[dependencies]
axonos-swarm = { git = "https://github.com/AxonOS-org/axonos-swarm" }
```

```rust
use axonos_swarm::neural_ptp::NeuralPtpKalman;
use axonos_swarm::swarm::SwarmScheduler;

// Initialise clock synchroniser
let mut ptp = NeuralPtpKalman::new();

// Each PTP exchange: predict → update
ptp.predict(0.1);              // 100 ms since last exchange
ptp.update(measured_offset);   // raw PTP offset measurement (µs)

// Check sync quality before enabling swarm coordination
if ptp.sync_quality() >= 0.5 {
    println!("SC0 satisfied: offset = {:.1} µs ± {:.1} µs (3σ)",
        ptp.offset_us(),
        ptp.offset_uncertainty_3sigma_us());
}
```

---

## Neural PTP convergence

| Environment | Raw PTP σ (µs) | Neural PTP σ (µs) | Improvement |
|---|---|---|---|
| Clean RF (lab) | 48 | 8 | 6.0× |
| Moderate interference (office) | 180 | 22 | 8.2× |
| High interference (EMC chamber) | 520 | 61 | 8.5× |

Source: Article #36, § "Empirical Characterisation of the Neural PTP Mesh". Evidence level: `L2` (runtime measured on BLE mesh hardware).

---

## `no_std` compatibility

The entire crate is `#![no_std]` with no heap allocation on the hot path. All state is fixed-size. Compile for Cortex-M with:

```bash
cargo build --target thumbv7em-none-eabihf
cargo build --target thumbv8m.main-none-eabihf
```

Enable `std` feature only for `Display` trait implementations (formatting only):

```toml
axonos-swarm = { ..., features = ["std"] }
```

---

## Relationship to other AxonOS crates

| Crate | Role |
|---|---|
| [`axonos-consent`](https://github.com/AxonOS-org/axonos-consent) | Per-peer consent state machine (Layer 2) |
| [`axonos-sdk`](https://github.com/AxonOS-org/axonos-sdk) | Application-facing intent API |
| **`axonos-swarm`** | Mesh timing and fault coordination |

`axonos-swarm` is a low-level building block. It does not depend on `axonos-consent` or `axonos-sdk`.

---

## References

1. IEEE 1588–2019. *Precision Clock Synchronization Protocol for Networked Measurement and Control Systems.*
2. Welch, G. & Bishop, G. (2006). *An Introduction to the Kalman Filter.* UNC TR 95–041.
3. Kopetz, H. (1997). *Real-Time Systems: Design Principles for Distributed Embedded Applications.*
4. Buttazzo, G. (2011). *Hard Real-Time Computing Systems* (3rd ed.). Springer.
5. AxonOS Article #36 — [Swarm Real-Time: Distributed Hard Deadlines Across the Axon Protocol Mesh](https://medium.com/@AxonOS/axonos-article-36-swarm-real-time-distributed-hard-deadlines-across-the-axon-protocol-mesh-4e30d730c2ad)

---

## License

Dual-licensed under **Apache-2.0 OR MIT** at your option.

© 2026 Denis Yermakou · [axonos.org](https://axonos.org) · [info@axonos.org](mailto:info@axonos.org)
