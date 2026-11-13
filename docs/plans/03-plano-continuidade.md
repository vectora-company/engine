# Plano de continuidade da Vectora Engine

**Estado verificado:** 13 de novembro de 2026, contra o código da branch do topo
da stack (`fix/storage-vector-collection-dimensions`, PR
[#77](https://github.com/vectora-company/engine/pull/77) ←
[#79](https://github.com/vectora-company/engine/pull/79); a branch local
`feat/chat-media-routing-evidence` está empilhada sobre #79.
**`main` no GitHub:** `dc1aa5e` (PRs #45–#72 integrados por squash).
**Stack aberta:** `main` ← [#73](https://github.com/vectora-company/engine/pull/73)
(spec 17) ← [#76](https://github.com/vectora-company/engine/pull/76) (tipos de
mídia) ← [#77](https://github.com/vectora-company/engine/pull/77) (dimensão
vetorial) ← [#79](https://github.com/vectora-company/engine/pull/79) (payload
nativo).
**Objetivo:** transformar as capabilities experimentais numa engine
distribuível e adotável com segurança, sem substituir o backend Python nem
presumir paridade.

Este é o plano ativo do [goal loop](../goal-loop/README.md). Ele substitui as
fotografias anteriores (`a13140b`, `22c861f`), que ficam no histórico do Git.
Cada item do backlog indica a origem (spec, plano ou análise), o critério de
saída e as dependências. **[D]** marca itens que exigem decisão de `brunosrz`
antes de fixar contrato; a pergunta é feita no chat depois de concluído o
trabalho independente.

## Estado verificado

| Capability                                                                              | Estado                                                                       | Evidência                                                                                                                                                                             |
| --------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Chat OpenAI-compatible, Anthropic, Gemini, Cohere, fallback, montagem de turno          | Implementada (experimental), bindings Python e TypeScript                    | `chat/openai_compatible.rs`, `chat/native_providers.rs`, `chat/fallback.rs`, `chat/turn.rs`; fixture `tests/fixtures/conformance-v1.json` (fallback com cancelamento nos três níveis) |
| Embeddings e rerank com fallback por quota                                              | Implementada (experimental)                                                  | `llm/embeddings.rs`, `llm/rerank.rs`; testes Python                                                                                                                                   |
| Run lifecycle, dispatch, policy/aprovação, loop modelo/tool, gates de verificação       | Implementada no core e nos bindings                                          | `harness/*.rs`, `harness/model_tool_loop.rs`, `harness/policy.rs`, `harness/verification.rs`                                                                                          |
| Stores SQLite de lifecycle, loop e policy                                               | Implementada                                                                 | `storage/sqlite_run.rs`, `sqlite_loop.rs`, `sqlite_policy.rs`                                                                                                                         |
| Ledger SQLite: idempotência, conflito de versão, multiprocesso, restart                 | Implementada                                                                 | `storage/sqlite.rs` (`multiprocess_child_writer`, `concurrent_appends_with_same_expected_version_commit_exactly_one`)                                                                 |
| Backup/restore SQLite online e offline coordenado                                       | Implementada (#53)                                                           | `storage/sqlite/backup.rs` (`backup_online`, `backup_offline`, `verify_backup`, `restore`)                                                                                            |
| Projeção PostgreSQL com SQLite como autoridade                                          | Implementada                                                                 | `storage/resilient_ledger.rs` (`flush_outbox`); teste `projection_resumes_after_postgres_is_stopped_and_started`                                                                      |
| Vetorial: cosseno em `[-1, 1]`, dimensões 1–65536, coleção ausente, dimensão divergente | Implementada (#57, #65, #77)                                                 | `storage/vector.rs` (`DistanceType::Cosine`, `valid_vector_dimensions`, `validate_lance_dimensions`)                                                                                  |
| KV: TTL nativo no NATS (2.11+), CAS por revisão, incremento, prefixo, watch             | Implementada (#59, #67–#72)                                                  | `storage/messaging.rs` (`allow_message_ttl`, `compare_and_set`, `increment`, `list_keys`, `watch`)                                                                                    |
| Filas: retenção finita (100000), 5 tentativas, DLQ consumível                           | Implementada (#60–#64)                                                       | `storage/messaging.rs` (`max_messages`, `max_attempts`, `receive_dead_letter`)                                                                                                        |
| Relatório estático de capabilities por backend                                          | Implementada (#46)                                                           | `storage/capabilities.rs`                                                                                                                                                             |
| Snapshot de contratos para hosts TypeScript                                             | Implementada (#45)                                                           | `bindings/typescript/src/index.ts` (`engineContractSnapshot`)                                                                                                                         |
| Mídia no chat: tipos, classificação, capability por modalidade, decisão por parte       | Tipos e serialização por protocolo no PR #79; integração e limites pendentes | `chat/media.rs`, `chat/native_providers.rs`, `chat/openai_compatible.rs`; testes `chat::` 44/44; checklist `docs/checklists/04-midia-no-chat.md`                                      |

Os testes service-real (Redis, NATS, Qdrant, PostgreSQL) são condicionais às
variáveis `VECTORA_TEST_*`; sem elas contam como não executados. Evidências e
contagens por execução estão na
[auditoria de storage](../analyses/05-auditoria-storage-s0-s5.md).

## Decisões vigentes

As decisões abaixo foram tomadas por `brunosrz` e estão registradas na fonte
indicada. Não as reabra sem pedido dele.

- Equivalência por par de storage e especialização só por incompatibilidade:
  `AGENTS.md` (regra 5) e [spec 02](../specs/02-storage-memoria-contexto-rag.md).
- SQLite é a cópia local obrigatória e a autoridade de recuperação do ledger e
  dos estados críticos; PostgreSQL é projeção ou replicação retomável:
  [plano de storage](04-plano-storage-ledger.md).
- Backup do ledger SQLite em dois modos (online e offline coordenado pelo host)
  e sem limite novo de tamanho para metadata vetorial, valores de KV e payloads
  de fila (o ledger mantém 1 MiB por evento):
  [auditoria de storage](../analyses/05-auditoria-storage-s0-s5.md).
- NATS Server 2.11 ou superior para TTL por chave; filas com retenção finita
  configurável (100000), cinco tentativas e DLQ; vetores com cosseno em
  `[-1, 1]` e dimensões de 1 a 65536.
- Processo: PRs sempre empilhados; commit, push e PR sem pedido; aprovar PR e
  criar issue só a pedido; decisão vital perguntada no chat; CI remoto não é
  evidência: [loop de desenvolvimento](05-loop-de-desenvolvimento.md).

## Backlog por fases

### Fase 0 — correções imediatas

1. **Mídia: decisão `native` sem envio nativo — implementada nesta etapa.**
   O PR #79, empilhado sobre #77, serializa PDF base64 na Anthropic, PDF e áudio
   WAV/MP3 no OpenAI Chat Completions, e PDF/áudio/vídeo em `inlineData` no
   Gemini. Testes `chat::` (44), fmt e Clippy passaram. O PR desta etapa e a
   integração do preparador no envio ainda pendem; limites, fallback em rejeição
   e evidência de roteamento permanecem na Fase 1. Checklist:
   [`docs/checklists/04-midia-no-chat.md`](../checklists/04-midia-no-chat.md).
   OpenAI Responses continua fora do adapter. Origem: spec 17.
2. **Mídia: imagens de resultado de tool descartadas.** `message_text` em
   `chat/native_providers.rs` ignora `ImageUrl` nas mensagens de tool. Saída:
   mapeamento da tabela da spec 17 ou forma textual registrada. Origem: spec 17.
3. **Segredo em `Debug`.** `storage/vector.rs::VectorStoreConfig` deriva
   `Debug` com `api_key`. Saída: `Debug` manual com redação, como em
   `chat/openai_compatible.rs`. Origem: spec 01.
4. **Testes ausentes.** `SqliteModelToolLoopStore::submit_tool_result`,
   `recover` e `reconcile_tool_result` (spec 13), `PolicyDigestReuse` e
   `StateTooLarge` (spec 16), KV (CAS, incremento, watch, prefixo, delete) em
   Rust e Python (hoje só Node).
5. **Validação real de #62–#64.** Transferência para a DLQ no NATS e corrida
   ACK×transferência no Redis; uma entrega por leitura e expiração no JetStream;
   preservação de configuração e recusa de subjects incompatíveis. Origem:
   [checklist de storage](../checklists/03-auditoria-storage-s0-s5.md).
6. **`capabilities()` incompleto.** Declarar backup no SQLite, a dependência de
   `notify-keyspace-events` no watch do Redis e as ausências reais das filas e
   do vetorial (filtros, índice, quantização).

### Fase 1 — spec 17, mídia no chat

Origem: [spec 17](../specs/17-midia-no-chat.md),
[issue #58](https://github.com/vectora-company/engine/issues/58) e evidência do
consumidor [Vectora #479–#502](https://github.com/vectora-company/vectora/pulls),
todos integrados em stack em 2026-11-13. O consumidor fechou a jornada multimodal
em Python, inclusive UI/transcrição local; a Engine implementa somente contratos
compartilháveis e mantém sua própria implementação Rust.

1. **Etapa atual:** serialização de PDF/áudio/vídeo em formatos nativos nos
   adapters Anthropic, Gemini e OpenAI Chat Completions. Código e testes locais
   passaram; PR #79 foi aberto sobre #77. A próxima etapa registra a decisão vital em branch filha.
2. **[DECISÃO VITAL]** Definir como `MediaStore` e capabilities chegam ao envio
   real: contexto efêmero por chamada, dependência no cliente ou preparação pelo
   consumidor. A pergunta, opções e recomendação estão em
   [`docs/analyses/00-analise-e-referencias-de-desenvolvimento.md`](../analyses/00-analise-e-referencias-de-desenvolvimento.md).
   Também decidir se a leitura do store é async. Não alterar API pública antes da
   resposta. Após decidir, integrar `prepare_content` ao envio, resolver
   capability por modelo com metadados/catálogo e produzir payload mais
   evidência/evento de roteamento provider-neutral sem bytes/segredos.
3. Limitar bytes por provider/modelo e reidratar apenas mídia do turno atual;
   expor erro estável e fallback textual sem persistir bytes. Validação precisa
   cobrir limites, MIME/digest e histórico (Vectora #481/#494).
4. Retry no máximo uma vez como texto apenas em rejeição específica de mídia;
   preservar erros de rede/autenticação sem retry mascarado (Vectora #481).
5. Resultado de tool em partes (texto e imagem por `MediaRef`) com mapeamento
   por protocolo; snapshot do loop lendo os anteriores. **[D]** compatibilidade
   do contrato `ToolResult`.
6. `Inline` não persistível nem aceito pelos bindings. **[D]** segurança/dados.
7. Reclassificar bytes ao reidratar e tratar MIME divergente.
8. Bindings Python/TypeScript e fixtures de conformidade nos três níveis;
   resolver o contrato compartilhado de `MediaStore` antes de publicar.
9. STT, OCR, renderização/previews, UI, permissões de OS e empacotamento são
   capacidades do consumidor nesta fase. Só promover uma delas ao core após
   definir contrato comum e necessidade em mais de um consumer.
10. OpenAI Responses API não existe na Engine; não aplicável até existir adapter.

### Fase 2 — storage: base equivalente

Origem: [plano de storage](04-plano-storage-ledger.md) e
[spec 02](../specs/02-storage-memoria-contexto-rag.md).

1. Vetorial nos dois backends: filtro por metadado, índice ANN parametrizado,
   quantização, busca lexical/híbrida, falhas de indexação visíveis e
   backup/snapshot. **[D]** mapeamento comum de índices.
2. Filas nos dois backends: NACK e reentrega com atraso, deduplicação na
   publicação, replay e ordenação com contrato e teste, shutdown gracioso dos
   consumers e redrive atômico da DLQ; envelope de job versionado. **[D]**
   envelope de job.
3. Pub/sub: reconexão observável e assinatura por padrão/curinga.
4. `capabilities()` estruturado (lacuna, emulação, especialização com motivo).
   **[D]** formato.
5. Health/readiness com os seis estados da spec 02 e observabilidade sem
   segredos (adapter, duração, retry, correlação). **[D]** biblioteca de
   tracing/métricas.
6. `flush_outbox` e `pending_count` do ledger resiliente nos bindings; fixtures
   comuns de KV, filas e vetores nos três níveis.
7. PostgreSQL como projeção dos stores de lifecycle, loop e policy, backup e
   verificação equivalentes, testes concorrentes com serviço real e migrations
   além da v1 com caminho de rollback documentado.
8. Interface SQL genérica tipada e limitada em SQLite e PostgreSQL. **[D]**
   segurança e escopo.
9. Migração de dados (fase 6 da spec 02): contagens, hashes e rollback.

### Fase 3 — eventos, conversas e replay

Origem: [spec 04](../specs/04-conversas-streaming-e-threads.md) e spec 02.

1. Envelope público de evento (event_id, sequência/cursor, versão de schema,
   IDs correlacionados, timestamp) e classes mínimas. **[D]** dados.
2. Gaps representáveis e replay por cursor com reconexão sem duplicação.
3. API de conversa/sessão, histórico paginado e projeções; admissão com
   `queued`, `blocked` e `rejected` e concorrência por conversa. **[D]**.
4. Buffer, coalescência e cancelamento do assinante.
5. Referência de artifact fora do ledger (hash, tamanho, expiração, acesso).
   **[D]** dados e segurança.

### Fase 4 — runtime agêntico

Origem: [spec 01](../specs/01-runtime-agentico-e-harness.md) e
[plano do loop agêntico](02-plano-loop-agentico.md).

1. Tipos `Turn`, `Step`, `Artifact` e evento terminal versionado.
2. Orçamento de tempo, tokens, custo e contexto no loop e no `PolicySnapshot`.
3. Output schema de tool, validação de saída estruturada e dry-run declarado.
4. Ligação automática lifecycle, loop e policy (revalidação antes de cada
   efeito).
5. Hooks pre-tool, post-tool e pre-final; Plan Mode; subagentes com escopo e
   orçamento derivados; contrato de isolamento/sandbox. **[D]** segurança.
6. Decision Models. **[D]**.
7. Redaction e fixtures de dois perfis de consumidor.

### Fase 5 — memória, contexto e RAG

Origem: spec 02 (fase S5 do plano de storage). Contratos de memória,
documento, chunk e proveniência; Context Planner determinístico; compactação
que preserva unidades; indexação com falha visível e RAG opt-in (lexical,
híbrido, rerank). **[D]** dados e segurança. Depende das fases 2 e 3.

### Fase 6 — adoção, distribuição e medição

1. Integração do host de policy (recompilar, autenticar recibo, revalidar antes
   do efeito) e de cancelamento (propagar, impor timeout no ponto de execução,
   testes de falha parcial e resultado `unknown`).
2. GH Bot consumindo `engineContractSnapshot()`; ampliar o snapshot para
   storage e mídia e expô-lo também em Python. O Bot usa hoje só `chatStream`
   e monta as versões em `app/src/server/engine.ts`
   (`/media/brunosrz/externo/vectora-repos/gh-bot`); a troca é um PR no
   repositório do Bot. Integração com `ModelToolLoop`, `ToolPolicy` e
   `VerificationGate` vem depois.
3. Piloto reversível no Vectora App, uma capability por vez, sem chamadas
   paralelas a providers reais.
4. Publicação em crates.io, PyPI e npm com matriz de plataformas e workflow de
   release separado do CI; alinhar versões (crate `0.0.1`, bindings `0.1.0`).
   **[D]** política de versão.
5. Medições reproduzíveis por SHA (latência, memória, throughput, custo,
   recuperação).
6. Ports do PR7 do [plano de portabilidade](01-plano-portabilidade-llm-rust.md)
   (STT, imagem, TTS, vídeo, web search) e SystemOne, depois do piloto e das
   medições.

## Gates de adoção e release

Uma capability só avança de experimental quando:

- contratos e erros são versionados, documentados e testados nas três entradas;
- timeout, cancelamento, redaction e falhas/degradação têm resultado observável;
- concorrência, restart e idempotência são testados quando houver efeitos;
- fallback e rollback do consumidor são testados sem duplicar efeitos;
- métricas são reproduzíveis e limitadas ao workload medido;
- consumidores e limites de compatibilidade estão identificados.

Nenhuma capability passou esse gate até esta revisão.

## Próxima etapa

Fase 0, item 1: corrigir a incoerência entre a decisão `native` e o envio da
mídia, em PR empilhado sobre o topo da stack.

## Desvios registrados

- A mídia no chat (#73, #76) foi iniciada antes do piloto e das medições da
  Fase 6, ao contrário da ordem do plano anterior. Ela segue como Fase 1 porque
  a spec 17 foi aprovada e a correção da Fase 0 depende dela.
- O `origin` do checkout local pode apontar para um clone desatualizado; o
  estado da stack é sempre conferido no GitHub.
