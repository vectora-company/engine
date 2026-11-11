# Contratos de tools e preparação de dispatch

## Estado

Experimental: o Rust valida contratos JSON Schema, argumentos, estado da
execução e capabilities, depois devolve decisões ordenadas de invocação ou
negação. As mesmas funções são expostas pelos bindings Python e TypeScript.

## Contrato implementado

- `ToolContract` é versionado e declara nome, versão do schema, JSON Schema de
  entrada, capability requerida, timeout, classe de efeito e garantia de
  idempotência declarada pelo adapter.
- Schemas precisam ser objetos, caber no limite de 256 KiB e compilar sem
  resolução remota. O núcleo não acessa rede para carregar `$ref`.
- Cada chamada do modelo é validada contra o schema completo via JSON Schema.
  Argumentos malformados, nome desconhecido, capability não concedida, schema
  incompatível e excesso do limite de chamadas produzem uma decisão `deny`
  tipada, sem incluir argumentos ou valores sensíveis no motivo.
- A execução exige run em estado `running` ou `tool_running`. A identidade e as
  capabilities devem ter sido fornecidas pelo host confiável; este contrato não
  autentica nem emite permissões.
- A saída conserva a ordem das chamadas. Lotes com apenas efeitos `read_only`
  podem ser executados concorrentemente; qualquer efeito autorizado diferente
  exige execução sequencial do lote pelo consumidor.
- `requires_key` produz uma chave determinística a partir do run e do ID da
  chamada. Isso não é uma reserva durável nem uma garantia de exactly-once.
- `tool_chat_definitions` gera os schemas visíveis ao provider a partir dos
  mesmos contratos validados, evitando duas descrições divergentes.

## Limites

O dispatch não executa ferramentas, não aplica RBAC, policy completa, HITL ou
aprovação, não impõe timeouts, não cancela adapters e não persiste decisões,
resultados ou efeitos. O consumidor executa cada `invoke` através de seu adapter,
deve impor o timeout declarado, revalidar sua política imediatamente antes do
efeito e persistir/reconciliar resultados conforme o plano de storage. A chave
de idempotência só ajuda se o adapter/provedor a suportar.

O coordenador experimental das rodadas está definido em
[`11-model-tool-loop.md`](11-model-tool-loop.md). Persistência, aprovação e
recovery durável não são prometidos por aquele contrato.

## Próxima parte do plano

Use o gate experimental de [`12-policy-and-approval.md`](12-policy-and-approval.md)
antes de executar os adapters. A função aqui continua sendo uma primitiva de
preparação; ela isoladamente não prova que uma tool passou pela policy nem por
aprovação humana.
