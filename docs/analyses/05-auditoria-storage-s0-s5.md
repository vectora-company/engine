# Matriz de conformidade das capabilities

**Base de referência:** `main` no GitHub em
`4140ddeffa646c4dfd2f0e6a7402a1b00e337099`; a matriz foi atualizada em
2026-11-17 com evidências locais da etapa ativa. O status publicado e a base da
stack variam e devem ser consultados no GitHub no momento da entrega. Código
local ou em revisão não equivale a integração em `main`. A matriz registra
gaps de conformidade, não um selo de maturidade ou prontidão de release.
**Plano:** [`03-plano-continuidade.md`](../plans/03-plano-continuidade.md)
**Checklist:** [`02-matriz-conformidade-capabilities.md`](../checklists/02-matriz-conformidade-capabilities.md)

Esta matriz distingue código, binding, testes existentes e maturidade. A
presença de uma função ou teste não prova conformidade integral nem uso por um
consumer. O backlog atual está no [plano ativo](../plans/03-plano-continuidade.md).

## Legenda

- **Implementada experimentalmente:** existe no core e a API indicada chega ao
  binding; pode continuar incompleta para produção ou adoção por consumer.
- **Parcial:** somente parte do contrato planejado está implementada, ou a
  seleção/configuração existe sem adapter executável.
- **Planejada:** a capability está especificada, mas não há API correspondente
  no core/bindings.
- **Cobertura local:** Rust tem testes unitários nos módulos citados; Python e
  TypeScript têm testes de binding. Isso não significa que cada adapter foi
  testado contra um serviço real.

## Matriz por capability

