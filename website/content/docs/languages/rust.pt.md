---
title: Rust
description: Implementação canônica e integração direta.
weight: 1
---

O crate `vectora-engine` é a implementação canônica. Consumidores Rust podem usar os módulos diretamente pelo workspace. Ainda não foi anunciada publicação no crates.io.

O código atual agrupa APIs em `chat`, `llm`, `harness` e `storage`. Prefira contratos tipados e configurações explícitas de adapters. Chamadas a providers exigem credenciais e configuração gerenciadas pelo consumidor. Seleção de stores, identidade, policy, execução dos adapters de tools e retenção de dados também continuam sob responsabilidade do consumidor.

Use `cargo doc --workspace --all-features --no-deps --locked` para gerar a documentação local da API ou consulte o crate e as specs de capabilities no repositório.
