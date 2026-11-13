# Plano de continuidade da Vectora Engine

**Estado verificado:** 13 de novembro de 2026. `main` no GitHub está em
`dc1aa5e3e9357903d85c8bf82bfcb6b9e4cf59d1` (PRs #45–#72 integrados por
squash). A stack aberta é `main` ← [#73](https://github.com/vectora-company/engine/pull/73)
← [#76](https://github.com/vectora-company/engine/pull/76) ←
[#77](https://github.com/vectora-company/engine/pull/77) ←
[#79](https://github.com/vectora-company/engine/pull/79) ←
[#80](https://github.com/vectora-company/engine/pull/80) ←
[#81](https://github.com/vectora-company/engine/pull/81) ←
[#82](https://github.com/vectora-company/engine/pull/82) ← PR documental
[#83](https://github.com/vectora-company/engine/pull/83) ← [#84](https://github.com/vectora-company/engine/pull/84) ← [#85](https://github.com/vectora-company/engine/pull/85) ← [#86](https://github.com/vectora-company/engine/pull/86) ← PR documental [#87](https://github.com/vectora-company/engine/pull/87) (`docs/revisao-markdown-pos-86`) ← [#88](https://github.com/vectora-company/engine/pull/88) (`fix/chat-media-reclassify-on-hydration`). Os PRs #82–#88 estão abertos; o #82 tem head `5c36f985f71717ad5f604f8aa821bcd0a07862d3`. O PR documental [#83](https://github.com/vectora-company/engine/pull/83) está
empilhado sobre a branch `feat/chat-media-bindings` do #82. A branch filha `feat/chat-media-provider-limits-docs83` está publicada no [PR
#84](https://github.com/vectora-company/engine/pull/84), head
`58980b3647e96b495756f7986bfdbfdd40d09e8c` (limites por provider/modelo; docs sincronizados).

**Objetivo de produto:** a primeira release deve ser ampla e completa, não um
MVP. A proposta de escopo e os gates ainda precisam ser consolidados neste
plano. Isso não implica substituir o backend Python nem presumir paridade sem
testes.

## Proposta de escopo da primeira release

O pedido de produto é uma primeira release completa, não um MVP. A proposta
para o charter 1.0 é cobrir uma jornada de agente utilizável de ponta a ponta:

- providers de chat e multimodalidade com fallback, streaming, erros e limites
  conhecidos;
- runtime com lifecycle, coordenação da execução de tools via adapters,
  policy/aprovação, cancelamento propagado e observável, recovery e evidência
  de verificação;
- conversas/eventos duráveis, cursor e replay;
- storage com equivalência de base em cada par definido, health/readiness,
  migrations e recovery;
- memória, contexto e RAG que integrem embeddings, vetores, busca lexical ou
  híbrida e rerank;
- APIs Rust, Python e TypeScript com fixtures de conformidade, distribuição
  suportada, compatibilidade/versionamento e documentação operacional;
- testes de falha, reinício, concorrência, segurança e adoção reversível em
  consumidores reais.

Autenticação, contas, billing, UI, workbenches e UX continuam no host. Isso é
uma proposta de escopo para orientar a revisão; antes de congelar o contrato da
release, o plano deve decompor cada linha em entregas, dependências e critérios
de aceite testáveis. A lista de capacidades planejadas no README ou nas specs
não é promessa de que todas serão incluídas automaticamente.

Este é o plano ativo do [goal loop](../goal-loop/README.md). Ele substitui as
fotografias anteriores (`a13140b`, `22c861f`), que ficam no histórico do Git.
Cada item do backlog indica a origem (spec, plano ou análise), o critério de
saída e as dependências. **[D]** marca itens que exigem decisão de `brunosrz`
antes de fixar contrato; a pergunta é feita no chat depois de concluído o
trabalho independente.

## Estado verificado

| Capability                                                                              | Estado                                                                                                                                                                | Evidência                                                                                                                                                                             |
| --------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Chat OpenAI-compatible, Anthropic, Gemini, Cohere, fallback, montagem de turno          | Implementada (experimental), bindings Python e TypeScript                                                                                                             | `chat/openai_compatible.rs`, `chat/native_providers.rs`, `chat/fallback.rs`, `chat/turn.rs`; fixture `tests/fixtures/conformance-v1.json` (fallback com cancelamento nos três níveis) |
| Embeddings e rerank com fallback por quota                                              | Implementada (experimental)                                                                                                                                           | `llm/embeddings.rs`, `llm/rerank.rs`; testes Python                                                                                                                                   |
| Run lifecycle, dispatch, policy/aprovação, loop modelo/tool, gates de verificação       | Implementada no core e nos bindings                                                                                                                                   | `harness/*.rs`, `harness/model_tool_loop.rs`, `harness/policy.rs`, `harness/verification.rs`                                                                                          |
| Stores SQLite de lifecycle, loop e policy                                               | Implementada                                                                                                                                                          | `storage/sqlite_run.rs`, `sqlite_loop.rs`, `sqlite_policy.rs`                                                                                                                         |
| Ledger SQLite: idempotência, conflito de versão, multiprocesso, restart                 | Implementada                                                                                                                                                          | `storage/sqlite.rs` (`multiprocess_child_writer`, `concurrent_appends_with_same_expected_version_commit_exactly_one`)                                                                 |
| Backup/restore SQLite online e offline coordenado                                       | Implementada (#53)                                                                                                                                                    | `storage/sqlite/backup.rs` (`backup_online`, `backup_offline`, `verify_backup`, `restore`)                                                                                            |
| Projeção PostgreSQL com SQLite como autoridade                                          | Implementada                                                                                                                                                          | `storage/resilient_ledger.rs` (`flush_outbox`); teste `projection_resumes_after_postgres_is_stopped_and_started`                                                                      |
| Vetorial: cosseno em `[-1, 1]`, dimensões 1–65536, coleção ausente, dimensão divergente | Implementada (#57, #65, #77)                                                                                                                                          | `storage/vector.rs` (`DistanceType::Cosine`, `valid_vector_dimensions`, `validate_lance_dimensions`)                                                                                  |
| KV: TTL nativo no NATS (2.11+), CAS por revisão, incremento, prefixo, watch             | Implementada (#59, #67–#72)                                                                                                                                           | `storage/messaging.rs` (`allow_message_ttl`, `compare_and_set`, `increment`, `list_keys`, `watch`)                                                                                    |
| Filas: retenção finita (100000), 5 tentativas, DLQ consumível                           | Implementada (#60–#64)                                                                                                                                                | `storage/messaging.rs` (`max_messages`, `max_attempts`, `receive_dead_letter`)                                                                                                        |
| Relatório estático de capabilities por backend                                          | Implementada (#46)                                                                                                                                                    | `storage/capabilities.rs`                                                                                                                                                             |
| Snapshot de contratos para hosts TypeScript                                             | Implementada (#45)                                                                                                                                                    | `bindings/typescript/src/index.ts` (`engineContractSnapshot`)                                                                                                                         |
| Mídia no chat: tipos, preparação, contexto e serialização por protocolo                 | Implementada parcialmente nos PRs #79–#86; limites por provider/modelo no #84, retry multimodal no #85 e resultados tipados no #86; documentação atualizada no PR #87 | `chat/media.rs`, adapters OpenAI/native/fallback; helper e contexto Python/TS; checklist `docs/checklists/04-midia-no-chat.md`                                                        |

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

1. **Mídia nativa e integração do preparo — implementadas nos PRs abertos #79–#82; refinadas nos #84–#86.**
   #79 adiciona payloads nativos nos protocolos cobertos; #81 conecta `MediaStore`
   ao envio Rust; #82 expõe preparo manual, contexto efêmero por chamada e contexto
   padrão nos bindings. Como a stack ainda está aberta, isso não está integrado
   em `main`. Retry textual único para rejeição multimodal classificada está no PR aberto #85; `ToolResult` tipado está no PR aberto #86, filho do #85; catálogo automático permanece pendente. OpenAI Responses não tem adapter.
   Evidências e critérios: [`docs/checklists/04-midia-no-chat.md`](../checklists/04-midia-no-chat.md).
2. **Mídia: imagens de resultado de tool — implementado nesta etapa.** `ToolResult` usa partes tipadas; Anthropic, Gemini e OpenAI Chat Completions mapeiam imagens conforme a spec 17. Cohere mantém representação textual porque o protocolo não declara suporte a imagem no resultado. Evidência e testes estão registrados na spec 17 e checklist de mídia.
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
todos integrados em stack em 2026-11-13. A Engine mantém implementação Rust e
contratos compartilháveis; UI, STT local, previews e empacotamento permanecem no
Vectora consumidor.

1. **Payload nativo:** PR #79 sobre #77 implementou Anthropic, Gemini e OpenAI
   Chat Completions.
2. **Decisão resolvida no PR #80:** oferecer preparo manual, contexto efêmero por
   chamada e contexto padrão por cliente; chamada explícita prevalece sobre o
   padrão. `MediaStore` é async e `ChatRequest` não carrega bytes persistidos.
3. **Core Rust — entregue no PR #81:** branch filha do #80 integra o contexto aos
   clients OpenAI-compatible, native e fallback. Limita cada asset a 20 MiB por
   padrão (configurável), prepara apenas a mensagem do usuário atual e resultados
   de tools posteriores, e reporta decisões sem bytes na completion e no stream.
   Entregue no PR #81. Verificações: 49 testes `chat::`, testes Rust do workspace
   e docs no hook, fmt, Clippy e typecheck TS passaram.
4. **Bindings — implementados no PR aberto #82:** `prepare_chat_content`/
   `prepareChatContent`, contexto efêmero e contexto padrão em Python/TypeScript.
   O host passa um mapa temporário `asset_id → bytes` após leitura autorizada no
   AssetStore; bytes não entram em `ChatRequest`. Evidência local registrada
   nesta stack: Rust 158 testes + 3 fixtures, Python 51 e Node 37 passaram; 10
   testes Node dependentes de serviços foram ignorados sem URLs configuradas.
   Essas contagens descrevem a execução registrada, não CI remoto nem `main`.
5. **Limites por provider/modelo/modalidade — implementados no PR aberto #84, filho do #83:**
   `provider_limits` explícito combina-se com o teto global por asset; o menor
   limite vale e ausência de dado não inventa limites do provider. O core e os bindings Python/TS têm cobertura local direcionada: 18 testes Rust,
   1 teste Python e 1 teste Node passaram; maturin e `pnpm build` passaram. Ver
   [`docs/checklists/04-midia-no-chat.md`](../checklists/04-midia-no-chat.md) e
   a análise de referências. Catálogo/metadado automático,
   endpoint/protocolo, limites agregados, MIME/digest e histórico (Vectora #481/#494) permanecem etapas posteriores.
6. **Retry textual único — PR aberto [#85](https://github.com/vectora-company/engine/pull/85), filho do #84:** somente `MultimodalIncompatible` após rejeição HTTP 400 reconhecida; no mesmo provider/modelo, uma vez, inclusive no fallback. Streams só repetem antes de devolver o stream ao consumidor; erros de autenticação/rede e erros após início do stream não são convertidos em retry. Os 51 testes `chat::` passaram; os hooks completos (Clippy, suíte Rust do workspace e rustdoc), fmt, Prettier e `git diff --check` passaram no commit `0767636ee88567b19ddce93f1e8d0b15cace396f`. Origem: Vectora #481.
7. **Partes tipadas em `ToolResult` — PR [#86](https://github.com/vectora-company/engine/pull/86), filho do #85:** brunosrz decidiu substituir `output: String` diretamente sem manter v1. A branch `feat/chat-tool-result-parts` implementa `parts` com texto/imagem, conversão central Rust e payloads por protocolo; loop v2→v3 e resultado v1→v2, sem compatibilidade antiga. Hooks completos, suíte Rust (163 testes + 3 fixtures), Clippy, check, rustdoc, Python e Node passaram localmente no SHA `b188a94`; Node teve 37 testes aprovados e 10 dependentes de serviços ignorados sem endpoints.
8. `Inline` não persistível nem aceito pelos bindings. **[D]** segurança/dados.
9. **Reclassificação MIME na reidratação — PR [#88](https://github.com/vectora-company/engine/pull/88), filho do #87:** usa a assinatura dos bytes, envia o MIME real quando a modalidade coincide e rebaixa para texto/referência quando os bytes indicam outra modalidade. 22 testes `chat::media::tests` e hooks completos passaram no head `18c2eca`.
10. OpenAI Responses API não existe na Engine; não aplicável até haver adapter.

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

Nenhuma capability está declarada estável. Código em PR aberto ou testes
isolados não satisfazem, por si só, o gate de release; consulte a matriz de
conformidade para o estado e a cobertura de cada capability.

## Próxima etapa

A revisão Markdown inicial foi publicada no PR #83; uma segunda revisão ampla
está no PR [#87](https://github.com/vectora-company/engine/pull/87), filho do #86, porque a stack posterior tornou
referências de estado e versões obsoletas. Depois dela, a próxima etapa independente
é consolidar uma proposta verificável de charter para a primeira release completa: áreas incluídas, critérios de maturidade, paridade
Rust/Python/TypeScript, compatibilidade, suporte de plataformas, publicação e
adoção-piloto. O plano atual contém gaps por capability, mas ainda não os
organiza como um único gate de release 1.0.

## Desvios registrados

- A mídia no chat (#73, #76) foi iniciada antes do piloto e das medições da
  Fase 6, ao contrário da ordem do plano anterior. Ela segue como Fase 1 porque
  a spec 17 foi aprovada e a correção da Fase 0 depende dela.
- O `origin` do checkout local pode apontar para um clone desatualizado; o
  estado da stack é sempre conferido no GitHub.
