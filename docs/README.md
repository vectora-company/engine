# Documentação da Vectora Engine

Use este índice para navegar da orientação geral às especificações de uma
capability. `AGENTS.md` continua sendo a autoridade para as regras do repositório.

## Capability ativa

Consulte os [documentos de storage no goal loop](goal-loop/README.md#capability-ativa).

## Operação de desenvolvimento

- [Prompt curto para o goal](plans/00-prompt-loop-goal.md): texto reutilizável
  para iniciar o loop, apontando para os documentos de referência e as regras
  de encerramento.
- [Pacote do goal loop](goal-loop/README.md): plano operacional, checklist
  completo, hierarquia de fontes/evidências e critérios para encerrar.
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
  para planos, análises de referências e checklists completos por etapa.
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
  inventário concluído e trabalho pendente antes da próxima etapa do plano.

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
