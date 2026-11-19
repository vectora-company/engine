# Análise: eventos versionados, conversas e replay

**Atualizada em:** 2026-11-17
**Plano:** [`docs/plans/conversation-events.md`](../plans/conversation-events.md)
**Spec:** [`docs/specs/04-conversas-streaming-e-threads.md`](../specs/04-conversas-streaming-e-threads.md)
**Estado examinado:** código do checkout em 2026-11-18, verificado localmente em Linux (comandos na seção Evidência de verificação).

## Pergunta

Qual fatia pode avançar sem misturar eventos de provider, lifecycle de run e
uma API de conversa durável ainda inexistente?

## Evidência observada

- `harness::RunLifecycle` modela transições de run com sequência monotônica,
  mas não define um evento público com timestamp ou conversation identity
  completa.
- `storage::sqlite::LedgerEvent` já possui `event_id`, `sequence`, type e
  payload genérico por aggregate; a store controla sequência no append e o
  SQLite é a autoridade local.
- `chat::ChatEvent` é protocolo/provider-specific e não serve como envelope
  genérico de execução ou conversa.
- `chat::TurnAccumulator` agrega saída de um stream em resposta, mas não persiste
  histórico nem oferece replay de conversa.
- A spec 04 exige evento versionado, IDs correlacionados, timestamp, estado
  terminal autoritativo, admission e replay; aponta que acesso e transporte são
  adaptados pelo consumidor.

## Decisão técnica desta fatia

Adicionar `ConversationEvent` no core como valor tipado, exigindo que o produtor
forneça IDs, sequência e timestamp. Isso permite validar payload e correlação
sem inventar gerador de IDs, relógio, persistência ou autorização. Os payloads
ficam tipados e fechados; não se adiciona `serde_json::Value` arbitrário que
pudesse transportar raciocínio interno ou segredos sem classificação. A versão
desconhecida é rejeitada para não interpretar payload futuro como contrato v1.

Não há decisão vital pendente para esta fatia: as escolhas de retenção,
assinatura de cursores e política de acesso pertencem às fases de persistência e
replay e não são necessárias para o envelope em memória.

## Log durável de eventos

`SqliteLedger::append_conversation_event` valida o envelope e grava cada evento
no ledger: cada conversa é um agregado, a store atribui a sequência dentro de
uma transação imediata e o `event_id` é a chave de idempotência. Repetir o
mesmo rascunho devolve o evento gravado; reutilizar o `event_id` com conteúdo
diferente é conflito, e reutilizá-lo em outra conversa retorna
`EventIdReused`, porque o ledger garante `event_id` único por escopo. O append
entra na fila de projeção PostgreSQL somente quando a projeção está habilitada,
como os demais stores; habilitá-la depois faz backfill dos eventos existentes.

`conversation_history` lê uma página por cursor numa única transação de leitura,
de modo que `latest_sequence` e os eventos são consistentes. O cursor é vinculado
a scope, conversa e versão do contrato, mas não é assinado: o host autoriza quem
pode ler o escopo. Uma lacuna na sequência é erro no modo estrito (padrão) e
dado (`gaps`, intervalo inclusivo) no modo tolerante, que continua a leitura e
posiciona o cursor no último evento lido. Uma lacuna inicial indica eventos
anteriores ao cursor que não estão mais armazenados. A reconexão usa
`conversation_cursor_after_event(event_id)`: retoma depois do evento e recusa
IDs de outra conversa ou escopo. Os dois modos coexistem; o estrito é o padrão
recomendado porque nunca entrega história com buraco sem o consumidor pedir.

## Eventos derivados do run

`SqliteRunLifecycleStore::sync_conversation_events` lê o histórico durável do
run (admissão, transições e verificação) e acrescenta à conversa os eventos que
faltam. Cada `event_id` deriva do run e da sequência do ledger; a sincronização
não depende de relógio nem de estado em memória, e uma segunda chamada, mesmo
após reabrir o store, não acrescenta nada. Gravar o evento de conversa na mesma
transação da transição exigiria coordenar três agregados num único append e fica
para a etapa do coordenador de execução. Decisão de mapeamento: `RecoveryRequired`
é progresso, não terminal, porque o run pode retomar; assim cada run tem um único
`Terminal`, e o log recusa eventos do run depois dele.