| Capability e contrato                                                           | Rust                                                                                    | Python                                                                                        | TypeScript/Node                                                                  | Evidência de teste em `main`                                                                                                                                                                 | Maturidade e limite observado                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Chat comum, mensagens, completion e eventos — `CHAT_CONTRACT_VERSION=1`         | `chat/types.rs`, `chat/openai_compatible.rs`                                            | `chat_complete`, `chat_stream`                                                                | `chatComplete`, `chatStream`                                                     | Rust `chat/*`; Python/TS têm testes locais de protocolo                                                                                                                                      | **Implementada experimentalmente.** Transporte OpenAI-compatible e SSE. Revisões de redirects, headers e campos nulos presentes na branch de correção não fazem parte do SHA auditado.                                                                                                                                                                                                                                                               |
| Providers nativos Anthropic, Google e Cohere — contrato de chat v1              | `chat/native_providers.rs`                                                              | `native_chat_complete`, `native_chat_stream`                                                  | `nativeChatComplete`, `nativeChatStream`                                         | Conversão request/response/stream nos três níveis de teste                                                                                                                                   | **Implementada experimentalmente.** Sem catálogo, descoberta de credenciais, billing ou policy de produto.                                                                                                                                                                                                                                                                                                                                           |
| Fallback de chat                                                                | `chat/fallback.rs`                                                                      | `fallback_chat_complete`, `fallback_chat_stream`                                              | `fallbackChatComplete`, `fallbackChatStream`                                     | Fixture comum de completion, stream e interrupção da iteração nos três bindings; testes locais passaram                                                                                      | **Implementada experimentalmente.** Candidatos são explícitos e fornecidos pelo host; não há decisão automática de provider. Interromper a leitura do stream não comprova tempo de cancelamento do socket upstream.                                                                                                                                                                                                                                  |
| Montagem de turno streaming — chat v1                                           | `chat/turn.rs::assemble_turn`                                                           | `assemble_turn`                                                                               | `assembleTurn`                                                                   | Testes Rust e casos de binding para fragmentos, limites e evento terminal                                                                                                                    | **Implementada experimentalmente.** Acumula eventos em completion; não é armazenamento de conversa, sessão ou replay.                                                                                                                                                                                                                                                                                                                                |
| Embeddings — `EMBEDDING_CONTRACT_VERSION=1`                                     | `llm/embeddings.rs`                                                                     | `embed`, `fallback_embed`                                                                     | `embed`, `fallbackEmbed`                                                         | Adaptadores de protocolo e fallback local em Python/TS, testes unitários Rust                                                                                                                | **Implementada experimentalmente.** Fallback e limites de cada provider precisam manter cobertura por protocolo. Não há pipeline de indexação ou memória.                                                                                                                                                                                                                                                                                            |
| Rerank — `RERANK_CONTRACT_VERSION=1`                                            | `llm/rerank.rs`                                                                         | `rerank`, `fallback_rerank`                                                                   | `rerank`, `fallbackRerank`                                                       | Protocolo, ordenação e respostas vazias cobertos localmente                                                                                                                                  | **Implementada experimentalmente.** Não define recuperação ou RAG por si só.                                                                                                                                                                                                                                                                                                                                                                         |
| Identidade e máquina de estados de run — `RUN_CONTRACT_VERSION=1`               | `harness.rs`                                                                            | `create_run`, `transition_run`                                                                | `createRun`, `transitionRun`                                                     | Transições válidas/inválidas no core e bindings                                                                                                                                              | **Implementada experimentalmente.** Máquina de estados pura; não autentica a identidade fornecida pelo host.                                                                                                                                                                                                                                                                                                                                         |
| Contrato e preparo de tools — `TOOL_CONTRACT_VERSION=1`, dispatch v1            | `harness/tool_dispatch.rs`                                                              | `prepare_tool_dispatch`, `tool_chat_definitions`                                              | `prepareToolDispatch`, `toolChatDefinitions`                                     | Schema, argumentos, run ativo e default-deny cobertos localmente                                                                                                                             | **Parcial.** Valida e prepara dispatch, mas não descobre nem executa adapters, não aplica sandbox e não prova que o host aplicou timeout/capability no efeito.                                                                                                                                                                                                                                                                                       |
| Policy e aprovação — `TOOL_POLICY_CONTRACT_VERSION=1`                           | `harness/policy.rs`                                                                     | `authorize_tool_dispatch`, versões e serialização                                             | `authorizeToolDispatch`, versões e serialização                                  | Binding e testes de aprovação vinculada à chamada                                                                                                                                            | **Implementada experimentalmente.** Engine valida o vínculo; host autentica aprovador e recibo, recompila policy e executa a tool.                                                                                                                                                                                                                                                                                                                   |
| Gates de verificação — `EVIDENCE_GATE_CONTRACT_VERSION=1`                       | `harness/verification.rs`                                                               | `evaluate_verification`                                                                       | `evaluateVerification`                                                           | Precedência, evidência expirada/redação e veredito separados nos bindings                                                                                                                    | **Implementada experimentalmente.** SQLite persiste report/avaliação em stores específicos; engine não executa checks nem valida a veracidade de `redacted`.                                                                                                                                                                                                                                                                                         |
| Loop modelo/tool — `MODEL_TOOL_LOOP_CONTRACT_VERSION=3`, resultado de tool v2   | `harness/model_tool_loop.rs`                                                            | start/accept/submit/recover/cancel/reconcile                                                  | APIs equivalentes camelCase                                                      | Rust e bindings cobrem transições, cancelamento e reconciliação                                                                                                                              | **Implementada experimentalmente.** Estado de coordenação, não execução de adapter; resultado de efeito incerto requer decisão do host.                                                                                                                                                                                                                                                                                                              |
| Ledger append-only e reserva em SQLite; `CHECKPOINT_VERSION=1`                  | `storage/sqlite.rs`                                                                     | append/read/reserve/checkpoint/admit                                                          | APIs `sqlite*` correspondentes                                                   | Rust, Python e TS exercitam idempotência, reopen, conflito e checkpoint                                                                                                                      | **Implementada experimentalmente.** API é ledger de eventos/checkpoints, não interface SQL genérica nem schema de produto. Escopo e redação são responsabilidade do host.                                                                                                                                                                                                                                                                            |
| Lifecycle durável de run — store SQLite v1                                      | `storage/sqlite_run.rs`                                                                 | `sqlite_run_*`                                                                                | `sqliteRun*`                                                                     | Admission/transição/verificação/reopen testados nos bindings                                                                                                                                 | **Implementada experimentalmente.** Persistência específica SQLite; não existe adapter PostgreSQL equivalente para lifecycle.                                                                                                                                                                                                                                                                                                                        |
| Persistência de loop e verificação — store SQLite v2                            | `storage/sqlite_loop.rs`                                                                | `sqlite_model_tool_loop_*`                                                                    | `sqliteModelToolLoop*`                                                           | Restart, recovery, cancelamento e verificação persistida testados                                                                                                                            | **Implementada experimentalmente.** Snapshot v1 é migrado/leitura limitada; não coordena sozinho processos do host nem effects externos.                                                                                                                                                                                                                                                                                                             |
| Persistência de policy/aprovação — store SQLite v1                              | `storage/sqlite_policy.rs`                                                              | `sqlite_tool_policy_*`                                                                        | `sqliteToolPolicy*`                                                              | Reopen, stale state, binding e receipts cobertos nos bindings                                                                                                                                | **Implementada experimentalmente.** Não autentica aprovador e não é RBAC compartilhado entre produtos.                                                                                                                                                                                                                                                                                                                                               |
| Ledger PostgreSQL e projeção resiliente SQLite-first                            | `storage/postgres.rs`, `storage/resilient_ledger.rs`                                    | `resilient_ledger_append`                                                                     | `resilientLedgerAppend`                                                          | Rust: projeção, conflito, admission, reserva standalone e PostgreSQL parado/religado com serviço real                                                                                        | **Parcial/experimental.** PostgreSQL recebe projeção retomável do ledger SQLite-first. Eventos, checkpoints, reservations e admissions têm outbox/projeção; bindings expõem flush, contagem de pendências e rebuild/verify. Testes reais cobrem idempotência, divergência, reconstrução e concorrência entre projetores no mesmo processo. Migrations PostgreSQL versionadas com rollback e concorrência entre processos seguem pendentes.           |
| Seleção de storage e relatório de capabilities — `STORAGE_CONTRACT_VERSION=1`   | `storage.rs`, `storage/capabilities.rs`                                                 | `validate_storage_selection`, `storage_capabilities`                                          | `validateStorageSelection`, `storageCapabilities`                                | Serialização, versão, rejeição de `memory` e relatório por backend                                                                                                                           | **Implementada experimentalmente.** O relatório é estático; não tem campo estruturado de lacuna/emulação/especialização nem health/readiness.                                                                                                                                                                                                                                                                                                        |
| Busca vetorial — `VECTOR_CONTRACT_VERSION=2`                                    | `storage/vector.rs`: LanceDB e Qdrant                                                   | `vector_store_upsert/search/delete`                                                           | `vectorStoreUpsert/Search/Delete`                                                | Contrato comum com cosseno, dimensões 1–65536, dimensão divergente, coleção ausente e reinício do Qdrant; Node com Qdrant real                                                               | **Implementada experimentalmente.** Score cosseno em `[-1, 1]` nos dois. Faltam filtro por metadado, índice ANN, quantização, busca híbrida e falhas de indexação.                                                                                                                                                                                                                                                                                   |
| KV — messaging v1                                                               | `storage/messaging.rs`: Redis e NATS JetStream                                          | `kv_store_put/get/delete/list_keys/compare_and_set/increment`, `kv_watch_*`                   | equivalentes camelCase                                                           | Node existente; em 2026-11-17, testes Rust e Python com Redis 8.10.3 e NATS 2.15.3 reais cobriram CAS concorrente, incremento concorrente, prefixo/ordenação, TTL/expiração, watch e remoção | **Implementada experimentalmente.** TTL nos dois (NATS 2.11+). A cobertura indicada existe em Rust, Python e Node; falta o runner integrado e os cenários de fila/DLQ registrados no plano. Watch do Redis exige `notify-keyspace-events`.                                                                                                                                                                                                           |
| Fila durável — messaging v1                                                     | `DurableQueue`: Redis Streams e NATS JetStream                                          | `queue_*`, `queue_receive_dead_letter`                                                        | `queue*`                                                                         | Rust service-real: ACK, redelivery, retenção, cinco tentativas e DLQ consumível; bindings Python e Node                                                                                      | **Implementada experimentalmente.** Faltam NACK/atraso, deduplicação na publicação, replay, shutdown gracioso, redrive atômico e validação real dos cenários de recuperação de DLQ e ACK concorrente.                                                                                                                                                                                                                                                |
| Pub/sub efêmero — messaging v1                                                  | `EphemeralPubSub`: Redis e NATS Core                                                    | `pubsub_*`                                                                                    | `pubsub*`                                                                        | Rust service-real: mensagem sem assinante descartada                                                                                                                                         | **Implementada experimentalmente.** Não é durável; faltam reconexão observável e assinatura por curinga.                                                                                                                                                                                                                                                                                                                                             |
| Backup/restore do ledger SQLite                                                 | `storage/sqlite/backup.rs`                                                              | `sqlite_ledger_backup_online/offline`, `sqlite_ledger_verify_backup`, `sqlite_ledger_restore` | equivalentes camelCase                                                           | 5 testes Rust (snapshot com escritor ativo, `Busy`, `.replaced`, arquivos inválidos) e testes Python/Node                                                                                    | **Implementada experimentalmente.** Sem equivalente no PostgreSQL.                                                                                                                                                                                                                                                                                                                                                                                   |
| Mídia no chat (spec 17)                                                         | `chat/media.rs`, `native_providers.rs`, `openai_compatible.rs`                          | `prepare_chat_content`, `chat_complete`/`chat_stream` e equivalentes native/fallback          | `prepareChatContent`, `chatComplete`/`chatStream` e equivalentes native/fallback | Integração multimodal nos protocolos compatíveis; payload, contexto, limites, retry classificado e resultados tipados em Rust/Python/Node; assets por mapa efêmero                           | **Parcial.** Os bindings oferecem preparo manual, contexto por chamada e contexto padrão. O host autoriza e fornece bytes; requests persistíveis permanecem separados do envelope efêmero. Os limites por provider/modelo e modalidade, retry textual único após rejeição multimodal classificada e adaptação de imagem em resultados de tools estão implementados. Ainda faltam catálogo automático confiável, limites agregados e fixtures comuns. |
| Memória, Context Planner, compactação/context engineering e RAG                 | Não há módulo/API de memory/context/RAG no core atual                                   | Ausente                                                                                       | Ausente                                                                          | Ausente                                                                                                                                                                                      | **Planejada.** Embedding, vector store e ledger são primitivas; não equivalem a esses sistemas.                                                                                                                                                                                                                                                                                                                                                      |
| Conversa/sessão, timeline de eventos e replay de stream/canal                   | Eventos de chat e `run_id/thread_id` existem; não há store/API geral de sessão e replay | Ausente como API de conversas                                                                 | Ausente como API de conversas                                                    | Sem conformidade de replay/cursor/canal                                                                                                                                                      | **Parcial/planejada.** O ledger e a montagem de turno não constituem contrato de conversa, canal, subscriber ou retomada de stream.                                                                                                                                                                                                                                                                                                                  |
| Subagentes, workflow/orquestração multiagente, scheduler, skills, hooks/plugins | Sem API correspondente                                                                  | Ausente                                                                                       | Ausente                                                                          | Ausente                                                                                                                                                                                      | **Planejada.** Não inferir suporte a partir de tipos de run, dispatch ou loop modelo/tool.                                                                                                                                                                                                                                                                                                                                                           |
| Sandbox, hooks de egress, permission runtime, execução de ferramentas           | Não há execução isolada de tools no core                                                | Ausente                                                                                       | Ausente                                                                          | Ausente                                                                                                                                                                                      | **Planejada/fora do core até contrato definido.** A policy é um gate de autorização; o host ainda executa e isola adapters.                                                                                                                                                                                                                                                                                                                          |

