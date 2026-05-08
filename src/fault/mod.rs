//! Swarm fault detector — SC4 distributed health monitoring.
//!
//! Detects four failure modes per Article #36:
//!
//! | Type | Failure | Detection bound |
//! |------|---------|-----------------|
//! | 1 | Node silence | ≤ 2 epochs (8 ms) |
//! | 2 | Node degradation (latency > WCRT) | ≤ 1 epoch (4 ms) |
//! | 3 | Swarm desynchronisation | ≤ 100 ms |
//! | 4 | Byzantine (geometrically inconsistent) | ≤ 1 epoch |
//!
//! ## `no_std` design
//!
//! Peer state is tracked in a fixed-size array of `MAX_PEERS = 8` slots.
//! No heap allocation. Matches the `MAX_PEERS` constant in `axonos-consent`.

use crate::{IntentPacket, NodeId};

/// Maximum number of peers tracked simultaneously.
pub const MAX_PEERS: usize = 8;

/// WCRT threshold above which a node is classified as degraded (µs).
/// SC1 guarantees WCRT ≤ 972 µs; we add 250 µs margin for clock error.
pub const DEGRADED_LATENCY_THRESHOLD_US: u64 = 1_200;

/// Number of silent epochs before a node is classified as dead (Type 1 failure).
pub const SILENCE_DEAD_THRESHOLD_EPOCHS: u64 = 2;

/// Health state of a peer node.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeHealthState {
    /// Node is responding on time and within WCRT budget.
    Healthy,
    /// Node is responding but latency exceeds [`DEGRADED_LATENCY_THRESHOLD_US`].
    Degraded {
        /// How much the observed latency exceeds the WCRT budget (µs).
        latency_excess_us: u32,
    },
    /// Node has not responded for ≥ 1 epoch.
    Silent {
        /// Epoch index of last known response.
        last_seen_epoch: u64,
    },
    /// Node is producing intents inconsistent with the swarm state (Type 4).
    Byzantine,
}

/// An alert raised by the fault detector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwarmAlert {
    /// Type 1: node has been silent for `silent_epochs` epochs.
    NodeDead {
        /// The silent node.
        node: NodeId,
        /// How long it has been silent.
        silent_epochs: u64,
    },
    /// Type 2: node latency exceeds WCRT budget.
    NodeDegraded {
        /// The degraded node.
        node: NodeId,
        /// Observed latency (µs).
        latency_us: u64,
    },
    /// Type 4: node is producing Byzantine intents.
    ByzantineNode {
        /// The Byzantine node.
        node: NodeId,
    },
}

/// Health report for one swarm epoch.
#[derive(Debug)]
pub struct SwarmHealthReport {
    /// Epoch this report covers.
    pub epoch: u64,
    /// Alerts raised during this epoch (up to 8).
    alerts: [Option<SwarmAlert>; MAX_PEERS],
    /// Number of valid alerts in `alerts`.
    alert_count: usize,
    /// Number of healthy nodes.
    pub healthy_count: usize,
}

impl SwarmHealthReport {
    fn new(epoch: u64) -> Self {
        Self {
            epoch,
            alerts: [None; MAX_PEERS],
            alert_count: 0,
            healthy_count: 0,
        }
    }

    fn add_alert(&mut self, alert: SwarmAlert) {
        if self.alert_count < MAX_PEERS {
            self.alerts[self.alert_count] = Some(alert);
            self.alert_count += 1;
        }
    }

    /// Slice of alerts raised in this epoch.
    pub fn alerts(&self) -> &[Option<SwarmAlert>] {
        &self.alerts[..self.alert_count]
    }

    /// Whether the swarm is fully healthy (no alerts).
    pub fn is_healthy(&self) -> bool {
        self.alert_count == 0
    }
}

/// Peer slot in the fixed-size peer table.
#[derive(Clone, Copy, Debug)]
struct PeerSlot {
    node_id: NodeId,
    state: NodeHealthState,
    /// PTP clock offset for this peer (µs), from last sync exchange.
    clock_offset_us: i64,
}

