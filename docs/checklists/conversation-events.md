# Checklist: eventos, conversas e replay

**Estado:** `em andamento`
**Plano:** [`docs/plans/conversation-events.md`](../plans/conversation-events.md)
**Análise:** [`docs/analyses/conversation-events.md`](../analyses/conversation-events.md)
**Spec:** [`docs/specs/04-conversas-streaming-e-threads.md`](../specs/04-conversas-streaming-e-threads.md)

## Envelope e validação

- [x] Envelope Rust inclui versão, event/conversation IDs, IDs correlacionáveis,
      sequência, timestamp e origem. Evidência: `harness::ConversationEvent`.
- [x] Payloads tipados representam turno, run, step, tool, aprovação,
      progresso, texto visível, usage e terminal. Não existe payload para raciocínio
      interno bruto.
- [x] Validação rejeita versão não suportada, campos obrigatórios vazios,
      sequência zero e correlação ausente.
- [x] Testes unitários de roundtrip, correlação e versão/sequência executados
      localmente (ver Verificação e entrega).
- [x] Bindings Python e TypeScript expõem validação pela mesma API Rust. Evidência:
      `validate_conversation_event` e `validateConversationEvent`.
- [x] Fixture comum `tests/fixtures/conversation-events-v1.json` cobre
      aceitação, rejeição e correlação nos testes Rust/Python/Node.

## Estado, persistência e replay

- [ ] Admission idempotente por scope/conversation/turn com estados explícitos;
      implementação aguarda decisão sobre saturação, limite de fila, semântica de
      `blocked` e comportamento padrão. Comparativo:
      [`docs/analyses/conversation-events.md`](../analyses/conversation-events.md).
- [ ] Concorrência configurável por conversation.
- [x] Ledger SQLite mantém eventos canônicos por conversa com sequência atribuída
      pela store, idempotência por `event_id` e fila de projeção quando habilitada.
- [ ] Payloads grandes ficam fora do ledger por referência validada.
- [x] Eventos do lifecycle do run (admissão, transições, verificação) são
      espelhados na conversa de forma idempotente; `RecoveryRequired` é progresso
      e cada run tem um único evento terminal.
- [ ] Gravar o evento de conversa na mesma transação da transição do run (hoje
      o espelho é derivado e retomável).
- [x] History paginado e cursores validados por scope, conversation e versão,
      lidos numa única transação.
- [ ] Replay reporta lacuna como dado representável e cursor expirado, e não
      altera o estado de consumers. Hoje a lacuna é reportada como erro.
- [ ] Subscriber tem buffer limitado, política de coalescência e cancelamento
      independente do run.
- [x] Reabrir o ledger preserva a sequência do log de conversa. Pendente: recovery que não repete efeitos externos a partir do lifecycle.
- [ ] Redaction e autorização de leitura são aplicadas pelo host antes da
      divulgação; limites de tamanho permanecem no contrato da Engine.

## Verificação e entrega

- [x] `cargo fmt --all -- --check` e `cargo clippy --workspace --all-targets
--all-features --locked -- -D warnings` passaram (2026-11-18, Linux).
- [x] Testes Rust: 8 de `storage::sqlite::conversation`, 1 de mapeamento em
      `harness::events` e 2 de `storage::sqlite_run` (espelho do run) passaram.
      Envelope e fixture compartilhada: ver análise.
- [x] Testes Python (`pytest -k "conversation or mirrored"`, 3 testes,
      `maturin develop`) e Node (`--test-name-pattern "conversation|mirrored"`, 3
      testes, `pnpm build`) passaram.
- [ ] Fixture comum Rust/Python/Node para o log durável.
- [ ] Verificações com subscriber e replay reais quando forem implementados.
- [ ] Diff revisado e documentação sincronizada a cada etapa.
