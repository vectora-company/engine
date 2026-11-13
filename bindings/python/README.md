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
`python/vectora_engine/`. O build de wheels multi-plataforma e a publicação no
PyPI ainda não estão configurados; ambos exigem um workflow de release separado.

Mídia usa `MediaExecutionContext(assets={asset_id: bytes})`. O mapa é
temporário; no Vectora, construa-o somente com bytes obtidos por
`AssetStore.read` após validar owner/workspace/thread. `prepare_chat_content`
é o modo manual; `chat_complete(..., media_context=ctx)` é por chamada;
`settings["media_context"] = ctx` configura o padrão. O limite padrão é 20 MiB.

Anthropic Messages, Google Gemini e Cohere Chat v2 usam `native_chat_complete`
e `native_chat_stream` com o mesmo contrato Rust. `settings.provider` deve ser
`anthropic`, `google` ou `cohere`; credenciais e configuração específica pertencem ao
consumer. Campos como Gemini `safetySettings` podem ser passados em
`request["extensions"]`, sem defaults de política de produto.

## Famílias de API

Além do chat, o binding expõe o mesmo núcleo Rust para: fallback de chat,
montagem de turno, embeddings e rerank com fallback; run lifecycle, dispatch e
policy de tools, loop modelo/tool e gates de verificação; stores SQLite de
lifecycle, loop e policy; ledger SQLite com backup e restore e projeção
PostgreSQL; seleção de storage e `capabilities`; vetores (LanceDB e Qdrant), KV,
filas com DLQ e pub/sub (Redis e NATS). Todas são experimentais; o estado de
cada uma está em `docs/plans/03-plano-continuidade.md`.
