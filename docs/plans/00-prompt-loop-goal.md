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
Estado atualizado em 16/11/2026: PRs #113–#119 adicionaram cliente Laya, embeddings e rerank llama-server progressivamente; PR [#120](https://github.com/vectora-company/engine/pull/120) valida respostas de rerank e está publicado sobre #119. Branch ativa `feat/engine-cli`, filha do #120: crate separada de configuração/chat ReAct e Decision Model via Laya standalone ou llama.cpp/Laya-GGUF. Cinco testes CLI, smoke init/set/validate/show, `scons tests` (242 Rust, 3 fixtures, 1 teste binding Python, pytest 99/1 skip, Node 52/11 skips), `scons lint`, pre-commit e `scons build-cli` passaram. O binário release e `--help` foram verificados; criar commit, enviar ao GitHub e publicar PR filho do #120 sem esperar CI remoto. Os documentos específicos e links completos constam em `docs/specs/20-engine-cli.md`, `docs/plans/08-plano-engine-cli.md`, `docs/checklists/08-engine-cli.md` e `docs/analyses/08-auditoria-engine-cli.md`. Leia spec, plano, análise e checklist específicos vinculados neste goal; continue a stack sem esperar CI remoto.

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
Artefatos específicos desta etapa: `docs/specs/20-engine-cli.md`, `docs/plans/08-plano-engine-cli.md`, `docs/checklists/08-engine-cli.md` e `docs/analyses/08-auditoria-engine-cli.md`.
