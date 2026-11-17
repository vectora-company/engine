# Análise: eventos versionados, conversas e replay

**Atualizada em:** 2026-11-17
**Plano:** [`docs/plans/conversation-events.md`](../plans/conversation-events.md)
**Spec:** [`docs/specs/04-conversas-streaming-e-threads.md`](../specs/04-conversas-streaming-e-threads.md)
**Checkout examinado:** `/home/brunosrz/Área de trabalho/engine`, branch `feat/versioned-conversation-events`, antes do commit

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

Os testes unitários estão no código, mas não foram executados localmente por
solicitação do mantenedor. A validação não prova serialização cross-language,
persistência, ordenação real, redaction ou replay. A entrega deve permanecer
marcada parcial até bindings e integração com ledger terem checks verdes.
