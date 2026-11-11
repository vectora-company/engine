---
title: Primeira chamada Rust
description: Use o crate Rust canônico deste workspace.
weight: 3
---

O crate Rust é a implementação canônica. Durante o desenvolvimento, use `vectora-engine` como dependência local/path ou trabalhe dentro do workspace; ainda não há uma versão publicada no crates.io.

O núcleo expõe módulos tipados para chat, adapters de LLM, contratos do harness e storage. Funções puras de coordenação, como validação de dispatch de tools e avaliação de verificação, podem ser chamadas sem acessar providers. Funções de chat com rede exigem configurações e credenciais fornecidas pelo consumidor.

```rust
use vectora_engine::harness::evaluate_verification;
// Monte um VerificationReport e EvidenceDispositionPolicy versionados;
// avalie o report antes de permitir que o run do host prossiga.
```

Este exemplo orienta a API, mas não é um trecho completo pronto para copiar: construa o report e a policy exigidos com os tipos públicos do crate. Consulte a [referência de capacidades](../../reference/capabilities/) e a documentação no código Rust para os campos exatos. Os contratos públicos continuam experimentais.
