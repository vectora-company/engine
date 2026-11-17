# Plano: loop agêntico e harness

## Objetivo

Implementar um runtime compartilhado, canônico em Rust, para lifecycle de runs,
montagem de turnos, dispatch de tools, policy, persistência e verificação.
Backend Python do Vectora e documentação de requisitos são fontes de
comportamento, não autorização para migrar consumidores em bloco. UI, RBAC,
identidade, execução de adapters e storage de produto continuam no host.

## Estado e fases

1. **Lifecycle e montagem de turno — implementados experimentalmente.**
   Lifecycle versionado e acumulação de eventos/streams existem em Rust e nos
   bindings. Adoção por consumidor e conformidade integral permanecem gates.
2. **Tools e dispatch — implementados experimentalmente.** Schema, capabilities,
   erros, decisões default-deny e coordenação de rodadas modelo → tool → modelo
   existem. O host executa e isola adapters; timeout/idempotência declarados
   não são impostos ao processo externo pela Engine.
3. **Policy e aprovação — implementadas com persistência SQLite.** Snapshot,
   requests e receipts são validados e persistidos. O host autentica aprovador,
   verifica a autorização e executa/cessa a tool.
4. **Persistência e recovery — parcial.** Stores SQLite para lifecycle,
   model/tool loop e policy gravam estado transacionalmente e permitem reopen,
   retry idempotente, cancelamento pendente e reconciliação explícita. Efeitos
   externos não são atômicos nem exactly-once. PostgreSQL é projeção; SQLite
   permanece autoridade local e fonte de rebuild. Migrations versionadas com
   rollback e recovery operacional abrangente continuam no plano de storage.
5. **Verification gates — implementados experimentalmente.** Reports/gates
   tipados são associados à conclusão. A Engine persiste evidência recebida;
   o host executa e autentica os checks, cuja veracidade não é inferida.

As specs de referência são
[`01-runtime-agentico-e-harness.md`](../specs/01-runtime-agentico-e-harness.md),
[`10-tool-contracts-e-dispatch.md`](../specs/10-tool-contracts-e-dispatch.md),
[`11-model-tool-loop.md`](../specs/11-model-tool-loop.md),
[`12-policy-and-approval.md`](../specs/12-policy-and-approval.md),
[`13-sqlite-model-tool-loop-store.md`](../specs/13-sqlite-model-tool-loop-store.md),
[`14-sqlite-durable-run-lifecycle.md`](../specs/14-sqlite-durable-run-lifecycle.md)
e [`15-verification-gates.md`](../specs/15-verification-gates.md).

## Gaps e critérios restantes

- [ ] Fechar integração de stores com projeção PostgreSQL e migrations com
      rollback, mantendo SQLite como autoridade e cópia de recovery.
- [ ] Ampliar cobertura de concorrência multiprocesso em reservas/admissions,
      retries, recovery e projeção.
- [ ] Validar shutdown/cancelamento através de adapters reais; resultado incerto
      continua exigindo reconciliação antes de avançar.
- [ ] Definir conformidade compartilhada de redaction, estados e erros entre
      Rust, Python e TypeScript.
- [ ] Validar adoção reversível no Vectora e GH Bot sem execução duplicada de
      efeitos.
- [ ] Manter sandbox, RBAC e identidade do aprovador como responsabilidades do
      consumidor até existir contrato aprovado.

## Evidência de referência do consumidor

`conversation_loop.py`, `tool_batch.py`, `guardrails.py`, `hitl.py`,
`stream_events.py`, `decision_record.py` e `native_stream.py` ajudam a
caracterizar semântica de streaming, orçamento, policy, identidade, recovery e
serialização. Não devem ser copiados como framework único nem importados pela
Engine.
