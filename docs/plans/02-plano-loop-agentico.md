# Plano de implementação do loop agêntico

## Objetivo

Implementar na engine Rust o runtime reutilizável descrito em
[`01-runtime-agentico-e-harness.md`](../specs/01-runtime-agentico-e-harness.md),
usando `../issues/01-loop-agentico-e-harness.md` e
`../vectora/backend/engine/` como fontes de requisitos e comportamento
observável. O backend Python é referência para semântica; detalhes de produto,
UI, RBAC, workbenches, adapters e storage proprietário não viram API da engine.

Toda capacidade pública deve ser implementada uma vez no Rust e exercitada
pelos bindings Python/PyPI e TypeScript/npm já incluídos no bootstrap. Nenhuma
etapa troca automaticamente o runtime de qualquer consumidor.

## Fases empilhadas

1. **Contrato de ciclo de vida** — identidade de execução versionada e estados
   válidos. Implementado experimentalmente com `SqliteRunLifecycleStore`: a
   admissão, evento inicial e checkpoint são persistidos atomicamente; as
   transições verificam sequência esperada. Não generaliza esse lifecycle para
   PostgreSQL nem torna efeitos externos atômicos.
2. **Montagem de turno** — acumular texto, tool calls fragmentadas, usage e
   término em resposta canônica; validar ordem e impor limites. API incremental
   no Rust e API equivalente de conformidade nos dois bindings. Não executar
   tools nesta fase.
3. **Contrato de tools e loop de modelo** — dividido em entregas empilhadas:
   (a) contrato versionado, schemas, capabilities, erros tipados e decisões
   default-deny de dispatch; (b) resultados de adapters e máquina de rodadas
   modelo → dispatch → resultados. A parte (a) está implementada
   experimentalmente em `docs/specs/10-tool-contracts-e-dispatch.md`; ela
   propaga timeout e idempotência declarados, mas não os impõe nem executa tools.
   A máquina de rodadas e o protocolo de resultados foram implementados
   experimentalmente em `docs/specs/11-model-tool-loop.md`. Ambos os bindings
   expõem o núcleo. O consumidor fornece adapters; persistência, aprovação e
   recovery durável foram acrescentados nas etapas seguintes; adapters de tool,
   identidade e autorização do host continuam fora do core.
4. **Política e aprovação** — definir snapshot efetivo e decisão default-deny;
   vincular HITL a conteúdo, arguments normalizados, escopo e digest da policy.
   Revalidar antes de efeitos e retomar. Falha de policy interrompe o efeito.
   O contrato síncrono de snapshot, bloqueios e aprovação por binding foi
   implementado experimentalmente em `docs/specs/12-policy-and-approval.md` e
   exposto pelos dois bindings. PR #30 acrescentou `SqliteToolPolicyStore`,
   exposto em Python e TypeScript, para persistir snapshot, pedidos e recibos,
   reavaliar o estado mais recente e rejeitar conflitos de binding. O contrato
   do store está em `docs/specs/16-sqlite-durable-tool-policy.md`. O host segue
   responsável por autenticar aprovadores e validar recibos; o adapter não
   executa nem cancela tools.
5. **Persistência e recovery** — integrar com os contratos de storage planejados
   em `docs/plans/01-plano-portabilidade-llm-rust.md`: reserva atômica por
   thread/turno, ledger ordenado e checkpoint antes/depois de efeitos. O adapter
   experimental `SqliteModelToolLoopStore` agora persiste o par
   evento/checkpoint em uma transação antes de devolver cada ação ao host; os
   bindings Python e TypeScript expõem o mesmo adapter. Seu contrato e seus
   limites estão em `docs/specs/13-sqlite-model-tool-loop-store.md`. O adapter
   `SqliteRunLifecycleStore` também reserva run/thread/turn e grava o evento de
   admissão, o estado inicial e o checkpoint na mesma transação; suas
   transições usam sequência esperada e checkpoints duráveis. O contrato está
   em `docs/specs/14-sqlite-durable-run-lifecycle.md`. Esses adapters não
   tornam efeitos externos atômicos, não fornecem exactly-once e não autenticam
   o host. Integração genérica com PostgreSQL, coordenação dos
   providers/adapters, autenticação de recibos pelo host e recuperação
   operacional ainda precisam ser concluídas. O cancellation request é
   durável e o host recebe os IDs pendentes para interromper e reconciliar; isso
   não comprova término dos processos. Recovery nunca repete silenciosamente
   um efeito de resultado incerto.
6. **Verificação e conformidade** — o contrato de resultados e vereditos está
   em `docs/specs/15-verification-gates.md`. `SqliteRunLifecycleStore` grava
   report, policy, avaliação e transição terminal no mesmo evento/checkpoint;
   `completed` exige veredito `passed`, `unverified` vai para
   `recovery_required`. Separadamente, `SqliteModelToolLoopStore::verify`
   exige loop concluído e grava report, policy e avaliação no checkpoint do
   loop; o consumidor ainda executa e autentica os checks. Ambos os caminhos
   estão expostos pelos bindings Python/TypeScript. O contrato de snapshot do
   loop é v2, com leitura compatível de snapshots v1. Verificação e pedido de
   cancelamento são persistidos no SQLite; o host recebe IDs de tools pendentes
   para interrompê-las e deve registrar o resultado de cada uma. Isso não prova
   que adapters/processos terminaram. Resultados incertos exigem reconciliação
   antes do estado terminal `cancelled`; sem pedido de cancelamento, a
   reconciliação permite continuar o loop. Redaction, dois perfis de consumidor e integração com
   o fluxo completo seguem abertas antes de qualquer alegação de paridade ou
   adoção.

Cada parte do plano deve ser entregue em PR próprio empilhado sobre o anterior.
Todo contrato público é implementado uma vez no Rust e coberto pelos bindings
Python/PyPI e TypeScript/npm no mesmo PR. Cada PR deixa explícito o que ainda
não é durável nem seguro para produção.

## Evidência de referência no backend Python

- `conversation_loop.py`: streaming, montagem de tool calls, limite de iterações,
  detecção de repetição, execução em lotes e resultado terminal.
- `tool_batch.py`, `guardrails.py` e `hitl.py`: budgets, policy antes de efeito,
  aprovação e redaction.
- `stream_events.py`, `decision_record.py` e `native_stream.py`: envelopes,
  identidade de run, ledger, status, recuperação, evidência e limites de
  serialização.

Esses módulos são fontes de comportamento separadas por responsabilidade; não
devem ser copiados como um único framework nem importados pela engine.
