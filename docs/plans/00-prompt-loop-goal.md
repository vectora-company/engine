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
Retome a próxima etapa do plano ativo e leia sua spec, análise e checklist.
A etapa ativa de execução continua no PR [#100](https://github.com/vectora-company/engine/pull/100), topo da stack de providers. A próxima etapa solicitada por `brunosrz` é adicionar storage de grafos FalkorDB como PR filho do #100, no checkout `/home/brunosrz/Área de trabalho/engine/`. Leia [`docs/specs/19-storage-de-grafos-falkordb.md`](../specs/19-storage-de-grafos-falkordb.md), [`docs/plans/07-plano-storage-grafos-falkordb.md`](07-plano-storage-grafos-falkordb.md), [`docs/analyses/07-auditoria-storage-grafos-falkordb.md`](../analyses/07-auditoria-storage-grafos-falkordb.md) e [`docs/checklists/06-storage-grafos-falkordb.md`](../checklists/06-storage-grafos-falkordb.md). A API pública é uma decisão vital pendente: query OpenCypher específica, property graph portável ou duas camadas. Conclua pesquisa e documentação independente, peça a decisão aqui no chat sem marcar brunosrz no GitHub e pare a implementação dependente até a resposta. Não confunda FalkorDB graph com Redis KV, vetores ou os cinco pares de equivalência da spec 02. Preserve a alteração preexistente em `bindings/typescript/native/index.d.ts` e exclua-a do commit.

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
