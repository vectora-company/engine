# Matriz de conformidade das capabilities

**Base de código auditada:** `main` em `22c861f`
**Data da auditoria:** 11 de novembro de 2026
**Plano:** [`03-plano-continuidade.md`](../plans/03-plano-continuidade.md)
**Checklist:** [`02-matriz-conformidade-capabilities.md`](../checklists/02-matriz-conformidade-capabilities.md)

Esta matriz distingue código, binding, testes existentes e maturidade. A
presença de uma função ou teste não prova conformidade integral nem uso por um
consumer. Os dados abaixo foram conferidos em `main`; alterações em branches
abertas são listadas separadamente e não são atribuídas à base.

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

| Capability e contrato                                                           | Rust                                                                                    | Python                                            | TypeScript/Node                                 | Evidência de teste em `main`                                                                            | Maturidade e limite observado                                                                                                                                                                                                                           |
| ------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | ------------------------------------------------- | ----------------------------------------------- | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Chat comum, mensagens, completion e eventos — `CHAT_CONTRACT_VERSION=1`         | `chat/types.rs`, `chat/openai_compatible.rs`                                            | `chat_complete`, `chat_stream`                    | `chatComplete`, `chatStream`                    | Rust `chat/*`; Python/TS têm testes locais de protocolo                                                 | **Implementada experimentalmente.** Transporte OpenAI-compatible e SSE. Revisões de redirects, headers e campos nulos presentes na branch de correção não fazem parte do SHA auditado.                                                                  |
| Providers nativos Anthropic, Google e Cohere — contrato de chat v1              | `chat/native_providers.rs`                                                              | `native_chat_complete`, `native_chat_stream`      | `nativeChatComplete`, `nativeChatStream`        | Conversão request/response/stream nos três níveis de teste                                              | **Implementada experimentalmente.** Sem catálogo, descoberta de credenciais, billing ou policy de produto.                                                                                                                                              |
| Fallback de chat                                                                | `chat/fallback.rs`                                                                      | `fallback_chat_complete`, `fallback_chat_stream`  | `fallbackChatComplete`, `fallbackChatStream`    | Fixture comum de completion, stream e interrupção da iteração nos três bindings; testes locais passaram | **Implementada experimentalmente.** Candidatos são explícitos e fornecidos pelo host; não há decisão automática de provider. Interromper a leitura do stream não comprova tempo de cancelamento do socket upstream.                                     |
| Montagem de turno streaming — chat v1                                           | `chat/turn.rs::assemble_turn`                                                           | `assemble_turn`                                   | `assembleTurn`                                  | Testes Rust e casos de binding para fragmentos, limites e evento terminal                               | **Implementada experimentalmente.** Acumula eventos em completion; não é armazenamento de conversa, sessão ou replay.                                                                                                                                   |
| Embeddings — `EMBEDDING_CONTRACT_VERSION=1`                                     | `llm/embeddings.rs`                                                                     | `embed`, `fallback_embed`                         | `embed`, `fallbackEmbed`                        | Adaptadores de protocolo e fallback local em Python/TS, testes unitários Rust                           | **Implementada experimentalmente.** Fallback e limites de cada provider precisam manter cobertura por protocolo. Não há pipeline de indexação ou memória.                                                                                               |
| Rerank — `RERANK_CONTRACT_VERSION=1`                                            | `llm/rerank.rs`                                                                         | `rerank`, `fallback_rerank`                       | `rerank`, `fallbackRerank`                      | Protocolo, ordenação e respostas vazias cobertos localmente                                             | **Implementada experimentalmente.** Não define recuperação ou RAG por si só.                                                                                                                                                                            |
| Identidade e máquina de estados de run — `RUN_CONTRACT_VERSION=1`               | `harness.rs`                                                                            | `create_run`, `transition_run`                    | `createRun`, `transitionRun`                    | Transições válidas/inválidas no core e bindings                                                         | **Implementada experimentalmente.** Máquina de estados pura; não autentica a identidade fornecida pelo host.                                                                                                                                            |
| Contrato e preparo de tools — `TOOL_CONTRACT_VERSION=1`, dispatch v1            | `harness/tool_dispatch.rs`                                                              | `prepare_tool_dispatch`, `tool_chat_definitions`  | `prepareToolDispatch`, `toolChatDefinitions`    | Schema, argumentos, run ativo e default-deny cobertos localmente                                        | **Parcial.** Valida e prepara dispatch, mas não descobre nem executa adapters, não aplica sandbox e não prova que o host aplicou timeout/capability no efeito.                                                                                          |
| Policy e aprovação — `TOOL_POLICY_CONTRACT_VERSION=1`                           | `harness/policy.rs`                                                                     | `authorize_tool_dispatch`, versões e serialização | `authorizeToolDispatch`, versões e serialização | Binding e testes de aprovação vinculada à chamada                                                       | **Implementada experimentalmente.** Engine valida o vínculo; host autentica aprovador e recibo, recompila policy e executa a tool.                                                                                                                      |
| Gates de verificação — `EVIDENCE_GATE_CONTRACT_VERSION=1`                       | `harness/verification.rs`                                                               | `evaluate_verification`                           | `evaluateVerification`                          | Precedência, evidência expirada/redação e veredito separados nos bindings                               | **Implementada experimentalmente.** SQLite persiste report/avaliação em stores específicos; engine não executa checks nem valida a veracidade de `redacted`.                                                                                            |
| Loop modelo/tool — `MODEL_TOOL_LOOP_CONTRACT_VERSION=2`, resultado de tool v1   | `harness/model_tool_loop.rs`                                                            | start/accept/submit/recover/cancel/reconcile      | APIs equivalentes camelCase                     | Rust e bindings cobrem transições, cancelamento e reconciliação                                         | **Implementada experimentalmente.** Estado de coordenação, não execução de adapter; resultado de efeito incerto requer decisão do host.                                                                                                                 |
| Ledger append-only e reserva em SQLite; `CHECKPOINT_VERSION=1`                  | `storage/sqlite.rs`                                                                     | append/read/reserve/checkpoint/admit              | APIs `sqlite*` correspondentes                  | Rust, Python e TS exercitam idempotência, reopen, conflito e checkpoint                                 | **Implementada experimentalmente.** API é ledger de eventos/checkpoints, não interface SQL genérica nem schema de produto. Escopo e redação são responsabilidade do host.                                                                               |
| Lifecycle durável de run — store SQLite v1                                      | `storage/sqlite_run.rs`                                                                 | `sqlite_run_*`                                    | `sqliteRun*`                                    | Admission/transição/verificação/reopen testados nos bindings                                            | **Implementada experimentalmente.** Persistência específica SQLite; não existe adapter PostgreSQL equivalente para lifecycle.                                                                                                                           |
| Persistência de loop e verificação — store SQLite v2                            | `storage/sqlite_loop.rs`                                                                | `sqlite_model_tool_loop_*`                        | `sqliteModelToolLoop*`                          | Restart, recovery, cancelamento e verificação persistida testados                                       | **Implementada experimentalmente.** Snapshot v1 é migrado/leitura limitada; não coordena sozinho processos do host nem effects externos.                                                                                                                |
| Persistência de policy/aprovação — store SQLite v1                              | `storage/sqlite_policy.rs`                                                              | `sqlite_tool_policy_*`                            | `sqliteToolPolicy*`                             | Reopen, stale state, binding e receipts cobertos nos bindings                                           | **Implementada experimentalmente.** Não autentica aprovador e não é RBAC compartilhado entre produtos.                                                                                                                                                  |
| Ledger PostgreSQL e projeção resiliente SQLite-first                            | `storage/postgres.rs`, `storage/resilient_ledger.rs`                                    | `resilient_ledger_append`                         | `resilientLedgerAppend`                         | Testes locais de projeção, retry e conflito em Rust; bindings expõem operação                           | **Parcial/experimental.** PostgreSQL é projeção do ledger local com outbox; não há adapter genérico de SQL nem substituição de SQLite como autoridade do append resiliente. Operação de serviço real e recuperação sob carga precisam de prova própria. |
| Seleção independente de storage — `STORAGE_CONTRACT_VERSION=1`                  | `storage.rs`                                                                            | `validate_storage_selection`                      | `validateStorageSelection`                      | Serialização e versão testadas                                                                          | **Parcial.** Seleção representa SQL/vector/KV/queue/pub-sub, mas não significa que cada opção selecionada seja conectável ou tenha garantias equivalentes.                                                                                              |
| Busca vetorial — `VECTOR_CONTRACT_VERSION=1`                                    | `storage/vector.rs`: LanceDB e Qdrant                                                   | `vector_store_upsert/search/delete`               | `vectorStoreUpsert/Search/Delete`               | Unidade Rust e wrappers cobertos; matrix de serviços reais não demonstrada                              | **Implementada experimentalmente.** LanceDB/Qdrant têm semânticas distintas de score/filtros; não existe contrato de memória ou indexação documental completo.                                                                                          |
| KV — messaging v1                                                               | `storage/messaging.rs`: Redis e NATS JetStream                                          | `kv_store_put/get`                                | `kvStorePut/Get`                                | Sem suite comum de conformidade por backend/serviço real                                                | **Parcial.** Redis aceita TTL; JetStream não implementa TTL idêntico. A seleção pública rejeita `memory`; doubles em memória são internos a testes.                                                                                                     |
| Fila durável — messaging v1                                                     | `DurableQueue`: Redis Streams e NATS JetStream                                          | `queue_*`                                         | `queue*`                                        | Sem suite comum cobrindo falhas de serviço e redelivery para ambos                                      | **Implementada experimentalmente.** ACK/release, grupo, redelivery e retenção variam por backend; handles pertencem ao processo. A seleção pública rejeita `memory`.                                                                                    |
| Pub/sub efêmero — messaging v1                                                  | `EphemeralPubSub`: Redis e NATS Core                                                    | `pubsub_*`                                        | `pubsub*`                                       | Cobertura de wrappers, sem comprovação de serviço real compartilhada                                    | **Implementada experimentalmente.** Não é durável; mensagens podem se perder sem assinante/reconexão. A seleção pública rejeita `memory`.                                                                                                               |
| Memória, Context Planner, compactação/context engineering e RAG                 | Não há módulo/API de memory/context/RAG no core atual                                   | Ausente                                           | Ausente                                         | Ausente                                                                                                 | **Planejada.** Embedding, vector store e ledger são primitivas; não equivalem a esses sistemas.                                                                                                                                                         |
| Conversa/sessão, timeline de eventos e replay de stream/canal                   | Eventos de chat e `run_id/thread_id` existem; não há store/API geral de sessão e replay | Ausente como API de conversas                     | Ausente como API de conversas                   | Sem conformidade de replay/cursor/canal                                                                 | **Parcial/planejada.** O ledger e a montagem de turno não constituem contrato de conversa, canal, subscriber ou retomada de stream.                                                                                                                     |
| Subagentes, workflow/orquestração multiagente, scheduler, skills, hooks/plugins | Sem API correspondente                                                                  | Ausente                                           | Ausente                                         | Ausente                                                                                                 | **Planejada.** Não inferir suporte a partir de tipos de run, dispatch ou loop modelo/tool.                                                                                                                                                              |
| Sandbox, hooks de egress, permission runtime, execução de ferramentas           | Não há execução isolada de tools no core                                                | Ausente                                           | Ausente                                         | Ausente                                                                                                 | **Planejada/fora do core até contrato definido.** A policy é um gate de autorização; o host ainda executa e isola adapters.                                                                                                                             |

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

