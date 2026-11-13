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
Retome a próxima etapa aprovada do plano ativo e leia sua spec, análise e
checklist. Implemente, verifique localmente e atualize os documentos da etapa.
O plano ativo é [`docs/plans/03-plano-continuidade.md`](03-plano-continuidade.md):
siga a próxima etapa indicada e o backlog por fases. Consulte o
[registro da capability ativa](../goal-loop/README.md#capability-ativa), a
[`docs/specs/17-midia-no-chat.md`](../specs/17-midia-no-chat.md) e, para
esta etapa, [`docs/checklists/04-midia-no-chat.md`](../checklists/04-midia-no-chat.md); para
storage, [`docs/analyses/05-auditoria-storage-s0-s5.md`](../analyses/05-auditoria-storage-s0-s5.md)
e [`docs/checklists/03-auditoria-storage-s0-s5.md`](../checklists/03-auditoria-storage-s0-s5.md).
Ao criar um Markdown, inclua no mesmo PR seu caminho completo a partir da raiz
como texto de link neste prompt, em `docs/README.md`, nos documentos do goal
que o usam e nos artefatos da capability que dependem dele.
Leia também a [sabatina do ecossistema](../analyses/04-sabatina-engine-ecossistema.md).
Desenvolva a Engine e o GH Bot conjuntamente: consulte o [guia de integração do
GH Bot](../../../gh-bot/docs/engine-integration.md) e exercite as capabilities no
Bot como ambiente experimental controlado, usando uma conta GitHub nova e
repositórios de teste. Durante o desenvolvimento, o Bot deve consumir
`../engine/bindings/typescript` localmente; não espere nem exija publicação ou
versionamento npm. Quando faltar uma capability, implemente-a no core Rust,
exponha-a pelos bindings necessários, teste-a e integre-a ao Bot na mesma stack,
sem duplicar o runtime em TypeScript. Mantenha credenciais e repositórios de
produção fora desse ambiente.
Mantenha suas respostas sempre atuais: após cada mudança de código, contrato,
teste, plano, referência ou snapshot de consumidor, revise e atualize as Q&As
afetadas, cite evidências e marque claramente implementação, parcialidade,
planejamento, decisão ou ausência. Antes de encerrar cada ciclo, confira o SHA,
a data e se nenhuma resposta confunde spec/roadmap com comportamento verificado.

**Encerre somente** quando o plano ativo estiver concluído, uma decisão vital
estiver documentada e aguardando `brunosrz`, ou um bloqueio externo impedir todo
avanço independente. Nos dois últimos casos, registre o que falta e aguarde.