## Conformidade e cobertura observadas

- No snapshot `main`, há **101 testes unitários Rust** declarados nos módulos
  do core, **38 testes Python** em `bindings/python/tests/test_binding.py` e
  **28 testes Node** em `bindings/typescript/test/package.test.mjs`. A contagem
  indica volume, não cobertura completa.
- A branch desta matriz adiciona uma fixture versionada compartilhada para
  lifecycle, verificação, seleção de storage, fallback, policy, loop e append
  do ledger. A fixture passou em Rust, Python e Node. A execução Node revelou
  artefatos N-API locais desatualizados em relação ao código-fonte. A compilação
  padrão `napi build --platform --esm --manifest-path native/Cargo.toml`
  concluiu e gerou o loader e o stub atuais;
  a variante anterior com `--profile dev` procurava o artefato no diretório
  errado. A suíte de pacote passou com o addon gerado. Isso cobre cenários
  escolhidos, não uma suite completa das três entradas nem uma instalação limpa
  do pacote Node.
- Não foi evidenciada nesta auditoria uma suite de integração que inicie todos
  os serviços reais (PostgreSQL, Qdrant, Redis e NATS) e prove a matriz completa
  de failure/restart/retention. Código adapter e unit tests não bastam para
  afirmar isso. Os testes existentes são condicionais às variáveis
  `VECTORA_TEST_POSTGRES_URL`, `VECTORA_TEST_QDRANT_URL`,
  `VECTORA_TEST_REDIS_URL` e `VECTORA_TEST_NATS_URL`; quando ausentes, retornam
  sem validar conectividade nem comportamento do serviço.
