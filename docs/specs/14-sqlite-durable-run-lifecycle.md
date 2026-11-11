# Lifecycle durável de runs em SQLite

## Estado

Experimental. O núcleo Rust e os bindings Python e TypeScript/Node expõem o
adapter `SqliteRunLifecycleStore`. A implementação fornece admissão durável,
transições ordenadas e leitura do checkpoint mais recente para um escopo.

## Contrato

- `admit(identity, request)` valida a identidade e o escopo, reserva o run pela
  combinação de scope, thread e turn e registra o evento `run_admitted` na
  thread. O evento de admissão, o evento `run_lifecycle_admitted`, o snapshot
  `planned` e o checkpoint inicial são gravados na mesma transação SQLite.
- Repetir a admissão com a mesma identidade/idempotency key e o mesmo request
  retorna a reserva e os eventos existentes. Uma colisão de chave com conteúdo
  diferente é reportada pelo contrato do ledger.
- `load_latest(run_id)` lê e valida o checkpoint mais recente dentro do escopo
  configurado. Um checkpoint de outro run, agregado, sequência ou versão é
  rejeitado como estado inválido.
- `transition(state, next, operation_id)` valida o snapshot, aplica a máquina
  de estados existente e persiste evento e checkpoint atomicamente. A sequência
  esperada impede gravar a partir de um snapshot obsoleto. Repetir a mesma
  operação é idempotente; tentar uma operação diferente a partir de estado
  obsoleto retorna conflito de versão. Uma transição para o estado já atual é
  um no-op somente quando o snapshot fornecido ainda corresponde ao último
  checkpoint.
- `cancelled` é um estado terminal durável da lifecycle. A transição para esse
  estado usa a mesma chave de operação, sequência esperada e checkpoint das
  demais transições; sua leitura após reabrir o banco e o replay da mesma
  operação devem produzir o mesmo resultado. A engine registra o estado, mas
  não interrompe tarefas do host nem cancela adapters externos.
- Os bindings encaminham as operações para esse mesmo código Rust e transportam
  os contratos como JSON versionado.

## Limites e responsabilidades do host

- SQLite é o único adapter implementado para esse contrato. Não há adapter de
  lifecycle PostgreSQL nem teste de conformidade entre bancos nesta etapa.
- A reserva protege a identidade do run, não torna atômicos efeitos externos e
  não oferece semântica exactly-once para chamadas de provider ou tools.
- A engine valida o formato e o escopo declarado, mas não autentica `actor_id`,
  não concede capabilities e não autoriza tools.
- O host deve remover segredos e conteúdo sensível de `request` antes de
  persistir, além de definir retenção, backup e permissões do arquivo SQLite.
- O host interrompe tarefas de provider/tool e reconcilia efeitos externos.
  Se uma tool puder ter produzido efeito e o resultado for incerto, o host deve
  usar `recovery_required` até reconciliar; marcar o run como `cancelled` não
  prova que o efeito foi interrompido ou revertido.
- O adapter não substitui `SqliteModelToolLoopStore` e ainda não conecta
  automaticamente lifecycle, policy, execução de tools, cancelamento ou
  recovery do loop completo.

## Verificação

Os testes Rust cobrem admissão idempotente, atomicidade observável após leitura,
sequência de eventos por thread, isolamento de escopo, persistência após
reabertura, transições e conflito de snapshot obsoleto. Os testes dos bindings
verificam que as duas linguagens chegam ao mesmo adapter nativo.
