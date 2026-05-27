# axonos-swarm

[![CI](https://github.com/AxonOS-org/axonos-swarm/actions/workflows/ci.yml/badge.svg)](https://github.com/AxonOS-org/axonos-swarm/actions/workflows/ci.yml)
[![Crate](https://img.shields.io/badge/Crate-v0.2.1-0a4a8f?style=flat-square)](https://github.com/AxonOS-org/axonos-swarm/releases/tag/v0.2.1)
[![Rust](https://img.shields.io/badge/Rust-no__std-CE422B?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-Apache--2.0%20OR%20MIT-475569?style=flat-square)](#license)
[![Status](https://img.shields.io/badge/Status-Pre--certification-475569?style=flat-square)](#repository-status)
[![Standard](https://img.shields.io/badge/Standard-v1.0.0-0a4a8f?style=flat-square)](https://github.com/AxonOS-org/axonos-standard)

**Real-time coordination layer for AxonOS mesh nodes.**

`axonos-swarm` extends AxonOS from a single deterministic BCI node to a
small distributed mesh of real-time nodes. It provides clock synchronisation,
synchronised epoch release, co-availability window estimation, and bounded
fault detection for distributed BCI pipelines.

This repository is not an AI-agent swarm, not a consensus system, and not a
clinical deployment baseline. It is a low-level validation-oriented Rust crate for
testing whether AxonOS timing guarantees can be lifted from one node to N
coordinated nodes.

---

## Position in the AxonOS stack

AxonOS is the deterministic operating layer between neural hardware and
intelligent applications: an operating system substrate for brain-computer
interfaces.

Within that architecture:

| Layer | Repository | Role |
|---|---|---|
| Canonical standard | [`axonos-standard`](https://github.com/AxonOS-org/axonos-standard) | Architecture manual, conformance criteria, validation taxonomy |
| Engineering RFCs | [`axonos-rfcs`](https://github.com/AxonOS-org/axonos-rfcs) | Numbered design proposals; normative once finalised |
| Kernel substrate | [`axonos-kernel`](https://github.com/AxonOS-org/axonos-kernel) | EDF scheduling, SPSC IPC, capability gate, monotonic time |
| Application boundary | [`axonos-sdk`](https://github.com/AxonOS-org/axonos-sdk) | Typed intents, manifests, ABI-compatible integration |
| Consent layer | [`axonos-consent`](https://github.com/AxonOS-org/axonos-consent) | Deterministic consent state machine and stimulation-gating protocol |
| Mesh coordination | **`axonos-swarm`** | Distributed timing, co-availability, and peer health monitoring |

`axonos-swarm` is deliberately independent from `axonos-sdk` and
`axonos-consent`. It can be reviewed as a small timing and fault-coordination
crate without pulling in the full AxonOS application stack.

---

## What this crate provides

| Module | Main type | Purpose |
|---|---|---|
| `neural_ptp` | `NeuralPtpKalman` | Kalman-filtered clock-offset estimator for noisy wireless PTP-style measurements |
| `swarm` | `SwarmScheduler` | Computes local release times for globally synchronised 4 ms swarm epochs |
| `fault` | `SwarmFaultDetector` | Tracks silence, degradation, desynchronisation, and Byzantine-like peer behaviour |

The crate is designed for `#![no_std]` environments and avoids heap allocation
on the real-time path. The optional `std` feature is used only for host-side
formatting and development ergonomics.

---

## Swarm Real-Time Contract

The swarm contract is stated as SC0–SC6. These are engineering contracts, not
clinical certification claims.

| Clause | Guarantee | Current status |
|---|---|---|
| SC0 | Clock-offset uncertainty remains within a bounded 3σ envelope | Analytically modelled; hardware validation pending |
| SC1 | Each node preserves its local AxonOS pipeline WCRT budget | Inherited from the single-node kernel/RFC model |
| SC2 | Nodes release their pipeline epochs against a shared global epoch | Implemented in `SwarmScheduler` |
| SC3 | Intent outputs become co-available within a bounded window | Implemented as a conservative window calculation |
| SC4 | Silent, degraded, desynchronised, or inconsistent peers are detected | Implemented in `SwarmFaultDetector` |
| SC5 | A degraded mesh can fall back to local-only operation | Architectural rule; integration policy pending |
| SC6 | Cross-node delivery is bounded probabilistically | Research target; not yet a hard runtime claim |

### Evidence posture

AxonOS distinguishes between claims by evidence level:

| Evidence level | Meaning in this repository |
|---|---|
| L1 | Analytical model, unit tests, and deterministic code-level checks |
| L2 | Runtime measurement on a development fixture or controlled harness |
| L3 | External instrumentation, GPIO/oscilloscope trace, or independent validation |

Current status: `axonos-swarm` should be read as **L1/L2-oriented validation infrastructure**. It is not yet an L3-validated distributed BCI runtime.

---

## Design constraints

`axonos-swarm` follows the same engineering constraints as the rest of AxonOS:

1. No allocator on the real-time path.
2. No hidden background coordination state.
3. No unbounded retry loops in fault-sensitive paths.
4. No claim above its evidence level.
5. No dependency on application-layer trust for timing correctness.

The crate is small on purpose. It should remain reviewable by an embedded
systems engineer, a real-time systems reviewer, or a safety assessor.

---

## Quick start

Add the crate from GitHub:

```toml
[dependencies]
axonos-swarm = { git = "https://github.com/AxonOS-org/axonos-swarm" }
```

Example: update the clock synchroniser from a PTP-style offset measurement.

```rust
use axonos_swarm::neural_ptp::NeuralPtpKalman;

let mut ptp = NeuralPtpKalman::new();

ptp.predict(0.1);
ptp.update(42.0);

let offset_us = ptp.offset_us();
let uncertainty_us = ptp.offset_uncertainty_3sigma_us();
let quality = ptp.sync_quality();

println!(
    "offset = {:.1} µs, uncertainty = {:.1} µs, quality = {:.2}",
    offset_us,
    uncertainty_us,
    quality
);
```

Example: compute a co-availability window for a synchronised swarm epoch.

```rust
use axonos_swarm::swarm::SwarmScheduler;

let mut scheduler = SwarmScheduler::new(0);

for _ in 0..300 {
    scheduler.ptp_mut().predict(0.1);
    scheduler.ptp_mut().update(10.0);
}

if let Some(window_us) = scheduler.co_availability_window_us() {
    println!("co-availability window: {window_us} µs");
} else {
    println!("sync quality insufficient; degrade to local-only mode");
}
```

---

## Building and testing

Run the standard test suite:

```bash
cargo test --lib --tests
cargo test --features std
```

Run formatting, clippy, and documentation checks:

```bash
cargo fmt --all --check
cargo clippy --all-targets --features std -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features std
```

Build for Cortex-M targets:

```bash
rustup target add thumbv7em-none-eabihf
rustup target add thumbv8m.main-none-eabihf

cargo build --target thumbv7em-none-eabihf --no-default-features
cargo build --target thumbv8m.main-none-eabihf --no-default-features
```

---

## `no_std` compatibility

The crate is `#![no_std]` by default.

Floating-point helper functions that are not available in `core` are routed
through `libm`, so the crate can build for embedded targets without depending
on `std`.

The `std` feature is reserved for host-side formatting and development support.

---

## What this repository does not claim

This repository does **not** claim:

- certified medical-device readiness;
- regulatory compliance;
- L3 oscilloscope-validated distributed timing;
- clinical deployment suitability;
- consensus or Byzantine-fault-tolerant state-machine replication;
- general-purpose AI-agent orchestration.

The intended claim is narrower:

> `axonos-swarm` is an `no_std` Rust crate for studying real-time
> coordination, synchronised epoch release, and peer health monitoring across
> AxonOS mesh nodes.

---

## Relationship to RFC-0004

`axonos-swarm` builds on the single-node dual-core timing model described in
[`RFC-0004`](https://github.com/AxonOS-org/axonos-rfcs/blob/main/rfcs/0004-dual-core-real-time-contract.md).

RFC-0004 defines the local real-time contract. This repository asks the next
question:

> If each node has a bounded local pipeline, what additional timing and fault
> constraints are required for a group of nodes to act as one coordinated BCI
> mesh?

The answer is expressed as the SC0–SC6 swarm real-time contract.

---

## Repository status

This repository is public and pre-certification.

Recommended interpretation:

| Audience | How to read this repository |
|---|---|
| Embedded engineer | Inspect `no_std` design, fixed-size state, and timing assumptions |
| Real-time systems reviewer | Check SC0–SC6, epoch scheduling, and co-availability math |
| BCI researcher | Treat the mesh layer as a coordination substrate, not a classifier |
| Investor / technical due diligence | Use this as evidence of AxonOS architecture depth, not as a finished product |

---

## Roadmap

Near-term engineering work:

- add deterministic fixed-point variants for embedded targets without FPU;
- separate Miri UB checks from numeric convergence tests;
- add raw trace fixtures for repeatable PTP convergence testing;
- connect swarm health reports to the AxonOS consent layer;
- publish a formal RFC for SC0–SC6;
- perform L3 GPIO/oscilloscope validation on a hardware fixture.

---

## References

1. IEEE 1588–2019 — *Precision Clock Synchronization Protocol for Networked Measurement and Control Systems*.
2. Welch, G. and Bishop, G. — *An Introduction to the Kalman Filter*.
3. Kopetz, H. — *Real-Time Systems: Design Principles for Distributed Embedded Applications*.
4. Buttazzo, G. — *Hard Real-Time Computing Systems*.
5. AxonOS RFC-0004 — *Dual-Core Real-Time Contract*.
6. AxonOS Article #36 — *Swarm Real-Time: Distributed Hard Deadlines Across the Axon Protocol Mesh*.

---

## License

Dual-licensed under either:

- Apache License, Version 2.0 — see [`LICENSE-APACHE`](./LICENSE-APACHE)
- MIT License — see [`LICENSE-MIT`](./LICENSE-MIT)

at your option. See [`LICENSE`](./LICENSE) for the full dispatcher and trademark notice.

---

<div align="center">

**The AxonOS Project** &nbsp;·&nbsp; [axonos.org](https://axonos.org) &nbsp;·&nbsp; [connect@axonos.org](mailto:connect@axonos.org) &nbsp;·&nbsp; [security@axonos.org](mailto:security@axonos.org)

<sub>Singapore · Zurich · Berlin · Milano · San Mateo</sub>

<sub>© 2026 Denis Yermakou · `axonos-swarm` v0.2.1</sub>

</div>