- Os testes de providers usam servidores locais/fakes ou transformações puras;
  não demonstram disponibilidade, performance ou compatibilidade de todos os
  serviços externos.
- Os wrappers Python e TypeScript chamam o mesmo core Rust para as superfícies
  listadas. A fixture comum compara policy/loop/ledger nos três bindings; a
  paridade global não está demonstrada.

## Gaps que alimentam o plano

1. A iteração do stream é interrompida nos três bindings após o primeiro delta;
   isso verifica a superfície do consumidor, mas não o prazo de fechamento do
   socket/upstream. A classificação e a regra de não repetir saída parcial já
   foram comparadas com código e testes.
2. Adicionar testes de integração reproduzíveis por serviço e por backend, com
   falha, reconnect, restart, ACK/redelivery, TTL/retention e encerramento.
3. **Resolvido nesta etapa:** seleção pública de `memory` foi removida de Rust
   e TypeScript e é rejeitada no parse dos bindings Python/TypeScript. Doubles
   em memória continuam internos aos testes; não representam modo de execução
   anunciado aos consumidores.
4. Desenhar as APIs de conversa/sessão/replay, memória/contexto/RAG, execução de
   tools e extensões antes de anunciar essas capacidades como harness completo.
5. Planejar stores PostgreSQL para lifecycle/loop/policy somente após definir as
   garantias e a relação com o ledger SQLite-first.

