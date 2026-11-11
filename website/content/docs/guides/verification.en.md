---
title: Verification gates
description: Evaluate host reports without confusing a decision with check execution.
weight: 6
---

The gate receives a versioned `VerificationReport` describing checks executed by the host. `evaluate_verification` combines status, requiredness, infrastructure failure, and evidence policy to produce a deterministic verdict.

Statuses distinguish passed, failed, infrastructure error, and not run. Evidence uses opaque references and an explicit disposition policy. The evaluation can be linked to the durable SQLite lifecycle or loop, but persistence does not authenticate the report's origin.

The host runs commands/tests, verifies artifacts, and associates evidence with checks. The engine does not run CI, open evidence links, verify artifact integrity, or attest that a report is true. Do not let model text masquerade as trusted evidence: the host must create reports from observed results and enforce gate rules before completing the run.

See the [`15-verification-gates.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/15-verification-gates.md) contract and the [maturity](../../concepts/maturity/) reference.
