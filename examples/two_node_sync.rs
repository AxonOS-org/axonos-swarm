//! Two-node synchronisation example.
//!
//! Demonstrates the Swarm Real-Time Contract SC2 (synchronised start) and
//! SC3 (co-availability) for a bilateral exoskeleton use case.
//!
//! Run with: cargo run --example two_node_sync --features std

use axonos_swarm::{
    fault::SwarmFaultDetector, swarm::SwarmScheduler, Direction, IntentKind, IntentPacket, NodeId,
};

fn main() {
    println!("=== AxonOS Swarm — Two-Node Sync Example ===\n");

    let node_a = NodeId(0xAAA0_0000_0000_0001);
    let node_b = NodeId(0xBBB0_0000_0000_0002);

    // Session starts at t=0
    let session_start_us = 0_u64;

    let mut sched_a = SwarmScheduler::new(session_start_us);
    let mut sched_b = SwarmScheduler::new(session_start_us);
    let mut detector = SwarmFaultDetector::new();

    detector.register_peer(node_a).expect("register A");
    detector.register_peer(node_b).expect("register B");

    println!("Initial sync quality:");
    println!("  Node A: {:.2}", sched_a.sync_quality());
    println!("  Node B: {:.2}", sched_b.sync_quality());
    println!("  (Expected: low — filters not converged yet)\n");

    // Simulate 60 seconds of Neural PTP exchanges at 10 Hz
    println!("Simulating 600 PTP exchanges (60 s at 10 Hz, moderate interference)...");
    for _ in 0..600 {
        sched_a.ptp_mut().predict(0.1);
        sched_b.ptp_mut().predict(0.1);
        // Simulate moderate interference: true offset 20 µs, measurement noise 180 µs RMS
        sched_a.ptp_mut().update(20.0 + 18.0);
        sched_b.ptp_mut().update(20.0 - 12.0);
    }

    println!("After 60 s of PTP synchronisation:");
    println!("  Node A sync quality: {:.2}", sched_a.sync_quality());
    println!("  Node B sync quality: {:.2}", sched_b.sync_quality());

    let window_a = sched_a.co_availability_window_us();
    let window_b = sched_b.co_availability_window_us();
    println!("  Co-availability window A: {:?} µs", window_a);
    println!("  Co-availability window B: {:?} µs", window_b);

    if let (Some(wa), Some(wb)) = (window_a, window_b) {
        let worst = wa.max(wb);
        println!("\n  SC3 co-availability: {} µs (≤ 500 µs required)", worst);
        if worst <= 500 {
            println!("  ✓ SC3 SATISFIED — bilateral exoskeleton coordination achievable");
        } else {
            println!("  ✗ SC3 NOT SATISFIED — increase PTP convergence time");
        }
    }

    // Simulate one swarm epoch
    println!("\n=== Epoch 0 assessment ===");
    let local_now = 4_001_u64; // just past first epoch

    let next_a = sched_a.next_epoch_start_local_us(local_now);
    let next_b = sched_b.next_epoch_start_local_us(local_now);
    let skew = (next_a as i64 - next_b as i64).unsigned_abs();
    println!("  Node A next epoch start: {} µs", next_a);
    println!("  Node B next epoch start: {} µs", next_b);
    println!("  Start skew: {} µs (≤ 100 µs required for SC2)", skew);

    // Simulate intent packets from both nodes
    let packets = vec![
        (
            node_a,
            IntentPacket {
                intent: IntentKind::Navigation(Direction::Left),
                sent_global_us: 972,
                arrival_local_us: 972 + 900, // 900 µs transport
                node_id: node_a,
                epoch: 0,
            },
        ),
        (
            node_b,
            IntentPacket {
                intent: IntentKind::Navigation(Direction::Right),
                sent_global_us: 972,
                arrival_local_us: 972 + 850, // 850 µs transport
                node_id: node_b,
                epoch: 0,
            },
        ),
    ];

    let report = detector.assess(0, &packets);
    println!("\n  Fault detector epoch 0 report:");
    println!("    Healthy: {}", report.healthy_count);
    println!("    Alerts:  {}", report.alerts().len());
    if report.is_healthy() {
        println!("    ✓ All nodes healthy");
    }

    println!("\n=== Done ===");
}