## Validações e correções dos adapters

As correções para redirects, validação do header de API key, serialização de
opcionais e independência do typecheck estão integradas em `main`; veja
[`01-auditoria-review-pr3.md`](01-auditoria-review-pr3.md).

## Fixture de conformidade

A branch desta análise acrescenta
[`conformance-v1.json`](../../tests/fixtures/conformance-v1.json), exercitada
para run lifecycle, gate de verificação, seleção de storage, fallback, policy,
loop e ledger. Ela passou nos três bindings. A verificação também encontrou
artefatos locais N-API de Node desatualizados frente às funções existentes no
core. A compilação padrão via `napi build --platform --esm --manifest-path
native/Cargo.toml` regenerou loader e stub; o problema anterior ocorria ao
informar `--profile dev`, que apontava o empacotador para o diretório incorreto.
A suíte do pacote passou com o addon gerado. Isso é uma amostra de contratos
serializáveis, não cobertura de todas as APIs nem evidência de paridade geral.
A fixture está integrada em `main` em `tests/fixtures/conformance-v1.json` e é
exercitada por `crates/vectora-engine/tests/conformance_fixtures.rs` e pelos
dois bindings. Ela ainda não cobre KV, filas, vetores nem mídia.

### Cobertura service-real observada

Há testes condicionais de PostgreSQL (ledger e lifecycle/checkpoint), projeção
SQLite-first, Qdrant (isolamento por scope e upsert/search/delete), Redis
(KV/TTL, pub/sub e redelivery de Redis Streams) e NATS (KV, pub/sub e
redelivery de JetStream). Eles não iniciam os serviços; ausência da variável de
ambiente correspondente faz o teste retornar sem executar as asserções de
serviço. Na data do snapshot não havia cobertura integrada de
indisponibilidade/reconnect, restart, retenção e falhas combinadas; ela foi
acrescentada em ciclos posteriores e está registrada na
[auditoria de storage](05-auditoria-storage-s0-s5.md). Isso não demonstra
paridade operacional.

### Conformidade KV em Rust e Python — 2026-11-17

- Rust: `cargo test -p vectora-engine --lib storage::messaging::tests --locked` passou 15/15; a suíte inclui operações contra os serviços reais, enquanto os testes de reinício opt-in não foram habilitados nesta execução. A matriz `kv_cas_increment_prefix_watch_and_delete_match_across_backends` exercitou CAS concorrente, oito incrementos concorrentes, TTL/expiração, listagem por prefixo/limite, watch de put/remove/expiração e delete nos dois serviços.
- Python: `uv run --project ../.. maturin develop --locked` concluiu. Com `VECTORA_TEST_REDIS_URL=redis://127.0.0.1:16379`, `VECTORA_TEST_REDIS_WATCH_URL=redis://127.0.0.1:16379` e `VECTORA_TEST_NATS_URL=nats://127.0.0.1:14222`, `uv run --project ../.. pytest tests/test_binding.py -q -k kv_conformance_against_real_services` passou (2 casos: Redis e NATS), incluindo TTL/expiração de watch, lista limitada e fechamento que cancela leitura pendente.
- Serviços: containers Podman temporários com `redis:8.10.3-alpine` e `nats:2.15.3-alpine`; Redis configurado com notificações `K$gxe` para os eventos de watch. Ambos foram acessados pelo loopback e não alteraram dados persistentes de usuário.
- Essa evidência valida os cenários listados na execução, não toda a matriz de storage. Continuam pendentes a automação integrada, falhas/reconexão adicionais e os cenários reais de fila/DLQ indicados no plano.

