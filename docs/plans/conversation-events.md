# Plano: eventos versionados, conversas e replay

**Estado:** `em andamento`: envelope e log durável verificados localmente; admission, subscriber e artifact pendentes
**Atualizado em:** 2026-11-18
**Spec:** [`docs/specs/04-conversas-streaming-e-threads.md`](../specs/04-conversas-streaming-e-threads.md)
**Análise:** [`docs/analyses/conversation-events.md`](../analyses/conversation-events.md)
**Checklist:** [`docs/checklists/conversation-events.md`](../checklists/conversation-events.md)

## Objetivo

Dar aos hosts uma API independente de transporte para representar eventos
ordenados de conversas e turnos, com admission, histórico e recuperação por
cursor. A Engine fornece os contratos e estado durável; o consumidor continua
responsável por autenticar principals, adaptar canais e decidir quais dados
pode divulgar.

## Estado observado

| Camada                           | Estado                               | Evidência                                                                                                                                                                                                                | Lacuna                                                                           |
| -------------------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------- |
| Core Rust: envelope              | Implementado e verificado localmente | `crates/vectora-engine/src/harness/events.rs`: envelope v1, payloads tipados e validação de correlação                                                                                                                   | Redaction e integração com o lifecycle                                           |
| Core Rust: log durável           | Implementado e verificado localmente | `crates/vectora-engine/src/storage/sqlite/conversation.rs`: sequência atribuída pela store, idempotência por `event_id`, histórico paginado por cursor, lacunas como erro, outbox de projeção só com projeção habilitada | Lacuna como dado representável, retenção e cursor expirado, replay com reconexão |
| Python                           | Implementado e verificado localmente | `validate_conversation_event`, `sqlite_conversation_append_event`, `sqlite_conversation_history`                                                                                                                         | Fixture comum do store                                                           |
| TypeScript/Node                  | Implementado e verificado localmente | `validateConversationEvent`, `sqliteConversationAppendEvent`, `sqliteConversationHistory`                                                                                                                                | Fixture comum do store                                                           |
| Admission e projeção de timeline | Pendente                             | Lifecycle store reserva run/thread/turn                                                                                                                                                                                  | Admission por conversa, listagem de conversas e projeção de timeline             |
| Replay/stream subscriber         | Pendente                             | Streams atuais representam respostas de providers                                                                                                                                                                        | Replay de eventos duráveis, gaps, backpressure e cancelamento independente       |

## Entregas ordenadas

1. [x] Definir envelope Rust versionado e payloads tipados mínimos; validar
       IDs, versão, sequência e correlação. Testes unitários foram adicionados,
       mas não executados localmente nesta etapa a pedido do mantenedor.
2. [parcial] Expor o contrato por Python e TypeScript/Node e adicionar fixture
   comum Rust/Python/Node em `tests/fixtures/conversation-events-v1.json`.
   Testes de conformidade estão no código, mas não foram executados localmente;
   os checks remotos ainda precisam concluir.
3. [bloqueado por decisão] Associar admission idempotente a
   `conversation_id`/`turn_id`; fechar estratégia de saturação, limites de fila,
   significado de `blocked` e modo padrão. Comparativo em
   [`docs/analyses/conversation-events.md`](../analyses/conversation-events.md).
4. [parcial] Persistir eventos ordenados no ledger SQLite. Feito: sequência
   atribuída pela store em transação imediata, idempotência por `event_id`,
   erro tipado para `event_id` reutilizado em outra conversa e fila de projeção
   quando habilitada. Pendente: payloads grandes por referência fora do ledger
   e emissão de eventos a partir do lifecycle do run sem duplicar efeito.
5. [parcial] Consulta de histórico paginado com cursor vinculado a scope,
   conversa e versão, em uma única transação de leitura. Feito: cursor
   adiantado, de outro escopo ou de versão desconhecida é recusado; lacunas
   são reportadas como erro. Pendente: lacuna como dado representável e
   política de retenção com cursor expirado.
6. [ ] Implementar subscriber com backpressure/coalescência e cancelamento da
       assinatura separado do cancelamento do run.
7. [ ] Integrar replay/recovery e testar restart, duplicatas, cursor inválido,
       concorrência e cancelamento nos três níveis.

## Limites da entrega atual

O envelope é um valor em memória. Não comprova autenticidade do produtor,
autorização de leitura, redaction, durabilidade, ordenação entre processos,
entrega ao consumidor ou retomada após reinício. O `timestamp_ms` é fornecido
pelo produtor e não substitui a sequência autoritativa da store. Artifact
references e políticas de expiração permanecem fora desta primeira fatia.
Inteiros de sequência, timestamp e usage são limitados ao intervalo seguro de
JSON/JavaScript para que os bindings mantenham o mesmo valor sem arredondamento.

## Critérios de saída

- Rust, Python e TypeScript aceitam/rejeitam os mesmos fixtures de evento.
- Store gera sequência monotônica e deduplica por ID/idempotency key no scope.
- History/replay preservam ordem, reportam gaps e não repetem efeitos.
- Cancelamento de subscriber não cancela run; cancelamento de run exige ação
  autorizada e produz estado terminal explícito.
- Tamanho, redaction e referências de artifacts limitam payloads sensíveis ou
  grandes.
- Plano, análise e checklist registram as verificações por ambiente e SHA.

## Próxima etapa

Emitir eventos de conversa a partir das transições do lifecycle do run, na mesma
transação do ledger, com `event_id` derivado do run e da sequência e um único
evento terminal por turno. Em seguida: replay com lacunas representáveis,
subscriber com backpressure e admission.

Admission tem dois modos que coexistem: fila FIFO durável limitada e admission
imediata com `rejected`; `blocked` fica reservado à suspensão explícita por
policy ou ação do host. O modo padrão recomendado é a fila limitada; o padrão e
os limites aguardam a confirmação de `brunosrz` no chat. SQLite permanece a
autoridade local e PostgreSQL recebe somente a projeção.
