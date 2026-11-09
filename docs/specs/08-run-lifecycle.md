# Ciclo de vida de execução

## Estado

Experimental: contrato versionado e transições determinísticas implementados
no núcleo Rust e expostos pelos bindings Python e TypeScript/Node.

## Incluído

- Identidade genérica de execução com `run_id`, chave de idempotência,
  `thread_id`, `turn_id`, identidade/escopo opcionais, provider/model e
  capabilities declaradas pelo consumidor. A engine valida apenas formato e
  versão; não autentica o contexto nem autoriza capabilities.
- Estados planejada, em execução, aguardando aprovação/provider, executando
  tool, verificando, concluída, degradada, falha, cancelada e recuperação
  necessária.
- Validação de transições, sequência monotônica e repetição idempotente do
  estado atual. Essa repetição local não reserva a chave de idempotência nem
  garante unicidade entre processos.
- API síncrona equivalente nos bindings, encaminhada ao mesmo código Rust.

## Limites atuais

Este contrato não reserva a chave de idempotência, não persiste snapshots,
eventos ou checkpoints e não recupera execução após restart. O consumidor é
responsável por persistir o snapshot atomicamente; até existir um contrato de
store e conformidade para concorrência/restart, esta API isolada não deve ser
usada para garantir ausência de efeitos duplicados.

Também não executa tools nem implementa autorização, HITL, política, redaction,
verificação ou gates. Transições válidas descrevem o ciclo; não comprovam que
uma aprovação, tool ou verificação ocorreu. Esses recursos permanecem
indisponíveis nesta etapa.

## Próximas decisões

- Definir store transacional com reserva atômica por `thread_id`/`turn_id` e
  chave idempotente, event ledger e recuperação após restart.
- Especificar eventos duráveis, concorrência, compare-and-set, migração de
  versões, redaction e política de retenção antes de alegar retomada segura.
- Separar estado terminal da execução, resultado da verificação e veredito do
  gate no contrato de conformidade.
