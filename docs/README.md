# Documentação da Vectora Engine

Use este índice para navegar da orientação geral às especificações de uma
capability. `AGENTS.md` continua sendo a autoridade para as regras do repositório.

## Capability ativa

Estado em 15/11/2026: a branch `feat/llama-server-streaming-tts` é filha do PR [#105](https://github.com/vectora-company/engine/pull/105) e implementa TTS incremental pelas rotas OpenAI-compatible e nativa. Workspace Rust (208 testes, 3 fixtures e teste nativo Python), Clippy, pytest (56 pass/1 skip), build NAPI/TypeScript e Node (42 pass/11 skips) passaram; o PR [#106](https://github.com/vectora-company/engine/pull/106) está `OPEN`/`MERGEABLE`. O checkout é `/home/brunosrz/Área de trabalho/engine`; pnpm usa o `node_modules` local, e a alteração em `bindings/typescript/native/index.d.ts` deve ficar fora do commit. Plano, spec, checklist e análise: [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md), [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](specs/18-providers-e-modalidades-llama-cpp-v1.md), [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](checklists/05-providers-e-modalidades-llama-cpp-v1.md), [`docs/analyses/06-auditoria-providers-modalidades.md`](analyses/06-auditoria-providers-modalidades.md).

Decision Model `/v1/systemone` foi publicado no PR [#103](https://github.com/vectora-company/engine/pull/103), filha do #102. O PR [#104](https://github.com/vectora-company/engine/pull/104), TTS bufferizado, está `MERGEABLE`. A branch atual `feat/llama-server-realtime-audio`, filha do #104, implementa WebSocket `/v1/realtime` com PCM16, transcription/conversation e eventos com backpressure; fixtures Rust e builds dos bindings passaram. Workspace Rust (205 testes + 3 fixtures), Clippy, maturin/pytest (55 + 1 skip), build NAPI/TypeScript e Node (41 + 11 skips) passaram; PR [#105](https://github.com/vectora-company/engine/pull/105), head `e97925b`, está aberto/mergeable sobre a branch do #104. A próxima etapa é streaming TTS binário pelas variantes `stream_format: audio` e `stream: true`; integração com servidor/modelo real segue pendente. Consulte o plano, spec, checklist e análise da capability abaixo. Consulte [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md), [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](specs/18-providers-e-modalidades-llama-cpp-v1.md), [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](checklists/05-providers-e-modalidades-llama-cpp-v1.md) e [`docs/analyses/06-auditoria-providers-modalidades.md`](analyses/06-auditoria-providers-modalidades.md).

O plano ativo é [`docs/plans/03-plano-continuidade.md`](plans/03-plano-continuidade.md),
com o charter de primeira release completa e o backlog por fases. O PR [#96](https://github.com/vectora-company/engine/pull/96) foi integrado; `main` está em `f4a8cf9d7e30d9fb667a7f443f6572067e536d81`. A revisão documental está no PR [#97](https://github.com/vectora-company/engine/pull/97), head `616f558`; o perfil Rust está no PR filho [#98](https://github.com/vectora-company/engine/pull/98), head `5a463bc`. Os bindings Python/TypeScript estão no PR filho [#100](https://github.com/vectora-company/engine/pull/100). A etapa FalkorDB está registrada no PR documental [#101](https://github.com/vectora-company/engine/pull/101), filho do #100; a implementação foi publicada no PR [#102](https://github.com/vectora-company/engine/pull/102), filho do #101; integração service-real concluída. Atualização real Cargo/uv concluída no branch; pnpm já estava atualizado, e `scons lint`/`scons tests` passaram. Os artefatos estão em [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](specs/18-providers-e-modalidades-llama-cpp-v1.md), [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md), [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](checklists/05-providers-e-modalidades-llama-cpp-v1.md) e [`docs/analyses/06-auditoria-providers-modalidades.md`](analyses/06-auditoria-providers-modalidades.md).

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
- [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](checklists/05-providers-e-modalidades-llama-cpp-v1.md):
  matriz de capacidade e gates para providers/modalidades.
- [`docs/checklists/06-storage-grafos-falkordb.md`](checklists/06-storage-grafos-falkordb.md): contrato, estado e aceite da etapa FalkorDB; decisões de API/client/MSRV respondidas.
- [`docs/analyses/06-auditoria-providers-modalidades.md`](analyses/06-auditoria-providers-modalidades.md): evidências do inventário e leitura da API `llama-server` v1.0.0.
- [`docs/analyses/07-auditoria-storage-grafos-falkordb.md`](analyses/07-auditoria-storage-grafos-falkordb.md): estado local, fatos upstream, implementação, simulação de updates e integração FalkorDB 6.0.3 concluídas localmente; TLS/cancelamento remoto documentados como lacunas.
- [`docs/checklists/04-midia-no-chat.md`](checklists/04-midia-no-chat.md): contrato,
  payloads nativos e verificações da etapa de mídia.

## Planos

- [Portabilidade de LLM para Rust](plans/01-plano-portabilidade-llm-rust.md)
- [Loop agêntico](plans/02-plano-loop-agentico.md)
- [Continuidade](plans/03-plano-continuidade.md)
- [Storage e ledger](plans/04-plano-storage-ledger.md)
- [Loop de desenvolvimento](plans/05-loop-de-desenvolvimento.md)
- [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md): plano ativo de providers e modalidades com referência llama.cpp v1.0.0.
- [`docs/plans/07-plano-storage-grafos-falkordb.md`](plans/07-plano-storage-grafos-falkordb.md): etapa de property graph FalkorDB; API de duas camadas, client 0.11.2 e MSRV 1.95 aprovados; conformidade service-real local passou, com limites TLS/cancelamento registrados.

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
