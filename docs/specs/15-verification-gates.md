# Resultados de verificação e gates

**Estado:** contrato experimental v1, implementado no núcleo Rust e exposto
pelos bindings Python e TypeScript/Node. O adapter SQLite do lifecycle persiste
report, policy, avaliação e transição no mesmo evento/checkpoint.

## Responsabilidade

Uma verificação reporta o resultado dos checks configurados pelo consumidor.
O gate combina apenas os checks obrigatórios e aplica uma política explícita.
`verification_status` descreve evidência observada; `verdict` decide se a
execução pode prosseguir. Um status de execução ou resposta textual do modelo
não substitui esta avaliação.

A engine valida o formato, correlação, digest e redação declarada. O consumidor
define quais checks são relevantes, executa-os, protege e persiste os
artifacts, confirma os resultados e autentica quem os forneceu.

## Contrato

`VerificationReport` contém versão, `run_id`, timestamp fornecido pelo host e
checks identificados. Cada check informa `passed`, `failed`, `infra_error` ou
`not_run` e pode referenciar artifacts. A evidência contém origem, referência
opaca do artifact, SHA-256 opcional, confirmação de redação e expiração
opcional. Conteúdo do artifact não é enviado ao núcleo.

`EvidenceDispositionPolicy` também contém a versão do contrato e lista
explicitamente os checks obrigatórios, define se cada check aprovado precisa
de evidência vigente e configura o destino para
falhas (`blocked` ou `degraded`) e resultados inconclusivos (`blocked`,
`degraded` ou `unverified`). Checks opcionais não alteram o gate.

A avaliação tem precedência determinística:

1. Qualquer check obrigatório `failed` produz status `failed` e o destino
   configurado para falha.
2. Sem falhas, qualquer `infra_error` produz status `infra_error` e destino
   inconclusivo.
3. Sem os anteriores, qualquer `not_run` produz status `not_run` e destino
   inconclusivo.
4. Evidência obrigatória ausente ou expirada em um check aprovado produz status
   `not_run` e destino inconclusivo. Checks `failed`, `infra_error` ou `not_run`
   não precisam de artifacts para explicar seu próprio resultado.
5. Somente checks obrigatórios aprovados e com as evidências exigidas vigentes
   produzem status/veredito `passed`.

Motivos são códigos estáveis: `required_check_failed`,
`verification_infrastructure_error`, `required_check_not_run`,
`required_evidence_missing_or_expired` e `all_required_checks_passed`.

## APIs

- Rust: `evaluate_verification` e `EVIDENCE_GATE_CONTRACT_VERSION`.
- Python: `evaluate_verification` e `evidence_gate_contract_version`.
- TypeScript: `evaluateVerification` e `evidenceGateContractVersion`.
- Lifecycle SQLite: `SqliteRunLifecycleStore::verify`, `sqlite_run_verify` e
  `sqliteRunVerify`.
- Loop SQLite: `SqliteModelToolLoopStore::verify`,
  `sqlite_model_tool_loop_verify` e `sqliteModelToolLoopVerify`.

Os bindings serializam os mesmos contratos e chamam a implementação Rust.

## Limites

- Persistência de report e decisão está disponível no adapter SQLite; outros
  adapters de lifecycle ainda não implementam este contrato.
- A verificação do loop durável exige fase `completed`, correlaciona `run_id`
  e grava report, policy e avaliação no checkpoint/evento do loop. Ela não
  altera o status de `RunLifecycle`; são dois contratos persistentes distintos.
- Cada loop aceita um único registro de verificação. Repetir a mesma operação
  com conteúdo idêntico é idempotente; outra operação não substitui a evidência.
- O contrato do snapshot do loop é v3. Snapshots anteriores (v1 e v2) são
  rejeitados pelo contrato atual; o armazenamento não os converte. A versão do
  evidence gate permanece v1 e é independente da versão do snapshot do loop.
- A engine não executa checks, não baixa nem valida o conteúdo referenciado,
  não autentica o host e não prova que `redacted: true` é verdadeiro.
- A verificação persiste apenas referências opacas, nunca conteúdo de logs ou
  artefatos. O consumidor controla sua retenção e disponibilidade.
- O timestamp vem do consumidor para manter a avaliação determinística; o
  host é responsável por uma fonte de tempo confiável.
- `evaluate_verification` é uma avaliação pura. `SqliteRunLifecycleStore::verify`
  persiste o report, a policy, o veredito e a transição correspondente em uma
  única operação. Um run inconclusivo pode ser retomado e receber um novo
  report; o ledger mantém cada evento e o checkpoint guarda a verificação mais
  recente.
- `SqliteModelToolLoopStore::verify` é uma operação separada: registra a
  avaliação depois da conclusão do loop, sem executar checks nem encerrar um
  run lifecycle.
- Transições usam o snapshot mais recente persistido. Um snapshot alterado ou
  desatualizado é rejeitado mesmo quando sua sequência numérica coincide.

## Conformidade

Testes cobrem precedência, checks opcionais, falha funcional, falha de
infraestrutura, check não executado, evidência faltante/expirada, digest
inválido, redação obrigatória, decisões configuráveis, persistência atômica,
replay idempotente, isolamento entre runs e chamadas pelos dois bindings. Eles
validam o contrato, não a autenticidade dos resultados nem a execução de
pipelines externos.
