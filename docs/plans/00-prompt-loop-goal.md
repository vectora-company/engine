# Goal loop

Siga [`AGENTS.md`](../../AGENTS.md) e o [guia do loop](../goal-loop/README.md).
Antes de escolher o trabalho, confira `git branch --show-current`, `git status`,
`gh pr list --repo vectora-company/engine` e, em storage, `podman ps -a`; o PR
aberto no topo da stack é o trabalho ativo. Faça commit, push para o remoto
GitHub e PR obrigatoriamente. Cada PR novo usa a branch do PR anterior como
base; abra a próxima etapa sem esperar aprovação nem CI. Decisões vitais são
perguntadas a brunosrz aqui no chat, sem marcar no GitHub.
Leia o [plano operacional](../goal-loop/01-plano-operacional.md), o
[checklist](../goal-loop/02-checklist-do-loop.md), as
[regras de evidência](../goal-loop/03-fontes-e-evidencias.md) e os
[critérios de encerramento](../goal-loop/04-criterios-de-encerramento.md).
Aplique [`docs/plans/05-loop-de-desenvolvimento.md`](05-loop-de-desenvolvimento.md),
[`docs/checklists/00-checklist-de-implementacao.md`](../checklists/00-checklist-de-implementacao.md)
e [`docs/analyses/00-analise-e-referencias-de-desenvolvimento.md`](../analyses/00-analise-e-referencias-de-desenvolvimento.md).
Use também o [`docs/checklists/07-checklist-completo-da-engine.md`](../checklists/07-checklist-completo-da-engine.md) como inventário transversal de capacidades, gaps, issues e documentos; revalide sua fotografia antes de atualizá-lo.
Retome a próxima etapa do plano ativo e leia sua spec, análise e checklist.
Estado atualizado em 16/11/2026: PR #113 integra cliente Laya; PR #114, filho dele, adiciona embeddings textuais; PR [#115](https://github.com/vectora-company/engine/pull/115), filho do #114, adiciona embeddings multimodais efêmeros por asset map. Branch ativa `feat/llama-server-rerank`, filha do #115, adiciona rerank textual Jina. Gates integrados passaram: Rust 236; pytest 98/1 skip; Node 51/11 skips; `scons lint`. Pre-commit e publicação ainda pendentes. Continue a stack sem esperar CI remoto. Consulte a spec, o plano, a análise e o checklist ativos em [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md), [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](06-plano-provedores-e-modalidades-llama-cpp-v1.md), [`docs/analyses/06-auditoria-providers-modalidades.md`](../analyses/06-auditoria-providers-modalidades.md) e [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](../checklists/05-providers-e-modalidades-llama-cpp-v1.md). Continue a stack sem aguardar CI remoto.

A primeira release deve ser ampla e completa, não um MVP; mantenha os gaps do charter em [`docs/plans/03-plano-continuidade.md`](03-plano-continuidade.md) explícitos. Implemente, verifique localmente e atualize os documentos da etapa.
Leia também a [sabatina do ecossistema](../analyses/04-sabatina-engine-ecossistema.md).
Desenvolva a Engine e o GH Bot conjuntamente: consulte o [guia de integração do
GH Bot](https://github.com/vectora-company/gh-bot/blob/main/docs/engine-integration.md)
e exercite as capabilities no Bot como ambiente experimental controlado, usando
uma conta GitHub nova e repositórios de teste. Durante o desenvolvimento, o Bot
deve consumir localmente `bindings/typescript` do checkout ativo da Engine; não
espere nem exija publicação ou versionamento npm. Quando faltar uma capability,
implemente-a no core Rust, exponha-a pelos bindings necessários, teste-a e
integre-a ao Bot na mesma stack, sem duplicar o runtime em TypeScript. Mantenha
credenciais e repositórios de produção fora desse ambiente.
Mantenha suas respostas sempre atuais: após cada mudança de código, contrato,
teste, plano, referência ou snapshot de consumidor, revise e atualize as Q&As
afetadas, cite evidências e marque claramente implementação, parcialidade,
planejamento, decisão ou ausência. Antes de encerrar cada ciclo, confira o SHA,
a data e se nenhuma resposta confunde spec/roadmap com comportamento verificado.

**Encerre somente** quando o plano ativo estiver concluído, uma decisão vital
estiver documentada e aguardando `brunosrz`, ou um bloqueio externo impedir todo
avanço independente. Nos dois últimos casos, registre o que falta e aguarde.
