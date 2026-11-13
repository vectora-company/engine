# Changelog

## Unreleased

Capabilities experimentais; nenhuma está declarada estável. O estado verificado
e o backlog estão em `docs/plans/03-plano-continuidade.md`.

### Chat e modelos

- Contrato de chat OpenAI-compatible com streaming SSE, erros tipados e
  extensões de gateway (#3, #4).
- Providers nativos Anthropic, Google Gemini e Cohere (#5, #6).
- Fallback explícito de chat no core, sem replay após saída parcial (#7, #14).
- Embeddings e rerank (Cohere, Voyage, OpenRouter, Ollama) com fallback por
  quota (#8).
- Mídia no chat (spec 17): payloads nativos de PDF para Anthropic, Gemini e
  OpenAI Chat Completions; áudio WAV/MP3 no OpenAI e áudio/vídeo no Gemini.
  Preparo por `MediaStore` e os três modos de integração estão nos PRs abertos
  #79–#82; ainda não estão integrados em `main`.

### Harness

- Run lifecycle, montagem de turno, contratos e dispatch de tools, loop
  modelo/tool, policy e aprovação, e gates de verificação (#9–#16 e seguintes).
- Stores SQLite duráveis de lifecycle, loop e policy, com cancelamento e
  reconciliação.

### Storage e mensageria

- Seleção de storage com seis backends e relatório `capabilities()` (#15, #43,
  #46).
- Ledger SQLite com concorrência entre processos, backup online e offline
  coordenado e restore (#49, #53); projeção PostgreSQL com retomada após queda.
- Vetores LanceDB e Qdrant com score cosseno comum e limites de dimensão (#57,
  #65, dimensão divergente da coleção na stack aberta).
- KV Redis e NATS JetStream com TTL (NATS 2.11+), prefixo, remoção, CAS,
  incremento e watch (#59, #67–#72).
- Filas Redis Streams e NATS JetStream com retenção finita, tentativas e DLQ
  (#60–#64); pub/sub efêmero.

### Bindings

- Bindings Python e TypeScript/Node para as capabilities acima, com fixture de
  conformidade comum (`tests/fixtures/conformance-v1.json`).
