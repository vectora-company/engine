# Rodadas de modelo e resultados de tools

## Estado

Experimental. O mesmo coordenador é implementado em Rust e exposto pelos
bindings Python/PyPI e TypeScript/npm. O consumidor continua responsável por
chamar o provider, executar adapters e persistir snapshots/ledger.

## Contrato

- `ModelToolLoopState` identifica run e turno e conta rodadas do modelo e
  chamadas de tools. É um snapshot serializável, mas não é persistente nem deve
  ser tratado como confiável ao ser carregado de armazenamento.
- `accept_model_turn` recebe uma resposta final já montada e o dispatch
  autorizado pelo contrato anterior. Confere identidade, ordem, chamada e modo
  de execução. O raciocínio do provider é removido do snapshot e dos eventos de
  coordenação.
- O host recebe uma ação discriminada: executar invocações, aguardar resultados,
  continuar com resultados ordenados, concluir, parar por limite ou interromper
  para reconciliação.
- Resultados precisam corresponder a run, turno, ID e nome da chamada pendente.
  Resultados repetidos, desconhecidos, inconsistentes ou acima dos limites são
  rejeitados. Resultados podem chegar fora de ordem; a continuação ao modelo
  conserva a ordem original das chamadas.
- Negações do dispatch viram resultados `denied`/`not_applied` sem chamar o
  adapter. A parte textual contém somente um código estável.
- `ToolResult` v2 contém `parts`, uma sequência de `text { text }` e
  `image { media_ref }`. O helper `to_chat_message` produz a mensagem de tool
  canônica; o host entrega o store autorizado ao contexto de mídia.
- Limites padrão: 50 rodadas de modelo, 128 chamadas de tools totais, 1 MiB por
  resultado e 4 MiB de resultados por lote. Para o loop, contam os bytes JSON
  serializados das partes, inclusive metadados da referência e texto. O asset
  binário é limitado pelo `MediaExecutionContext`. Cada limite é configurável e
  limitado a 16 MiB quando representa bytes.
- Um efeito externo com resultado `unknown` move o loop para
  `recovery_required`; o host não deve solicitar uma nova rodada que possa
  repetir cegamente esse efeito.
- `recover_model_tool_loop` converte um snapshot em `awaiting_tool_results` para
  `recovery_required` depois de um restart. A operação nunca retorna ação de
  execução de tools: o host deve reconciliar explicitamente cada chamada ainda
  sem resultado, inclusive chamadas de leitura, pois a engine não sabe se o
  adapter chegou a iniciá-las.
- `reconcile_tool_result` aceita somente um resultado com efeito conhecido
  (`not_applied`, `applied` ou `not_applicable`) para uma chamada ausente ou
  previamente marcada `unknown`. Quando todas as chamadas têm resultado
  conhecido, devolve os resultados na ordem original para a próxima rodada do
  modelo. Um resultado ainda `unknown` permanece bloqueado.
- `cancel_model_tool_loop` registra cancelamento separadamente do cancelamento
  do stream/provider. Durante `awaiting_model`, conclui imediatamente como
  `cancelled`. Durante `awaiting_tool_results`, muda para `cancelling` e retorna
  a lista de chamadas que o host deve interromper. O loop só se torna terminal
  depois que cada adapter informa um resultado conhecido. Se uma chamada tiver
  efeito `unknown`, o estado passa por `recovery_required`; a reconciliação
  continua obrigatória e, ao terminar, fecha como `cancelled` sem retomar o
  modelo.
- O snapshot do loop é contrato v3 e `ToolResult` é contrato v2. A troca de
  `output: String` por `parts` é breaking; snapshots de loop v1/v2 e resultados
  v1 não são aceitos nem convertidos.
- Se o limite de rodadas for atingido numa resposta que pede tools, o loop para
  antes de pedir sua execução. Assim, não inicia efeitos sem espaço orçado para
  consultar o modelo depois.

## Responsabilidades do consumidor

O consumidor impõe timeout e cancelamento, redige outputs, chama o gate de
[`12-policy-and-approval.md`](12-policy-and-approval.md) imediatamente antes
de executar adapters e persiste estado/ledger de forma durável quando
necessário. Ao receber `cancel_pending_tools`, o host deve propagar interrupção
às tasks/adapters e submeter um resultado para cada chamada; o estado não prova
que a task parou. O host precisa autenticar quem reconcilia e confirmar o resultado
com a fonte de verdade do adapter; a engine valida formato e correlação, mas
não consegue provar que a afirmação do host é verdadeira. O host também deve
persistir cada transição antes de executar efeitos. Esta máquina de loop não
chama providers ou adapters, não faz retry, não garante exactly-once e não
invoca o gate automaticamente. Snapshots fornecidos pelo consumidor podem ser
forjados; isto não é por si só uma fronteira de segurança ou autorização.

## APIs

- Rust: `ModelToolLoopState::new`, `advance_model_turn` e
  `advance_tool_result`, `cancel_model_tool_loop`, `recover_model_tool_loop` e
  `reconcile_tool_result`.
- Python: `start_model_tool_loop`, `accept_model_turn`, `submit_tool_result`,
  `tool_result_contract_version` e `tool_result_to_chat_message`,
  `cancel_model_tool_loop`, `recover_model_tool_loop` e `reconcile_tool_result`.
- TypeScript: `startModelToolLoop`, `acceptModelTurn`, `submitToolResult`,
  `toolResultContractVersion` e `toolResultToChatMessage`,
  `cancelModelToolLoop`, `recoverModelToolLoop` e `reconcileToolResult`.

Os dois bindings chamam o mesmo núcleo Rust e têm testes do ciclo completo.
