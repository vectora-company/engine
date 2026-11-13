# Documentação da Vectora Engine

Use este índice para navegar da orientação geral às especificações de uma
capability. `AGENTS.md` continua sendo a autoridade para as regras do repositório.

## Capability ativa

O plano ativo é [`docs/plans/03-plano-continuidade.md`](plans/03-plano-continuidade.md),
com o estado verificado, a direção de uma primeira release completa e o backlog
por fases. Em 2026-11-13, a stack aberta segue #82 → #83 → #84 → #85 → #86 → #87 → #88 → #89 → #90 → #91 → #92 → branch local `feat/postgres-admission-projection` (filha do #92; em andamento). O PR #86 substitui diretamente os resultados textuais de tools por partes tipadas; #87 revisa os Markdown; #88 reclassifica MIME na reidratação; #89 registra a fronteira de persistência; #90 implementa a decisão respondida no chat: request persistível separado do envelope efêmero para envio inline. O PR #90 é filho de #89; o PR #91 (diagnósticos da outbox) é filho do #90; o PR #92 (projeção de checkpoints) é filho do #91 e está aberto. A branch local `feat/postgres-admission-projection` é filha do #92 e implementa a etapa seguinte. Storage e runtime seguem com gaps registrados no plano e nas specs.

## Operação de desenvolvimento

- [Prompt curto para o goal](plans/00-prompt-loop-goal.md): texto reutilizável
  para iniciar o loop, apontando para os documentos de referência e as regras
  de encerramento.
- [Pacote do goal loop](goal-loop/README.md): índice e regras operacionais do
  mantenedor, com
  [`docs/goal-loop/01-plano-operacional.md`](goal-loop/01-plano-operacional.md),
  [`docs/goal-loop/02-checklist-do-loop.md`](goal-loop/02-checklist-do-loop.md),
  [`docs/goal-loop/03-fontes-e-evidencias.md`](goal-loop/03-fontes-e-evidencias.md) e
  [`docs/goal-loop/04-criterios-de-encerramento.md`](goal-loop/04-criterios-de-encerramento.md).
- [Plano do loop de desenvolvimento](plans/05-loop-de-desenvolvimento.md): ordem
  de leitura, ciclo de implementação, atualização dos documentos e critérios
  explícitos para continuar ou encerrar o loop.
- [Checklist de implementação](checklists/00-checklist-de-implementacao.md):
  checklist completo para preparar, implementar, verificar e entregar cada
  etapa de um plano.
- [Checklist da auditoria OpenAI-compatible](checklists/01-review-pr3-openai-compatible.md):
  classificação dos findings antigos e verificação das correções desta etapa.
- [Análise e referências de desenvolvimento](analyses/00-analise-e-referencias-de-desenvolvimento.md):
  hierarquia das fontes, investigação obrigatória, registro de decisões e
  limites para inferências.
- [Templates de desenvolvimento](templates/README.md): modelos reutilizáveis
  [`docs/templates/00-plano-de-capability.md`](templates/00-plano-de-capability.md),
  [`docs/templates/01-analise-de-capability.md`](templates/01-analise-de-capability.md) e
  [`docs/templates/02-checklist-de-etapa.md`](templates/02-checklist-de-etapa.md).
- [Auditoria de review do PR #3](analyses/01-auditoria-review-pr3.md): findings
  revalidados contra a implementação atual, com evidências e limitações.
- [Matriz de conformidade das capabilities](analyses/02-matriz-conformidade-capabilities.md):
  contratos, APIs Rust/Python/TypeScript, testes existentes, maturidade e gaps.
- [Análise cruzada: Akita, Engine, Vectora e GH Bot](analyses/03-analise-cruzada-akita-engine-vectora-gh-bot.md):
  livro, snapshots de código, capacidades comparadas, fronteiras e sabatina.
- [Sabatina da Engine e do ecossistema](analyses/04-sabatina-engine-ecossistema.md):
  200 perguntas e respostas sobre arquitetura, implementação, lacunas,
  consumidores e frameworks/harnesses relacionados. Atualize-a em cada ciclo
  do goal loop conforme o [prompt](plans/00-prompt-loop-goal.md).
- [Checklist da matriz de conformidade](checklists/02-matriz-conformidade-capabilities.md):
  inventário de 11 de novembro de 2026, concluído; registro histórico.
- [Auditoria de storage S0–S5](analyses/05-auditoria-storage-s0-s5.md):
  evidências, execuções service-real e decisões do mantenedor sobre storage.
- [Checklist da auditoria de storage](checklists/03-auditoria-storage-s0-s5.md):
  itens feitos e pendentes da capability de storage.
- [`docs/checklists/04-midia-no-chat.md`](checklists/04-midia-no-chat.md): contrato,
  payloads nativos e verificações da etapa de mídia.

## Planos

- [Portabilidade de LLM para Rust](plans/01-plano-portabilidade-llm-rust.md)
- [Loop agêntico](plans/02-plano-loop-agentico.md)
- [Continuidade](plans/03-plano-continuidade.md)
- [Storage e ledger](plans/04-plano-storage-ledger.md)
- [Loop de desenvolvimento](plans/05-loop-de-desenvolvimento.md)

## Documentação pública

A documentação pública da Engine é mantida no portal central da organização:
[docs.vectora.company/engine](https://docs.vectora.company/engine/). O site e o
workflow de publicação pertencem ao repositório
[`documentation`](https://github.com/vectora-company/documentation); este
repositório mantém planos, auditorias, sabatina e especificações internas em
`docs/`.

## Especificações

Consulte [o índice de specs](specs/README.md) para os contratos propostos e suas
referências locais. Specs definem o escopo da engine; planos ordenam a execução.

## Auditorias

- [Auditoria e walkthrough da main](audits/01-auditoria-e-walkthrough-da-main.md)

As auditorias registram observações datadas. Revalide o estado do código antes
de tratar uma observação histórica como estado atual.
