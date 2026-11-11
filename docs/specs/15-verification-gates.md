# Resultados de verificação e gates

**Estado:** contrato experimental v1, implementado no núcleo Rust e exposto
pelos bindings Python e TypeScript/Node.

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

Os bindings serializam os mesmos contratos e chamam a implementação Rust.

## Limites

- O report e a decisão ainda não são persistidos no ledger/run lifecycle.
- A engine não executa checks, não baixa nem valida o conteúdo referenciado,
  não autentica o host e não prova que `redacted: true` é verdadeiro.
- O consumidor deve persistir report e decisão de forma durável antes de
  tratar o gate como autoridade de retomada. Isso será integrado ao lifecycle
  em uma fase posterior.
- O timestamp vem do consumidor para manter a avaliação determinística; o
  host é responsável por uma fonte de tempo confiável.
- A avaliação não muda automaticamente o estado do run; o host controla a
  transição correspondente após persistir o resultado.

## Conformidade

Testes cobrem precedência, checks opcionais, falha funcional, falha de
infraestrutura, check não executado, evidência faltante/expirada, digest
inválido, redação obrigatória, decisões configuráveis e chamadas pelos dois
bindings. Eles validam o contrato, não a autenticidade dos resultados nem a
execução de pipelines externos.
