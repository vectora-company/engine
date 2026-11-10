# Ciclo de vida de execução

## Estado

Experimental: contrato versionado e transições determinísticas implementados
no núcleo Rust e expostos pelos bindings Python e TypeScript/Node. O adapter
SQLite `SqliteRunLifecycleStore` adiciona admissão e snapshots duráveis; veja
[`14-sqlite-durable-run-lifecycle.md`](14-sqlite-durable-run-lifecycle.md).

## Incluído

- Identidade genérica de execução com `run_id`, chave de idempotência,
  `thread_id`, `turn_id`, identidade/escopo opcionais, provider/model e
  capabilities declaradas pelo consumidor. A engine valida apenas formato e
  versão; não autentica o contexto nem autoriza capabilities.
- Estados planejada, em execução, aguardando aprovação/provider, executando
  tool, verificando, concluída, degradada, falha, cancelada e recuperação
  necessária.
- Validação de transições, sequência monotônica e repetição idempotente do
  estado atual.
- No adapter SQLite, reserva idempotente por scope/thread/turn, evento de
  admissão na thread e estado/checkpoint inicial do run dentro da mesma
  transação.
- Persistência das transições de lifecycle com compare-and-set pela sequência
  do ledger, detecção de snapshots obsoletos e recuperação do último checkpoint
  após reabrir a store.
- API síncrona equivalente nos bindings, encaminhada ao mesmo código Rust.

## Limites atuais

O contrato de lifecycle em memória não reserva chaves nem persiste snapshots.
O adapter SQLite cobre apenas a admissão e o estado do lifecycle. Ele não
coordena o loop completo de modelo/tools, nem persiste policy e concessões, e
não recupera efeitos externos de resultado incerto.

Também não executa tools nem implementa autorização, HITL, política, redaction,
verificação ou gates. Transições válidas descrevem o ciclo; não comprovam que
uma aprovação, tool ou verificação ocorreu. Esses recursos permanecem
indisponíveis nesta etapa.

## Próximas decisões

- Definir contratos e adapters equivalentes para PostgreSQL e para a integração
  do lifecycle com o loop de modelo/tools.
- Especificar eventos duráveis, concorrência, compare-and-set, migração de
  versões, redaction e política de retenção antes de alegar retomada segura.
- Separar estado terminal da execução, resultado da verificação e veredito do
  gate no contrato de conformidade.

## Checkpoints atômicos

O adapter SQLite também expõe `append_checkpointed`: evento e snapshot
versionado são gravados na mesma transação. Consumers devem gravar o estado
`effect_started` e a invocação pendente antes de chamar um adapter externo e
gravar o resultado em novo evento/checkpoint depois. Se o processo parar entre
essas gravações, o snapshot precisa manter a invocação como incerta para que o
host exija reconciliação explícita; não se deve repetir a chamada
automaticamente. Esta API atômica é experimental e não coordena por conta
própria o loop, o policy gate ou os adapters.
