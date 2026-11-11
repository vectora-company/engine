---
title: Python
description: Binding PyO3 para consumidores Python.
weight: 2
---

O pacote Python `vectora-engine` é uma fachada PyO3 para o Rust. Ele é destinado a hosts Python, como o backend do Vectora. Ainda não está publicado no PyPI; compile a partir do checkout.

```bash
scons install
cd bindings/python
uv run --project ../.. maturin develop
```

O Python expõe wrappers em snake_case, como `chat_complete`, `chat_stream`, `native_chat_complete`, `assemble_turn`, `start_model_tool_loop`, `prepare_tool_dispatch`, `evaluate_verification` e funções dos stores SQLite. Operações assíncronas de provider e stream são coroutines/iteradores assíncronos; persistência e contratos puros são síncronos quando definidos assim.

```python
import vectora_engine

print(vectora_engine.chat_contract_version())
```

Consulte `bindings/python/README.md` e as specs versionadas para schemas exatos, exceções e requisitos de ciclo de vida. A API é experimental; não presuma que todos os módulos Rust estejam expostos em Python.
