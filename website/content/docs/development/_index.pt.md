---
title: Desenvolvimento
description: Fluxo do repositório e orientação para contribuição.
weight: 6
---

Comece pelo [`CONTRIBUTING.md`](https://github.com/vectora-ltda/engine/blob/main/CONTRIBUTING.md) e pelo [`AGENTS.md`](https://github.com/vectora-ltda/engine/blob/main/AGENTS.md). A implementação canônica Rust e os bindings Python/TypeScript devem permanecer alinhados por testes de contrato. Prefira mudanças incrementais de capability com limites explícitos de fallback e rollback.

Comandos locais úteis: `scons install`, `scons build`, `scons lint` e `scons tests`. O site pode ser visualizado com `scons docs-serve` e compilado com `scons docs-build`.
