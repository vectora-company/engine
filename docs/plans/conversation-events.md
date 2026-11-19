# Plano: eventos versionados, conversas e replay

**Estado:** `aguardando decisão vital de contrato para admission`
**Atualizado em:** 2026-11-17
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

| Camada | Estado | Evidência | Lacuna |
| --- | --- | --- | --- |
| Core Rust | Parcial | `crates/vectora-engine/src/harness/events.rs`: envelope v1, payloads tipados e validação de correlação | Persistência, IDs/ordem atribuídos pela store, redaction e integração lifecycle |
| Python | Parcial | `validate_conversation_event` encaminha ao validador Rust; teste lê fixture compartilhada | Verificação local/CI pendente |
| TypeScript/Node | Parcial | `validateConversationEvent` e tipos discriminados encaminham ao validador Rust; teste lê fixture compartilhada | Verificação local/CI pendente |
| Conversation/Turn store | Pendente | Lifecycle store reserva run/thread/turn | Admission genérica, histórico paginado e projeção de timeline |
| Replay/stream subscriber | Pendente | Streams atuais representam respostas de providers | Replay de eventos duráveis, gaps, backpressure e cancelamento independente |

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
4. [ ] Persistir eventos ordenados no ledger SQLite, mantendo payloads grandes
   referenciados fora do ledger; conectar lifecycle sem duplicar evento ou
   efeito.
5. [ ] Implementar consulta de histórico paginado e cursor vinculado ao scope,
   conversation e versão; reportar retenção/gaps sem ocultar eventos ausentes.
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

A etapa de bindings foi publicada no PR #159, filho do PR #158; seus checks
remotos ainda precisam ser consultados no SHA corrente e não foram aguardados.
O código independente seguinte é admission idempotente por
scope/conversation/turn. A implementação está parada antes de alterar o schema
persistido porque o modo de saturação e a semântica de `blocked` são decisões
de contrato que dependem de brunosrz no chat. SQLite permanece a autoridade
local; PostgreSQL recebe somente a projeção.
