# Notable changes — axonos-swarm

All notable changes are documented in this file.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [v0.2.1] — 2026-05-27

Polish release. No source-code or API changes — v0.2.0 source is preserved
byte-identical. This release brings the repository up to the unified AxonOS
visual and documentation standard applied across the organisation.

### Added

- **`CITATION.cff`** — machine-readable citation metadata; GitHub renders the
  "Cite this repository" button from it.
- **`SECURITY.md`** — vulnerability-disclosure policy mirroring the policy used
  in `axonos-standard` and the AxonOS Project entry-point repository.

### Changed

- **README badge palette** — aligned to the canonical AxonOS palette: AxonOS
  blue `#0a4a8f` for versioned artefacts (Crate v0.2.1, Standard v1.0.0), Rust
  canonical orange `#CE422B`, slate `#475569` for licence and status. Replaces
  the previous generic `orange`/`blue` placeholder colours.
- **README stack table** — adds the missing `axonos-standard` row at the top
  (canonical normative spec) and separates `axonos-rfcs` onto its own row
  (engineering proposals). The full AxonOS stack is now visible from this
  repository's landing page.
- **README footer** — replaced ad-hoc copyright line with the canonical
  centered AxonOS footer block (project name, contacts, five-city locator).
- **Author email** — `denis@axonos.org` (personal) → `connect@axonos.org`
  (project-canonical) in `Cargo.toml` and `LICENSE`. The project's general
  contact is `connect@axonos.org` going forward; `security@axonos.org` remains
  the security disclosure address.

### Notes

- **Source-code unchanged.** All v0.2.0 APIs (`NeuralPtpKalman`, `SwarmScheduler`,
  `SwarmFaultDetector`) work as before. Wire format and SC0–SC6 contract bounds
  byte-identical.
- **Patch bump 0.2.0 → 0.2.1** per SemVer — purely additive, no breaking changes,
  no new dependency.

---

## [v0.2.0] — 2026-05-19

Infrastructure release. No source-code or API changes — the v0.1.0 implementation
(Neural PTP, Swarm Scheduler, Fault Detector) is preserved byte-identical.
This release adds the CI/CD, licensing, and release infrastructure that brings
this crate up to parity with `axonos-kernel` and `axonos-sdk`.

### Added — Continuous Integration

`.github/workflows/ci.yml` with **8 jobs**:

1. **test (× 3 OS)** — unit + doc tests on `ubuntu-latest`, `macos-latest`,
   `windows-latest`, both default features and `--features std`.
2. **fmt** — `cargo fmt --check` enforcing `rustfmt.toml` conventions.
3. **clippy** — `cargo clippy -D warnings` on default and `--features std`.
4. **MSRV 1.85** — verifies the minimum supported Rust version still builds.
5. **no_std × 2** — builds for `thumbv7em-none-eabihf` (Cortex-M4F, STM32F407)
   and `thumbv8m.main-none-eabihf` (Cortex-M33, STM32H573, future).
6. **docs** — `cargo doc --no-deps -D warnings` catches broken intra-doc links.
7. **miri** — undefined-behaviour detection with `MIRIFLAGS=-Zmiri-ignore-leaks`
   (continue-on-error).
8. **cargo-deny** — `EmbarkStudios/cargo-deny-action@v2` for licence + advisory
   + bans audit (continue-on-error).

Concurrency control: `cancel-in-progress: true` per branch to avoid stacking
runs on rapid pushes. Global `RUSTFLAGS: -D warnings` and `CARGO_TERM_COLOR: always`.

### Added — Auto-release on tag push

`.github/workflows/release.yml` triggers on every `v*.*.*` tag push and
creates a proper GitHub Release with:

- Title: the tag name (e.g. `v0.2.0`)
- Body: matching CHANGELOG section extracted via awk between `## [vX.Y.Z]`
  headers
- **Green "Latest" banner** on the repo page for stable releases
- Pre-release marker for `-rc`/`-beta`/`-alpha` tag suffixes
- Source `.tar.gz` and `.zip` archives attached via `git archive`
- Install snippet for `Cargo.toml` auto-generated in the release body

This replaces the previous plain "N tags" text with a prominent release
banner that visitors see at the top of the repo's right sidebar.

### Added — Dual-licence file structure

The crate's `Cargo.toml` declared `license = "Apache-2.0 OR MIT"` from
v0.1.0, but the repository contained only a single `LICENSE` (MIT) file.
GitHub's licence detector therefore showed "MIT license" instead of the
correct "Apache-2.0 OR MIT".

Fixed by adding the standard Rust-ecosystem dual-licence layout:

- **`LICENSE`** — dispatcher file (no extension) explaining the dual-licence
  model. GitHub's detector recognises this filename and reads the SPDX
  expression from `Cargo.toml`.
- **`LICENSE-APACHE`** — full Apache 2.0 text with Denis Yermakou attribution.
- **`LICENSE-MIT`** — full MIT text with Denis Yermakou attribution.

GitHub will now correctly display "Apache-2.0 OR MIT" on the repository
landing page.

### Added — Project infrastructure files

- **`CHANGELOG.md`** (this file) — version history per Keep a Changelog format.
- **`rustfmt.toml`** — locks formatting conventions for `cargo fmt` to match
  the kernel and SDK workspaces (`max_width = 100`, `tab_spaces = 4`,
  `newline_style = "Unix"`).
- **`deny.toml`** — minimal `cargo-deny` configuration permitting common
  permissive licences (MIT, Apache-2.0, BSD-2/3, ISC, Unicode) and denying
  copyleft (GPL, LGPL, AGPL).

### Notes

- **Source-code unchanged.** All v0.1.0 APIs (`NeuralPtpKalman`, `SwarmScheduler`,
  `SwarmFaultDetector`) work as before. Wire format and SC0–SC6 contract bounds
  are byte-identical.
- **First proper release.** v0.1.0 was pushed without a tag; v0.2.0 is the
  first release that will produce a GitHub Release with the green "Latest"
  banner via the new `release.yml` workflow.
- **Bumped from 0.1.0 → 0.2.0** to signal a substantive (infrastructure) milestone.
  This is a minor bump (additive only) per SemVer; no breaking changes.

---

## [v0.1.0] — 2026-05

Initial public release. Three modules implementing the **Swarm Real-Time
Contract (SC0–SC6)** documented in [RFC-0008](https://github.com/AxonOS-org/axonos-rfcs)
and [Article #36](https://medium.com/@AxonOS):

- **`neural_ptp::NeuralPtpKalman`** — Kalman-filtered IEEE 1588 PTP,
  converges to ±18 µs (3σ) on BLE mesh (8.5× better than raw PTP in
  high-interference environments).
- **`swarm::SwarmScheduler`** — Synchronised epoch start with skew bound
  SC2 ≤ 100 µs across N nodes.
- **`fault::SwarmFaultDetector`** — Silence / degradation / desync / Byzantine
  detection per SC4 (detection latency ≤ 8 ms).

`#![no_std]` throughout. Features: `std` (enables `Display` impls), `defmt`
(structured logging for embedded targets). MSRV: Rust 1.85.

---

[v0.2.1]: https://github.com/AxonOS-org/axonos-swarm/releases/tag/v0.2.1
[v0.2.0]: https://github.com/AxonOS-org/axonos-swarm/releases/tag/v0.2.0
[v0.1.0]: https://github.com/AxonOS-org/axonos-swarm/releases/tag/v0.1.0
