# Análise — CLI independente da Engine

## Contexto confirmado

A Engine já contém clientes Rust distintos: `LayaClient` implementa o protocolo Jev do servidor standalone `laya-serve`; `LlamaServerClient` implementa chat compatível e `/v1/systemone` do llama.cpp. Portanto o suporte a Laya-GGUF no llama.cpp usa o segundo cliente; não se deve tentar falar Jev com llama-server nem assumir que `laya-serve` expõe chat OpenAI-compatible. O launcher `laya-install.sh` já documenta ambos os runtimes.

O workspace não tinha binário consumidor para exercitar chat e configuração sem bindings/aplicações. A Engine já modela tool calls e mensagens de ferramenta em `ChatRequest`/`ChatCompletion`, então o CLI pode atuar como cliente fino e não precisa duplicar dispatch persistente do harness.

## Decisões de implementação

A CLI deve manter configuração separada do runtime, referenciar secrets via environment, limitar ReAct a uma ferramenta local allowlisted e separar endpoint de chat da seleção de Decision Model. Isso dá um caminho manual reproduzível aos dois servidores de Laya e evita execução local arbitrária em um utilitário de teste.

## Estado / evidência

Implementação atual em `crates/engine-cli`: configuração JSON sem segredo materializado, chat ReAct com calculator allowlisted, spinner/markdown/input interativo e decisão por ambos os clientes. Os cinco testes da crate passaram; `--help` e init/set/validate/show passaram manualmente. O workspace inclui a crate; `scons tests`, `scons lint`, pre-commit específico e `scons build-cli` passaram. O binário otimizado inicia com `--help`. Dependências stable consultadas no índice crates.io: rustyline 19.0.0, termimad 0.35.5 e indicatif 0.18.6. Limite: esses testes demonstram seleção/contrato; não provam inferência com checkpoint Laya real, que requer servidores e pesos configurados.