## Limites

O log não prova redaction, entrega ao consumidor, retenção nem retomada de
stream. `timestamp_ms` é do produtor e não substitui a sequência da store. Um
payload acima de 1 MiB é recusado pelo ledger; a referência de artifact é uma
decisão de contrato pendente. Ainda faltam política de retenção (trim) com erro
dedicado de cursor expirado, subscriber e admission.

## Evidência de verificação (2026-11-18, Linux, checkout local)

- `cargo fmt --all -- --check` e `cargo clippy --workspace --all-targets
--all-features --locked -- -D warnings`: passaram.
- `cargo test -p vectora-engine conversation` (SHA desta branch): 11 passaram,
  incluindo leitura tolerante com lacunas e retomada por `event_id`, além de
  sequência e idempotência, rascunhos inválidos, paginação sem alterar estado,
  vínculo do cursor, lacunas, reabertura com projeção, 8 escritores
  concorrentes e terminal único por run.
- Python: `pytest -k conversation` (4 passaram) e `ruff`; Node: `pnpm build`,
  `typecheck` e `node --test` (85 testes: 67 passaram, 0 falharam, 18 não
  executados por falta de serviços reais).
- `harness::events` (mapeamento do run, 1 teste) e `storage::sqlite_run`
  (espelho idempotente com reabertura e `RecoveryRequired`, 2 testes): passaram.
- `cargo test -p vectora-engine --lib --locked harness::events`: 6 passaram;
  `cargo test --locked --test conformance_fixtures conversation`: 1 passou
  (fixture compartilhada).
- Python: `maturin develop` e `pytest -k "conversation or mirrored"`: 3 passaram;
  `ruff check` e `ruff format --check` passaram.
- Node: `pnpm build` e `node --test --test-name-pattern "conversation|mirrored"`:
  3 passaram.

## Decisão de contrato pendente: admission sob concorrência

O store existente (`SqliteRunLifecycleStore::admit`) reserva thread/turn/run e
persiste o evento de admission e o lifecycle inicial atomicamente no ledger
SQLite. Repetições idênticas são idempotentes e conflitos de identidade ou
payload são rejeitados. Ele não modela `conversation_id`, capacidade ativa por
conversa, fila de turnos ou estado `blocked`. A projeção PostgreSQL não deve
ser a autoridade para essa decisão; a cópia SQLite continua local e primária.

Há duas estratégias de saturação que podem coexistir numa policy por consumer:

| Estratégia                   | Resultado ao atingir limite                                                                          | Vantagem                                                        | Custo/risco                                                                             |
| ---------------------------- | ---------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Fila FIFO durável e limitada | `queued`, com promoção em ordem quando liberar capacidade; fila cheia retorna resultado explícito    | Preserva submissões e permite ao consumer confirmar recebimento | Exige limite, política de expiração/cancelamento e recuperação da fila após restart     |
| Admission imediata           | Não cria execução quando não há slot; retorna `rejected` ou mantém o pedido `blocked` conforme causa | Menos estados e o host controla retry/agendamento               | O host precisa reter e reenviar; retry mal configurado pode perder ou duplicar intenção |

Essas estratégias são compatíveis como modos configuráveis. `blocked` precisa
ter semântica própria: a análise recomenda reservá-lo para suspensão explícita
por policy/ação do host, persistida até liberação ou rejeição; saturação normal
deve usar `queued` ou `rejected`, não `blocked`. A fila deve ter limites
explícitos e uma decisão de default antes de publicar o contrato de admission.
Essa escolha muda estados persistidos e o que consumidores podem confirmar ao
usuário; por isso não será inferida apenas da implementação atual do run store.
