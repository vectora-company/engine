# Análise: eventos versionados, conversas e replay

**Atualizada em:** 2026-11-17
**Plano:** [`docs/plans/conversation-events.md`](../plans/conversation-events.md)
**Spec:** [`docs/specs/04-conversas-streaming-e-threads.md`](../specs/04-conversas-streaming-e-threads.md)
**Checkout examinado:** `/home/brunosrz/Área de trabalho/engine`, branch `feat/conversation-event-bindings`, sobre `feat/versioned-conversation-events` (PR #158)

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

## Limites

O binding Python e o binding TypeScript/Node encaminham a validação ao mesmo
core Rust; uma fixture compartilhada cobre eventos válidos e inválidos nos
testes Rust, Python e Node. Os testes estão escritos, mas não foram executados
localmente por solicitação do mantenedor. Os checks remotos desta nova etapa
precisam ser consultados no SHA publicado antes de declarar conformidade.

A validação não prova persistência, ordenação atribuída pela store, redaction
ou replay. A etapa permanece parcial até os checks publicados passarem e a
integração durável com admission/ledger ser implementada.

## Decisão de contrato pendente: admission sob concorrência

O store existente (`SqliteRunLifecycleStore::admit`) reserva thread/turn/run e
persiste o evento de admission e o lifecycle inicial atomicamente no ledger
SQLite. Repetições idênticas são idempotentes e conflitos de identidade ou
payload são rejeitados. Ele não modela `conversation_id`, capacidade ativa por
conversa, fila de turnos ou estado `blocked`. A projeção PostgreSQL não deve
ser a autoridade para essa decisão; a cópia SQLite continua local e primária.

Há duas estratégias de saturação que podem coexistir numa policy por consumer:

| Estratégia | Resultado ao atingir limite | Vantagem | Custo/risco |
| --- | --- | --- | --- |
| Fila FIFO durável e limitada | `queued`, com promoção em ordem quando liberar capacidade; fila cheia retorna resultado explícito | Preserva submissões e permite ao consumer confirmar recebimento | Exige limite, política de expiração/cancelamento e recuperação da fila após restart |
| Admission imediata | Não cria execução quando não há slot; retorna `rejected` ou mantém o pedido `blocked` conforme causa | Menos estados e o host controla retry/agendamento | O host precisa reter e reenviar; retry mal configurado pode perder ou duplicar intenção |

Essas estratégias são compatíveis como modos configuráveis. `blocked` precisa
ter semântica própria: a análise recomenda reservá-lo para suspensão explícita
por policy/ação do host, persistida até liberação ou rejeição; saturação normal
deve usar `queued` ou `rejected`, não `blocked`. A fila deve ter limites
explícitos e uma decisão de default antes de publicar o contrato de admission.
Essa escolha muda estados persistidos e o que consumidores podem confirmar ao
usuário; por isso não será inferida apenas da implementação atual do run store.
