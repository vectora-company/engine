# Montagem de turno a partir do stream

## Estado

Experimental: o núcleo Rust acumula eventos versionados de chat em uma única
resposta de assistant. A mesma lógica está disponível pelos bindings Python e
TypeScript/Node.

## Contrato implementado

- Acumula deltas de texto em ordem e tool calls fragmentadas por índice.
- Preserva IDs e nomes enviados pelo provider; gera ID determinístico com o
  `turn_id` quando o provider não envia ID.
- Ordena tool calls pelo índice, preserva argumentos inválidos como estado
  explícito e associa o último evento de usage e modelo selecionado.
- Exige evento terminal `completed`, rejeita eventos posteriores, troca de
  modelo depois de saída visível, fragmentos conflitantes e IDs duplicados.
- Impõe limites configuráveis para texto, quantidade de tool calls e tamanho dos
  argumentos. Raciocínio bruto não entra na resposta montada.
- Não executa tools, não decide autorização e não persiste eventos.

O núcleo oferece `TurnAccumulator` incremental. As APIs Python e TypeScript
aceitam uma sequência de eventos já coletada e chamam `assemble_turn` no Rust;
elas são uma fronteira de conformidade e conveniência, não fazem streaming
incremental entre chamadas de binding.

## Limites e integração futura

A montagem não confirma que a resposta ou uma tool foi persistida, autorizada,
executada ou verificada. Os consumidores devem redigir dados antes de persistir
argumentos e resultados. O loop agêntico poderá usar diretamente o acumulador
Rust junto ao stream cancelável, sem materializar uma segunda lista de eventos.

Ainda são necessários contratos de tools e policy, execução cancelável,
persistência/idempotência, checkpoints, aprovação e evidência antes de efeitos
externos serem executados pelo Harness.
