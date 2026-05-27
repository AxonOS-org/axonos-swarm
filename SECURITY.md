# Security Policy — axonos-swarm

This document is the vulnerability-disclosure policy for the
`axonos-swarm` crate.

## Scope

This policy covers defects in the published `axonos-swarm` source
code that may have a security impact: panic-on-malformed-input paths,
out-of-bounds reads, unsound timing assumptions that an adversary
could exploit to violate the Swarm Real-Time Contract (SC0–SC6), and
similar.

For specification-level concerns about the Swarm Real-Time Contract
itself — concerns that hold for any conformant implementation — open
an issue or pull request against
[`axonos-rfcs`](https://github.com/AxonOS-org/axonos-rfcs) (RFC-0008)
instead.

## How to report

Report a suspected security problem by writing to
**security@axonos.org**. Describe the problem concretely: which file
or which contract clause, what an attacker could do, and where
possible how to reproduce or demonstrate it.

A reporter who prefers not to use email may instead open a private
security advisory through the GitHub security-advisory mechanism on
this repository. A reporter should **not** open an ordinary public
issue for a suspected security problem.

## What to expect

The project acknowledges a security report within five business days.
The default coordinated-disclosure window is ninety days from
acknowledgement to public disclosure, shortened if a fix is ready
sooner and extended only by mutual agreement where remediation is
genuinely complex. The reporter is credited in the public disclosure
unless they ask to remain anonymous.

## Supported versions

Security remediations are issued for the current minor version of the
crate, recorded in [`Cargo.toml`](./Cargo.toml). Older minor versions,
once superseded, do not receive remediations; a deployment on a
superseded version should plan its migration.

## What this policy does not cover

This policy does not cover security problems in third-party
implementations of the Swarm Real-Time Contract that are not this
reference crate. If such an issue is caused by a defect in the
underlying specification, that specification defect is in scope and
should be reported here or against `axonos-rfcs`.

This policy is not a warranty. The crate is provided under the dual
Apache-2.0 / MIT licence (see [`LICENSE`](./LICENSE)) with the customary
disclaimers.

---

The AxonOS Project · https://axonos.org · security@axonos.org
