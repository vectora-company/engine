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
- [x] Testes unitários adicionados para roundtrip, correlação e versão/sequência;
  não executados localmente nesta etapa por instrução do mantenedor.
- [ ] Bindings Python e TypeScript expõem a mesma API sobre o core Rust.
- [ ] Fixtures comuns verificam aceitação, rejeição e serialização nos três níveis.

## Estado, persistência e replay

- [ ] Admission idempotente por scope/conversation/turn com estados explícitos.
- [ ] Concorrência configurável por conversation.
- [ ] Ledger SQLite mantém eventos canônicos e índices para histórico; payloads
  grandes ficam fora do ledger por referência validada.
- [ ] History paginado e cursores validados por scope, conversation e versão.
- [ ] Replay reporta cursor expirado/gaps e não altera o estado de consumers.
- [ ] Subscriber tem buffer limitado, política de coalescência e cancelamento
  independente do run.
- [ ] Restart/recovery preserva sequência e não repete efeitos externos.
- [ ] Redaction e autorização de leitura são aplicadas pelo host antes da
  divulgação; limites de tamanho permanecem no contrato da Engine.

## Verificação e entrega

- [ ] `cargo fmt --all -- --check`.
- [ ] Testes Rust do envelope e das fixtures compartilhadas.
- [ ] Testes Python e Node de conformidade.
- [ ] Verificações de storage/replay reais quando o adapter for implementado.
- [ ] Sem execução local de testes nesta etapa por solicitação do mantenedor;
  aguardar evidência do CI no SHA publicado.
- [ ] PR empilhado, diff revisado e documentação sincronizada.
