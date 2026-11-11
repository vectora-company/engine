---
title: First Rust call
description: Use the canonical Rust crate from this workspace.
weight: 3
---

The Rust crate is the canonical implementation. During development, add `vectora-engine` as a workspace/path dependency or work inside the workspace; a crates.io release is not currently available.

The core exposes typed modules for chat, LLM adapters, harness contracts, and storage. For example, pure orchestration functions such as tool dispatch validation and verification evaluation can be called without contacting a provider. Network-backed chat functions require consumer-supplied settings and credentials.

```rust
use vectora_engine::harness::evaluate_verification;
// Construct a versioned VerificationReport and EvidenceDispositionPolicy,
// then evaluate the report before allowing the host run to proceed.
```

This is an API orientation, not a complete copy-paste example: construct the required versioned report and policy using the public types in the crate. See the [capability reference](../../reference/capabilities/) and the Rust source docs for exact fields. Public contracts remain experimental.