### Semântica da busca vetorial

`VECTOR_CONTRACT_VERSION` passou a 2: LanceDB configura `Cosine` e
converte sua distância em similaridade cosseno `[-1, 1]`, igual à semântica
Qdrant; nos dois adapters, score maior significa mais próximo. `capabilities()`
declara essa direção comum. A fixture usa vetores não unitários que distinguem
o ranking por cosseno do ranking L2. Verificação local em 2026-11-12: 9/9 testes
`storage::vector::tests`, incluindo LanceDB e Qdrant 1.12.4 real, e 4/4 testes
`storage::capabilities`. O teste de restart do Qdrant não foi executado nesta
rodada. Os limites de busca foram entregues em etapas posteriores; filtros, índices/ANN,
quantização e busca híbrida permanecem.

## Diagnóstico da outbox nos bindings

Os bindings Python e TypeScript expõem flush limitado da projeção
PostgreSQL e contagem exata de itens pendentes. A API preserva SQLite como cópia e autoridade local: sem PostgreSQL,
flush válido retorna zero e não remove itens. O limite fora de 1–1000 falha
antes de qualquer early return. TypeScript usa `bigint` para não truncar a
contagem.

Verificação local no commit de implementação `4f7e980`: workspace Rust 172
testes + 3 fixtures, teste nativo Python 1; Python 53; Node 39 aprovados e 10
ignorados por ausência de endpoints Redis/NATS/Qdrant; Clippy sem warnings,
rustdoc, hooks completos e diff limpo. Os testes direcionados do ledger foram
executados com PostgreSQL real. Estado registrado à época: aberto; a stack foi integrada depois
para iniciar a etapa seguinte.

## Projeção dos checkpoints dos stores SQLite

A outbox SQLite usa `checkpoint_json` e a marca persistente
`projection_enabled` para coordenar snapshots e projeção. `ResilientLedger` com URL PostgreSQL habilita a projeção e
faz backfill transacional do histórico; stores SQLite-only não acumulam cópias
na outbox. Com a projeção habilitada, admission e stores lifecycle/loop/policy
enfileiram evento + snapshot na mesma transação SQLite. O projetor grava ambos
numa transação PostgreSQL idempotente e remove a linha da outbox após confirmação
remota, mantendo ali apenas trabalho pendente.

Verificação após a correção: workspace Rust 175 testes + 3 fixtures e 1 teste
nativo; `storage::` 76/76 contra PostgreSQL 18.6, Redis 8.10.3, NATS 2.15.3 e
Qdrant 1.18.0. A projeção evento + checkpoint foi confirmada no PostgreSQL real;
34 testes SQLite focados passaram. A suíte geral confirmou os cenários de
storage real, inclusive os testes de reinício PostgreSQL; `VECTORA_TEST_PODMAN_RESTART`
não estava habilitado para os testes opt-in de Redis/NATS/Qdrant. Formatação,
Clippy `-D warnings`, rustdoc e hooks estão sendo finalizados. Python 53 e Node
39 (10 sem endpoints ignorados) são da verificação anterior; bindings não foram
alterados nesta correção. Os contêineres temporários foram removidos e os do
gerenciador do usuário permaneceram intocados.

Limites registrados nesta execução histórica: faltavam projeção consultável,
backup/verify e concorrência entre projetores. Esses itens foram implementados
depois; as lacunas restantes estão no plano ativo. O estado local SQLite continua sendo a
autoridade para recovery. A flag de projeção persiste no arquivo: depois de
configurado para PostgreSQL, o ledger continua enfileirando pendências durante a
indisponibilidade e também em aberturas posteriores sem URL PostgreSQL.

## Reservas e admissions na projeção PostgreSQL

