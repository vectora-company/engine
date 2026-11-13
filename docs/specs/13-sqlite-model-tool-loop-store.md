# Persistência SQLite do loop de modelo e tools

## Estado

Adapter experimental específico de SQLite para a máquina de loop definida em
[`11-model-tool-loop.md`](11-model-tool-loop.md). Ele não substitui a máquina
em memória nem define ainda uma interface de persistência comum a todos os
backends.

## Garantias

- Cada loop é identificado pelo par `run_id` e `turn_id` dentro de um `scope`.
  Loops de turnos diferentes não compartilham agregado ou checkpoint.
- `start` persiste o estado inicial antes de devolver o estado ao consumidor.
- Cada transição grava um evento ordenado e o snapshot correspondente na mesma
  transação SQLite. A ação resultante só é devolvida depois do commit; em
  particular, a ação `execute_tools` não chega ao host antes de sua transição
  estar persistida.
- `operation_id` é a chave de idempotência da operação. Repetir a mesma
  operação e conteúdo retorna o evento/checkpoint já gravados. Reutilizar a
  chave com conteúdo diferente é conflito. O chamador deve preservar a chave
  ao repetir uma solicitação cujo resultado ficou incerto.
- `ledger_sequence` fornece controle otimista: uma transição baseada em estado
  antigo falha em vez de sobrescrever uma versão posterior.
- `load_latest` valida versão do contrato, escopo, identidade run/turno,
  invariantes do loop e sequência do checkpoint. O snapshot atual usa contrato
  v3 e contém `ToolResult` v2 com partes tipadas; snapshots v1/v2 não são
  migrados nem aceitos.
- Recovery persiste `recovery_required`; ele não repete adapters. O host ainda
  deve autenticar o reconciliador e confirmar o efeito externo antes de chamar
  `reconcile_tool_result`.
- `verify` aceita somente um loop na fase `completed`, exige que o `run_id` do
  report corresponda e persiste report, policy e avaliação no mesmo evento e
  checkpoint. A execução dos checks e a autenticação de suas evidências
  continuam sob responsabilidade do consumidor. Uma verificação registrada é
  imutável; replay idêntico é idempotente.
- Snapshots do loop anteriores ao contrato v3 não são compatíveis com a nova
  forma tipada de `ToolResult`; esta quebra foi aceita antes da primeira release.
  O campo opcional `verification` permanece parte do estado versionado.

## Limites e responsabilidades

- A transação cobre somente o ledger/checkpoint SQLite. Ela não torna um efeito
  externo atômico com a gravação nem fornece exactly-once entre SQLite e um
  adapter. Uma queda após o efeito e antes do registro do resultado continua
  exigindo reconciliação.
- O host autoriza `scope`, escolhe o arquivo, protege permissões do arquivo e
  redige dados sensíveis antes de fornecer completion, argumentos e resultados.
  A engine não autentica o processo que chama os bindings.
- Eventos e snapshots estão sujeitos ao limite de payload do ledger. Erro de
  validação ou gravação impede que a ação seja entregue ao consumidor.
- Esta versão oferece somente SQLite. PostgreSQL e uma abstração comum entre
  adapters permanecem trabalho futuro; não há dual-write.

## APIs

- Rust: `SqliteModelToolLoopStore::open`, `start`, `load_latest`,
  `accept_model_turn`, `submit_tool_result`, `recover` e
  `reconcile_tool_result`.
- Python: `sqlite_model_tool_loop_start`, `sqlite_model_tool_loop_load`,
  `sqlite_model_tool_loop_accept_model_turn`,
  `sqlite_model_tool_loop_submit_tool_result`,
  `sqlite_model_tool_loop_recover` e
  `sqlite_model_tool_loop_reconcile_tool_result`,
  `sqlite_model_tool_loop_verify`.
- TypeScript: `sqliteModelToolLoopStart`, `sqliteModelToolLoopLoad`,
  `sqliteModelToolLoopAcceptModelTurn`,
  `sqliteModelToolLoopSubmitToolResult`, `sqliteModelToolLoopRecover` e
  `sqliteModelToolLoopReconcileToolResult`, `sqliteModelToolLoopVerify`.
