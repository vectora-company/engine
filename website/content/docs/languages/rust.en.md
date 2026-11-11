---
title: Rust
description: The canonical implementation and direct integration surface.
weight: 1
---

The `vectora-engine` crate is the canonical implementation. Rust consumers can use the modules directly from the workspace. A public crates.io release has not been announced.

The source currently groups APIs under `chat`, `llm`, `harness`, and `storage`. Prefer typed contracts and explicit adapter settings. Provider calls require consumer-managed credentials and configuration. Store selection, identity, policy, tool adapter execution, and data retention also remain consumer responsibilities.

Use `cargo doc --workspace --all-features --no-deps --locked` to generate local API docs, or browse the crate source and the capability specs in the repository.
