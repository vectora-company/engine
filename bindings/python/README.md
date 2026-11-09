# Binding Python

O pacote `vectora-engine` no PyPI é uma fachada PyO3 para o núcleo Rust. A API
Python serializa o contrato JSON versionado e delega configuração, transporte,
streaming, parsing e erros ao Rust. Não implementa política de provider.

## Desenvolvimento

Na raiz do repositório, instale o tooling com `scons install`. Para compilar e
instalar a extensão no ambiente Python ativo:

```bash
cd bindings/python
uv run --project ../.. maturin develop
```

Use `chat_complete(settings, request)` como coroutine ou consuma
`chat_stream(settings, request)` com `async for`. O fechamento do iterador
cancela o stream nativo e sua requisição HTTP. `settings` e `request` seguem os
schemas de `docs/specs/03-chat-openai-compatible.md`; erros são exceções com
mensagens redigidas do provider.

O Rust da extensão fica em `native/`, e a API Python de ergonomia mínima em
`python/vectora_engine/`. O build de wheels multi-plataforma e publicação no
PyPI serão habilitados em um workflow de release separado.
