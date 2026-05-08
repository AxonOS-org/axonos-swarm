//! # axonos-swarm
//!
//! Distributed real-time coordination for AxonOS mesh nodes.
//!
//! This crate implements the **Swarm Real-Time Contract** defined in
//! [AxonOS Article #36](https://medium.com/@AxonOS/axonos-article-36-swarm-real-time-distributed-hard-deadlines-across-the-axon-protocol-mesh-4e30d730c2ad)
//! and extends the single-node timing guarantees of
//! [RFC-0004](https://github.com/AxonOS-org/axonos-rfcs/blob/main/rfcs/0004-dual-core-real-time-contract.md)
//! to N nodes on a wireless mesh.
//!
//! ## Three components
//!
//! - [`neural_ptp`] — Kalman-filtered IEEE 1588 PTP for ±18 µs clock sync on BLE mesh
//! - [`swarm`] — synchronised epoch scheduling and co-availability window computation
//! - [`fault`] — distributed fault detector for silence, degradation, desync, Byzantine
//!
//! ## Swarm Real-Time Contract (SC0–SC6)
//!
//! | Clause | Guarantee | Bound |
//! |--------|-----------|-------|
//! | SC0 | Clock synchronisation | ≤ 50 µs 3σ between any two nodes |
//! | SC1 | Local pipeline (inherited from RFC-0004) | WCRT ≤ 972 µs per node |
//! | SC2 | Synchronised epoch start | Skew ≤ 2 × σ_sync = 100 µs |
//! | SC3 | Intent co-availability | All nodes ready within ≤ 500 µs |
//! | SC4 | Fault detection | Silence / degradation ≤ 8 ms; desync ≤ 100 ms |
//! | SC5 | Graceful degradation | N−1 nodes continue under SC1 when one fails |
//! | SC6 | Probabilistic cross-node delivery | P(T_mesh ≤ 14.5 ms) ≥ 0.999 |
//!
//! Evidence level: L1 (analytically derived from Neural PTP model).
//! L3 oscilloscope validation pending Phase 1 hardware fixture (Q2 2026).
//!
//! ## `no_std` compatibility
//!
//! The entire crate is `#![no_std]`. No heap allocation on the hot path.
//! All state is fixed-size. Enable the `std` feature for `Display` trait
//! implementations (formatting only, no runtime allocation in core logic).
//!
//! ## Example
//!
//! ```rust
//! use axonos_swarm::neural_ptp::NeuralPtpKalman;
//!
//! let mut ptp = NeuralPtpKalman::new();
//!
//! // Simulate 10 PTP exchanges at 10 Hz
//! for i in 0..10 {
//!     let dt_s = 0.1_f64;
//!     ptp.predict(dt_s);
//!     // Simulated measurement: 180 µs raw offset (moderate interference)
//!     ptp.update(180.0);
//! }
//!
//! let offset = ptp.offset_us();
//! let uncertainty = ptp.offset_uncertainty_3sigma_us();
//! // After 10 measurements, uncertainty should be well below 200 µs
//! assert!(uncertainty < 200.0);
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]
#![allow(clippy::doc_markdown)] // RFC/BCI/PTP/LSL are technical acronyms

pub mod fault;
pub mod neural_ptp;
pub mod swarm;

/// Node identifier — 64-bit UUID.
///
/// Assigned at manufacture or first boot. Used for PTP master election
/// (lower UUID wins tie-breaks) and fault detector peer tracking.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(transparent)]
pub struct NodeId(pub u64);

/// Intent kind produced by the local pipeline (abbreviated — full set in axonos-sdk).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum IntentKind {
    /// Directional navigation: left, right, up, down, idle.
    Navigation(Direction),
    /// Cognitive workload advisory.
    Workload(WorkloadLevel),
}

/// Navigation direction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    /// Left movement intent.
    Left,
    /// Right movement intent.
    Right,
    /// Upward movement intent.
    Up,
    /// Downward movement intent.
    Down,
    /// No movement intent — resting state.
    Idle,
}

/// Cognitive workload level.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorkloadLevel {
    /// Low workload.
    Low,
    /// Medium workload.
    Medium,
    /// High workload.
    High,
}

/// A timestamped intent packet transmitted on the mesh.
#[derive(Clone, Copy, Debug)]
pub struct IntentPacket {
    /// Intent kind produced by the local pipeline.
    pub intent: IntentKind,
    /// Global timestamp (µs) when the intent was produced, in PTP master clock.
    pub sent_global_us: u64,
    /// Local timestamp (µs) when the packet arrived at this node.
    pub arrival_local_us: u64,
    /// Producing node.
    pub node_id: NodeId,
    /// Swarm epoch index in which this intent was produced.
    pub epoch: u64,
}