## Estado de branches de revisão

Esta fotografia não atribui mudanças não merged a `main`. A branch
`fix/openai-compat-review` contém correções para redirects, validação do header
de API key, serialização de opcionais e independência do typecheck; veja
[`01-auditoria-review-pr3.md`](01-auditoria-review-pr3.md). Elas precisam ser
reavaliadas contra a base após merge, sem usar o resultado de CI remoto como
substituto de auditoria local.

## Fixture de conformidade em desenvolvimento

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
A fixture não existia no SHA `main` auditado acima; resultados locais ficam no
checklist.

### Cobertura service-real observada

Há testes condicionais de PostgreSQL (ledger e lifecycle/checkpoint), projeção
SQLite-first, Qdrant (isolamento por scope e upsert/search/delete), Redis
(KV/TTL, pub/sub e redelivery de Redis Streams) e NATS (KV, pub/sub e
redelivery de JetStream). Eles não iniciam os serviços; ausência da variável de
ambiente correspondente faz o teste retornar sem executar as asserções de
serviço. Não foi encontrada cobertura integrada de indisponibilidade/reconnect,
restart, retenção e falhas combinadas nos quatro serviços. Esse inventário
prepara a auditoria S0–S5; não substitui essa auditoria nem demonstra paridade
operacional.
