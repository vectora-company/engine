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
`settings["media_context"] = ctx` configura o padrão. O limite padrão é 20 MiB por asset. `provider_limits` pode definir limites específicos por provider/modelo e modalidade (`image_bytes`, `audio_bytes`, `video_bytes`, `document_bytes`, `file_bytes`); o menor entre esse valor e `max_media_bytes` vale. Isso é configuração explícita do host, não descoberta automática do provider.

Requests normais rejeitam `Inline` e URLs `data:` para que os bytes não sejam
serializados junto ao histórico. Para envio manual de `prepare_chat_content` ou
outro payload inline, use `EphemeralChatRequest` com
`chat_complete_ephemeral`/`chat_stream_ephemeral`; providers nativos e fallback
oferecem os equivalentes `native_*_ephemeral` e `fallback_*_ephemeral`. O
envelope efêmero valida base64, tipo/MIME pelos bytes e `max_media_bytes` antes
do transporte. Ele serve só para a chamada atual e não deve ser persistido.

Anthropic Messages, Google Gemini, Cohere Chat v2, Ollama nativo e OpenAI
Responses usam `native_chat_complete` e `native_chat_stream` com o mesmo
contrato Rust. `settings.provider` pode ser `anthropic`, `google`, `cohere`,
`ollama` ou `openai_responses`; credenciais e configuração específica pertencem ao
consumer. OpenAI Responses é um protocolo separado do perfil Chat Completions,
usa `store: false` por padrão e aceita override explícito em `request["extensions"]`.
Campos como Gemini `safetySettings` podem ser passados em
`request["extensions"]`, sem defaults de política de produto.

O servidor Laya usa `laya_health(settings)` e `laya_decide(settings, request)`
na API SystemOne (`/v1/systemone`). Decision Model é a categoria; Jev é um
modelo específico. `request["model"]` é opcional: informe um checkpoint Laya,
como `multilingual` ou `typed-decisions`, ou omita-o para permitir roteamento.
Um ID Jev não é checkpoint Laya. Laya não oferece `/v1/models` e a rota HTTP
não recebe mídia. Campos extras de resposta são aceitos sem ativar
`LAYA_JEV_STRICT`. O contrato upstream está em
[`docs/http-api.md`](https://github.com/NandhaKishorM/laya/blob/main/docs/http-api.md).

## Famílias de API

Além do chat, o binding expõe o mesmo núcleo Rust para: fallback de chat,
montagem de turno, embeddings e rerank com fallback; run lifecycle, dispatch e
policy de tools, loop modelo/tool e gates de verificação; stores SQLite de
lifecycle, loop e policy; ledger SQLite com backup e restore e projeção
PostgreSQL; seleção de storage e `capabilities`; vetores (LanceDB e Qdrant), KV,
filas com DLQ e pub/sub (Redis e NATS). Todas são experimentais; o estado de
cada uma está em `docs/plans/03-plano-continuidade.md`.
