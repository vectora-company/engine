---
title: Capability maturity
description: Interpret implementation status without assuming stability or parity.
weight: 2
---

The repository distinguishes four states:

| State | Meaning |
| --- | --- |
| Implemented | Code and tests exist for the stated scope. This does not automatically mean stable API or broad production readiness. |
| Experimental | A versioned contract exists, but compatibility, operational coverage, or consumer adoption can still change. |
| Partial | Some adapters, bindings, or conformance cases exist; the complete capability is not established. |
| Planned / unavailable | A design may exist, but consumers must not rely on an implementation. |

The status for each area is in the [capability matrix](../../reference/capabilities/). Public API and publication status can differ: a binding may compile from source while its package is not published to npm or PyPI.

No performance, parity, security-boundary, or exactly-once claim should be inferred from the presence of code or a passing unit test. Such claims need explicitly scoped conformance and reproducible operational evidence.
