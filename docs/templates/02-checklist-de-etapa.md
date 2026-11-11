# Checklist: [etapa/capability]

**Estado:** `proposto | aprovado | em andamento | concluído | bloqueado`  
**Plano:** [caminho em docs/plans/]  
**Análise:** [caminho em docs/analyses/]  
**Spec/requisitos:** [referências locais]

Estados dos itens: `pendente`, `feito`, `bloqueado` ou `não aplicável`. Para
`feito`, anote evidência; para `bloqueado` ou `não aplicável`, explique a razão.

## Preparação

- [ ] Confirmar repo, branch, base da stack e working tree. Evidência: [ ]
- [ ] Ler `AGENTS.md`, spec, plano e análise. Evidência: [ ]
- [ ] Listar arquivos, símbolos, testes existentes e consumidores afetados. Evidência: [ ]
- [ ] Fixar objetivo, escopo, exclusões e critérios de aceite. Evidência: [ ]
- [ ] Resolver ou registrar decisões vitais antes do trabalho dependente. Evidência: [ ]

## Contratos e segurança

- [ ] Tipos, versão, invariantes, erros e compatibilidade definidos. Evidência: [ ]
- [ ] Ownership de dados e efeitos; autenticação/autorização; fronteira de confiança. Evidência: [ ]
- [ ] Timeout, retry, cancelamento, concorrência e idempotência definidos. Evidência: [ ]
- [ ] Persistência, migração, replay e recovery definidos quando aplicáveis. Evidência: [ ]
- [ ] Redaction e observabilidade definidas; nenhum segredo em logs. Evidência: [ ]
- [ ] Dependências justificadas; fallback e rollback especificados. Evidência: [ ]

## Implementação

- [ ] Core Rust implementado sem ultrapassar o escopo. Evidência: [ ]
- [ ] Binding Python atualizado, se aplicável. Evidência: [ ]
- [ ] Binding TypeScript/Node atualizado, se aplicável. Evidência: [ ]
- [ ] Tipos/stubs, exports, documentação e exemplos sincronizados. Evidência: [ ]
- [ ] Testes de sucesso, erros, limites e falhas relevantes adicionados. Evidência: [ ]
- [ ] Testes de concorrência, timeout/cancelamento, restart/replay adicionados quando aplicáveis. Evidência: [ ]
- [ ] Conformidade entre bindings coberta por fixtures/casos equivalentes quando aplicável. Evidência: [ ]

## Verificação local

- [ ] `cargo fmt --all -- --check` — resultado: [ ]
- [ ] `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` — resultado: [ ]
- [ ] Testes Rust relevantes — comando/resultado: [ ]
- [ ] Checks Python relevantes — comando/resultado: [ ]
- [ ] Checks TypeScript/Node relevantes — comando/resultado: [ ]
- [ ] Pre-commit e checks compartilhados relevantes — comando/resultado: [ ]
- [ ] Falhas analisadas; limitações do ambiente registradas sem alegar sucesso. Evidência: [ ]

## Revisão e entrega

- [ ] Diff revisto; sem segredos, arquivos ignorados, artefatos ou alterações incidentais. Evidência: [ ]
- [ ] Análise e plano atualizados para refletir o código e evidências. Evidência: [ ]
- [ ] Critérios de aceite concluídos ou gaps transferidos explicitamente. Evidência: [ ]
- [ ] Próxima etapa e dependências registradas. Evidência: [ ]
- [ ] Commit/PR preparados conforme as regras do repositório e instruções do mantenedor. Evidência: [ ]

## Critério de fechamento desta etapa

Fechar a etapa quando todos os critérios de aceite do plano estiverem atendidos
e documentados, ou quando uma decisão vital/bloqueio externo exigir pausa. A
pausa deve incluir pergunta/evidências, impacto no trabalho e condição para
retomada; não converter item pendente em concluído.