A outbox do SQLite mantém reservations vinculadas à admission e reservas
standalone; o ledger faz backfill único dos registros anteriores. O flush
projeta reservation/admission/event/checkpoint na mesma transação PostgreSQL e
reservas standalone de forma idempotente. O adapter expõe APIs de leitura. A
migração de schema e o caminho de backfill foram verificados com serviço real.

Verificação local: workspace Rust 182 testes + 3 fixtures e 1 teste nativo;
`storage::` 83/83 contra PostgreSQL 18.6, Redis 8.10.3, NATS 2.15.3 e Qdrant
1.18.0. Migração v1 e upgrade de projeção habilitada v6→v8, backfill, reserva standalone
e admission consultável passaram; os fluxos PostgreSQL reais foram exercitados. Formatação, Clippy `-D warnings`, rustdoc e todos os hooks de pre-commit passaram.
Bindings não mudaram nesta etapa.

Backup/verify e condições negativas têm cobertura service-real. A concorrência
entre projetores foi coberta para handles independentes no mesmo processo.
Concorrência entre processos, disputa de admissions e migrations PostgreSQL com
rollback seguem pendentes. SQLite continua a cópia local e autoridade para
recuperação.

## Recuperação após perda integral do PostgreSQL

### Rebuild da projeção PostgreSQL

Implementada e integrada em `main`.

`ResilientLedger::rebuild_projection` repõe as outboxes no SQLite em uma
transação `IMMEDIATE`, faz replay idempotente e compara contagens de agregados/versões, eventos,
checkpoints, reservas e admissions nas scopes daquele arquivo. A operação falha
se PostgreSQL não estiver disponível ou houver conflito/divergência. O chamador
deve pausar writers para obter uma comparação estável; não há snapshot atômico
entre SQLite e PostgreSQL. As APIs estão expostas nos bindings Python e
TypeScript.

Verificação desta etapa: o teste service-real apagou as tabelas da scope isolada
e confirmou reconstrução dos dois eventos, checkpoint, reservation/admission e reserva standalone, com igualdade das cinco contagens no PostgreSQL.
`cargo fmt --check` e Clippy do crate com `-D warnings` passaram. Na suíte
`storage::`, 81 testes passaram e 3 testes NATS falharam porque o contêiner local
é `nats:2.10-alpine`; o contrato de TTL requer 2.11+. Os testes PostgreSQL e os
demais serviços, incluindo Redis e Qdrant, passaram. Python: 53 testes passaram. TypeScript build/typecheck e pacote: 39 passaram, 10 testes de serviços foram ignorados por ausência dos endpoints de teste.

A outbox atende indisponibilidade/reconexão, mas confirmação remota remove a
linha. Se o PostgreSQL for perdido, o SQLite mantém a fonte completa. A decisão
de brunosrz no chat adota backup integral `pg_dump`/gerenciado e rebuild/verify
pela Engine, sem formato próprio de backup. O rebuild re-enfileira e reproduz
idempotentemente a scope local e compara cinco contagens: agregados/versões,
eventos, checkpoints, reservas e admissions. A operação exige pausar writers;
não há snapshot atômico entre os bancos. Testes service-real cobrem reconstrução,
linha extra e conteúdo divergente.

### Integridade da projeção

Implementada e integrada em `main`.

Dois testes PostgreSQL reais verificam que `rebuild_projection` falha com `ProjectionMismatch` se houver uma linha de versão extra na scope e com `ProjectionConflict` se o payload de um evento projetado tiver sido alterado. A outbox local permanece pendente no conflito. Os três testes de rebuild (reconstrução completa e duas rejeições) passaram; Clippy passou.

### Concorrência entre projetores

Teste service-real em 16 rodadas abre dois `ResilientLedger` independentes sobre o mesmo arquivo SQLite e drena em paralelo dois eventos sequenciais na mesma scope/agregado. Cada evento aparece uma vez no PostgreSQL, não há erro nem outbox restante. Isso cobre handles/conexões independentes no mesmo processo; concorrência entre processos e admissions disputadas ainda não foram medidas. Clippy passou.

## Exposição de credenciais na configuração vetorial

`VectorStoreConfig` continha `api_key` e derivava `Debug`, o que expunha a
chave quando a configuração era formatada. A correção implementa `Debug`
manual e redige tanto a chave quanto o endpoint completo, pois este também pode
conter credenciais em userinfo ou query string. Um teste verifica que nenhum
desses valores aparece na saída formatada.
