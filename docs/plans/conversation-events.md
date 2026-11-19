# Plano: eventos versionados, conversas e replay

**Estado:** `em andamento`
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
| Python | Pendente | Sem API de eventos de conversa | Tipos, validação e fixtures do core |
| TypeScript/Node | Pendente | Sem API de eventos de conversa | Tipos, validação e fixtures do core |
| Conversation/Turn store | Pendente | Lifecycle store reserva run/thread/turn | Admission genérica, histórico paginado e projeção de timeline |
| Replay/stream subscriber | Pendente | Streams atuais representam respostas de providers | Replay de eventos duráveis, gaps, backpressure e cancelamento independente |

## Entregas ordenadas

1. [x] Definir envelope Rust versionado e payloads tipados mínimos; validar
   IDs, versão, sequência e correlação. Testes unitários foram adicionados,
   mas não executados localmente nesta etapa a pedido do mantenedor.
2. [ ] Expor o mesmo contrato por Python e TypeScript/Node e estabelecer
   fixtures de conformidade compartilhadas.
3. [ ] Associar admission idempotente a `conversation_id`/`turn_id`; definir
   respostas accepted/queued/blocked/rejected e concorrência por conversation.
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

Expor o tipo no binding Python e no binding TypeScript com a mesma validação
Rust e fixtures equivalentes. Isso não depende de escolher retenção, transporte
ou política de acesso a artifacts.