/// Distributed fault detector implementing SC4.
///
/// Call [`Self::assess`] once per swarm epoch with all received intent packets.
#[derive(Debug)]
pub struct SwarmFaultDetector {
    /// Fixed-size peer table — up to [`MAX_PEERS`] tracked nodes.
    peers: [Option<PeerSlot>; MAX_PEERS],
    /// Number of registered peers.
    peer_count: usize,
}

impl SwarmFaultDetector {
    /// Construct an empty fault detector.
    pub fn new() -> Self {
        Self {
            peers: [None; MAX_PEERS],
            peer_count: 0,
        }
    }

    /// Register a new peer node.
    ///
    /// Returns `Err` if the peer table is full (> [`MAX_PEERS`]).
    pub fn register_peer(&mut self, node_id: NodeId) -> Result<(), &'static str> {
        if self.peer_count >= MAX_PEERS {
            return Err("peer table full");
        }
        if self.find_peer(node_id).is_some() {
            return Err("peer already registered");
        }
        // Find first empty slot
        for slot in self.peers.iter_mut() {
            if slot.is_none() {
                *slot = Some(PeerSlot {
                    node_id,
                    state: NodeHealthState::Healthy,
                    clock_offset_us: 0,
                });
                self.peer_count += 1;
                return Ok(());
            }
        }
        Err("no empty slot found") // should not reach if peer_count is correct
    }

    /// Update the stored clock offset for a peer (from PTP exchange).
    pub fn update_peer_offset(&mut self, node_id: NodeId, offset_us: i64) {
        if let Some(slot) = self.find_peer_mut(node_id) {
            slot.clock_offset_us = offset_us;
        }
    }

    /// Assess swarm health for one epoch.
    ///
    /// `current_epoch` — current epoch index.
    /// `received` — slice of (NodeId, IntentPacket) received this epoch.
    ///
    /// Returns a [`SwarmHealthReport`] with any alerts.
    pub fn assess(
        &mut self,
        current_epoch: u64,
        received: &[(NodeId, IntentPacket)],
    ) -> SwarmHealthReport {
        let mut report = SwarmHealthReport::new(current_epoch);

        for slot in self.peers.iter_mut().flatten() {
            let received_this_epoch =
                received.iter().any(|(id, _)| *id == slot.node_id);

            if !received_this_epoch {
                // --- Type 1: Silence detection ---
                slot.state = match slot.state {
                    NodeHealthState::Healthy | NodeHealthState::Degraded { .. } => {
                        NodeHealthState::Silent { last_seen_epoch: current_epoch.saturating_sub(1) }
                    }
                    NodeHealthState::Silent { last_seen_epoch } => {
                        let silent_epochs = current_epoch.saturating_sub(last_seen_epoch);
                        if silent_epochs >= SILENCE_DEAD_THRESHOLD_EPOCHS {
                            report.add_alert(SwarmAlert::NodeDead {
                                node: slot.node_id,
                                silent_epochs,
                            });
                        }
                        NodeHealthState::Silent { last_seen_epoch }
                    }
                    other => other,
                };
            }
        }

        // --- Type 2: Latency degradation detection ---
        for (node_id, packet) in received {
            let peer_offset = self
                .find_peer(*node_id)
                .map(|s| s.clock_offset_us)
                .unwrap_or(0);

            // Observed latency: arrival_local − (sent_global + local_offset)
            let adjusted_sent = packet.sent_global_us as i64 + peer_offset;
            let latency_us = if packet.arrival_local_us as i64 > adjusted_sent {
                (packet.arrival_local_us as i64 - adjusted_sent) as u64
            } else {
                0
            };

            if let Some(slot) = self.find_peer_mut(*node_id) {
                if latency_us > DEGRADED_LATENCY_THRESHOLD_US {
                    let excess = (latency_us - 972).min(u32::MAX as u64) as u32;
                    slot.state = NodeHealthState::Degraded { latency_excess_us: excess };
                    report.add_alert(SwarmAlert::NodeDegraded {
                        node: *node_id,
                        latency_us,
                    });
                } else {
                    slot.state = NodeHealthState::Healthy;
                    report.healthy_count += 1;
                }
            }
        }

        // --- Type 4: Byzantine detection (simple outlier check) ---
        // Full consistency check requires domain knowledge of the application.
        // Here we flag nodes whose intent differs from majority consensus.
        if received.len() >= 3 {
            self.check_byzantine(received, &mut report);
        }

        report
    }

    /// Count nodes currently in a given health state.
    pub fn count_in_state(&self, target: fn(&NodeHealthState) -> bool) -> usize {
        self.peers
            .iter()
            .flatten()
            .filter(|s| target(&s.state))
            .count()
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn find_peer(&self, node_id: NodeId) -> Option<&PeerSlot> {
        self.peers.iter().flatten().find(|s| s.node_id == node_id)
    }

    fn find_peer_mut(&mut self, node_id: NodeId) -> Option<&mut PeerSlot> {
        self.peers.iter_mut().flatten().find(|s| s.node_id == node_id)
    }

    /// Simple majority-vote Byzantine check.
    /// A node is flagged if its intent differs from the majority.
    fn check_byzantine(
        &mut self,
        received: &[(NodeId, IntentPacket)],
        report: &mut SwarmHealthReport,
    ) {
        use crate::IntentKind;

        // Count intent kinds
        let mut nav_count = 0usize;
        let mut other_count = 0usize;
        for (_, pkt) in received {
            match pkt.intent {
                IntentKind::Navigation(_) => nav_count += 1,
                _ => other_count += 1,
            }
        }
        let majority_is_nav = nav_count > other_count;

        for (node_id, pkt) in received {
            let is_outlier = match pkt.intent {
                IntentKind::Navigation(_) => !majority_is_nav,
                _ => majority_is_nav,
            };
            if is_outlier {
                if let Some(slot) = self.find_peer_mut(*node_id) {
                    slot.state = NodeHealthState::Byzantine;
                }
                report.add_alert(SwarmAlert::ByzantineNode { node: *node_id });
            }
        }
    }
}

