# Binding TypeScript/Node

O pacote `@vectora/engine` no npm usa napi-rs sobre o núcleo Rust. A API pública
em `src/` fornece tipos TypeScript e converte as mensagens JSON da fronteira
nativa; transporte, SSE, validação e erros continuam no Rust.

## Desenvolvimento

Na raiz do repositório, `scons install` instala Rust, tooling Python e
dependências pnpm. Dentro deste diretório, `pnpm build` compila o addon local e
gera a fachada JavaScript e as declarações. `pnpm test` executa a suíte do pacote
contra o addon local; os cenários com Redis, NATS, Qdrant e PostgreSQL só rodam
quando as variáveis `VECTORA_TEST_*` estão definidas. O build de artefatos para
outras plataformas e a publicação no npm ainda não estão configurados e exigem
um workflow de release separado.

Consuma `chatComplete(settings, request)` como `Promise` ou `chatStream` como
`AsyncIterable`. Encerrar a iteração cancela a tarefa Rust e a requisição HTTP.
Os objetos `settings`, `request` e eventos seguem
`docs/specs/03-chat-openai-compatible.md`.

Mídia usa `MediaExecutionContext` com `assets: Record<string, Uint8Array>`.
O mapa é temporário; o host valida autorização do asset antes de fornecê-lo.
`prepareChatContent` prepara manualmente; o terceiro argumento de
`chatComplete`/`chatStream` fornece contexto por chamada; `media_context` em
settings configura o padrão. Contexto por chamada tem precedência; limite padrão
de 20 MiB.

Anthropic Messages, Google Gemini e Cohere Chat v2 usam `nativeChatComplete` e
`nativeChatStream` com o mesmo contrato Rust. `settings.provider` deve ser
`anthropic`, `google` ou `cohere`; credenciais e configuração específica pertencem ao
consumer. Campos como Gemini `safetySettings` podem ser passados em
`request.extensions`, sem defaults de política de produto.

## Famílias de API

Além do chat, o binding expõe o mesmo núcleo Rust para: fallback de chat,
montagem de turno, embeddings e rerank com fallback; run lifecycle, dispatch e
policy de tools, loop modelo/tool e gates de verificação; stores SQLite de
lifecycle, loop e policy; ledger SQLite com backup e restore e projeção
PostgreSQL; seleção de storage e `capabilities`; vetores (LanceDB e Qdrant), KV,
filas com DLQ e pub/sub (Redis e NATS). Todas são experimentais; o estado de
cada uma está em `docs/plans/03-plano-continuidade.md`.
