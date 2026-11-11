# Documentação da Vectora Engine

Use este índice para navegar da orientação geral às especificações de uma
capability. `AGENTS.md` continua sendo a autoridade para as regras do repositório.

## Operação de desenvolvimento

- [Prompt curto para o goal](plans/00-prompt-loop-goal.md): texto reutilizável
  para iniciar o loop com referências e critérios de encerramento explícitos.
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

## Planos

- [Portabilidade de LLM para Rust](plans/01-plano-portabilidade-llm-rust.md)
- [Loop agêntico](plans/02-plano-loop-agentico.md)
- [Continuidade](plans/03-plano-continuidade.md)
- [Storage e ledger](plans/04-plano-storage-ledger.md)
- [Loop de desenvolvimento](plans/05-loop-de-desenvolvimento.md)

## Especificações

Consulte [o índice de specs](specs/README.md) para os contratos propostos e suas
referências locais. Specs definem o escopo da engine; planos ordenam a execução.

## Auditorias

- [Auditoria e walkthrough da main](audits/01-auditoria-e-walkthrough-da-main.md)

As auditorias registram observações datadas. Revalide o estado do código antes
de tratar uma observação histórica como estado atual.