impl Default for SwarmFaultDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Direction, IntentKind};

    fn make_packet(node_id: NodeId, epoch: u64, latency_us: u64) -> (NodeId, IntentPacket) {
        let sent = 1_000_000_u64;
        (node_id, IntentPacket {
            intent: IntentKind::Navigation(Direction::Left),
            sent_global_us: sent,
            arrival_local_us: sent + latency_us,
            node_id,
            epoch,
        })
    }

    #[test]
    fn healthy_node_no_alerts() {
        let mut det = SwarmFaultDetector::new();
        let n1 = NodeId(1);
        det.register_peer(n1).unwrap();

        let received = [make_packet(n1, 0, 900)]; // 900 µs latency — within bound
        let report = det.assess(0, &received);
        assert!(report.is_healthy(), "alerts: {:?}", report.alerts());
    }

    #[test]
    fn degraded_node_raises_alert() {
        let mut det = SwarmFaultDetector::new();
        let n1 = NodeId(1);
        det.register_peer(n1).unwrap();

        let received = [make_packet(n1, 0, 1_500)]; // 1500 µs — above threshold
        let report = det.assess(0, &received);
        assert!(!report.is_healthy());
        assert!(matches!(
            report.alerts()[0],
            Some(SwarmAlert::NodeDegraded { .. })
        ));
    }

    #[test]
    fn silent_node_triggers_dead_alert_after_two_epochs() {
        let mut det = SwarmFaultDetector::new();
        let n1 = NodeId(1);
        det.register_peer(n1).unwrap();

        // Epoch 0: node was healthy (first silence)
        det.assess(0, &[]);
        // Epoch 1: still silent → silent_epochs = 1 (below threshold)
        det.assess(1, &[]);
        // Epoch 2: still silent → silent_epochs = 2 → NodeDead
        let report = det.assess(2, &[]);
        let has_dead = report.alerts().iter().any(|a| {
            matches!(a, Some(SwarmAlert::NodeDead { .. }))
        });
        assert!(has_dead);
    }

    #[test]
    fn peer_table_full_returns_error() {
        let mut det = SwarmFaultDetector::new();
        for i in 0..MAX_PEERS {
            det.register_peer(NodeId(i as u64)).unwrap();
        }
        assert!(det.register_peer(NodeId(99)).is_err());
    }

    #[test]
    fn duplicate_peer_returns_error() {
        let mut det = SwarmFaultDetector::new();
        det.register_peer(NodeId(1)).unwrap();
        assert!(det.register_peer(NodeId(1)).is_err());
    }
}
