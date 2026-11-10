# Rodadas de modelo e resultados de tools

## Estado

Experimental e em memória. O mesmo coordenador é implementado em Rust e
exposto pelos bindings Python/PyPI e TypeScript/npm. O consumidor continua
responsável por chamar o provider e executar os adapters.

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
  adapter. O output dessas negações contém somente um código estável.
- Limites padrão: 50 rodadas de modelo, 128 chamadas de tools totais, 1 MiB por
  output e 4 MiB de outputs por lote. Cada limite é configurável e limitado a
  16 MiB quando representa bytes. A engine mede bytes UTF-8.
- Um efeito externo com resultado `unknown` move o loop para
  `recovery_required`; o host não deve solicitar uma nova rodada que possa
  repetir cegamente esse efeito. Reconciliação e retomada são trabalho futuro.
- Se o limite de rodadas for atingido numa resposta que pede tools, o loop para
  antes de pedir sua execução. Assim, não inicia efeitos sem espaço orçado para
  consultar o modelo depois.

## Responsabilidades do consumidor

O consumidor impõe timeout e cancelamento, redige outputs, chama o gate de
[`12-policy-and-approval.md`](12-policy-and-approval.md) imediatamente antes
de executar adapters e persiste estado/ledger de forma durável quando
necessário. Esta máquina de loop não chama providers ou adapters, não faz
retry, não garante exactly-once e não invoca o gate automaticamente. Snapshots
fornecidos pelo consumidor podem ser forjados; isto não é por si só uma
fronteira de segurança ou autorização.

## APIs

- Rust: `ModelToolLoopState::new`, `advance_model_turn` e
  `advance_tool_result`.
- Python: `start_model_tool_loop`, `accept_model_turn` e `submit_tool_result`.
- TypeScript: `startModelToolLoop`, `acceptModelTurn` e `submitToolResult`.

Os dois bindings chamam o mesmo núcleo Rust e têm testes do ciclo completo.
