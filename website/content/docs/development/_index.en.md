---
title: Development
description: Repository workflow and contribution guidance.
weight: 6
---

Start with the repository [`CONTRIBUTING.md`](https://github.com/vectora-ltda/engine/blob/main/CONTRIBUTING.md) and [`AGENTS.md`](https://github.com/vectora-ltda/engine/blob/main/AGENTS.md). The canonical Rust implementation and the Python/TypeScript bindings must stay aligned through contract tests. Prefer incremental capability changes with explicit fallback and rollback boundaries.

Useful local commands: `scons install`, `scons build`, `scons lint`, and `scons tests`. The website itself can be previewed with `scons docs-serve` and built with `scons docs-build`.
