---
title: Build e verificações
description: Compile o núcleo Rust e os bindings com SCons.
weight: 2
---

Execute os comandos na raiz do repositório:

```bash
scons                  # lista os alvos
scons install          # dependências e sidecars; não compila
scons build            # compila o núcleo Rust e os bindings existentes
scons build-core       # compila apenas o crate principal
scons build-python     # compila o pacote PyO3
scons build-typescript # compila o pacote N-API
scons lint
scons tests
```

`scons build` compartilha o diretório Cargo `target/` entre o núcleo e os bindings nativos, permitindo reutilizar os artefatos incrementais do Cargo em builds seguintes. Um build limpo ou uma mudança de toolchain/features pode exigir uma compilação longa.

O alvo de testes executa Rust, Python e os testes dos bindings TypeScript. Alguns testes de integração de storage dependem dos serviços PostgreSQL, Qdrant ou Redis e variam conforme o ambiente.

Para trabalhar no site localmente, use `scons docs-serve`; valide um build estático com `scons docs-build`. Nenhum desses comandos publica o site.
