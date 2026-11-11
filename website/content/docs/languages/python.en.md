---
title: Python
description: PyO3 binding for Python consumers.
weight: 2
---

The `vectora-engine` Python package is a PyO3 facade over Rust. It is intended for Python hosts such as the Vectora backend. The package is not published to PyPI yet; build it from the checkout.

```bash
scons install
cd bindings/python
uv run --project ../.. maturin develop
```

Python exposes ergonomic snake_case wrappers such as `chat_complete`, `chat_stream`, `native_chat_complete`, `assemble_turn`, `start_model_tool_loop`, `prepare_tool_dispatch`, `evaluate_verification`, and the SQLite store functions. Async provider and stream operations are Python coroutines/async iterators; persistence and pure contract operations are synchronous where defined.

```python
import vectora_engine

print(vectora_engine.chat_contract_version())
```

Consult `bindings/python/README.md` and the versioned specs for exact settings, request schemas, exceptions, and lifecycle requirements. The API is experimental; do not assume every Rust module is exposed to Python.
