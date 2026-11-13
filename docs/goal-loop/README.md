# Goal loop da Engine

Este conjunto define os documentos que orientam um goal contínuo de
implementação. O [prompt curto](../plans/00-prompt-loop-goal.md) é o ponto de
entrada; os documentos abaixo contêm o processo completo.

A [sabatina da Engine e do ecossistema](../analyses/04-sabatina-engine-ecossistema.md)
é uma referência viva: revise as respostas afetadas a cada ciclo e reconcilie
implementação, specs, planos, consumidores, referências e evidências.

## Desenvolvimento conjunto com o GH Bot

O GH Bot é o primeiro consumidor experimental da Engine e deve avançar junto
com ela. Os caminhos dos checkouts locais estão no `AGENTS.md`; não presuma que
sejam diretórios irmãos do checkout da Engine. Durante o desenvolvimento, use
o checkout configurado do GH Bot e o
[guia de integração da Engine](https://github.com/vectora-company/gh-bot/blob/main/docs/engine-integration.md): o
binding TypeScript é consumido diretamente do diretório local da Engine,
sem depender de publicação no npm. Se o fluxo do Bot precisar de uma capability
ausente, implemente o core Rust, os bindings e a integração do Bot na mesma
stack de trabalho. Valide em uma conta GitHub nova e repositórios experimentais
dedicados; mantenha credenciais e repositórios de produção isolados.

## Documentos

- [Plano operacional](01-plano-operacional.md): início de sessão, seleção da
  próxima etapa e atualização dos artefatos.
- [Checklist do loop](02-checklist-do-loop.md): verificações completas por
  etapa, da preparação à entrega.
- [Fontes, referências e evidências](03-fontes-e-evidencias.md): precedência
  documental, análise de código e registro de fatos e decisões.
- [Regras de encerramento](04-criterios-de-encerramento.md): condições exatas
  para concluir ou pausar o loop.

## Documentos de projeto aplicáveis

Em cada retomada, o prompt também aponta para os documentos vigentes da
capability: o plano ativo em `docs/plans/`, a spec em `docs/specs/`, a análise
em `docs/analyses/` e o checklist específico em `docs/checklists/`. Os arquivos
gerais relacionados são o [protocolo detalhado](../plans/05-loop-de-desenvolvimento.md),
o [checklist geral](../checklists/00-checklist-de-implementacao.md) e a
[análise geral](../analyses/00-analise-e-referencias-de-desenvolvimento.md).

## Capability ativa

A capability ativa voltou a ser storage/ledger. Os PRs #92 e #93 ampliam a
projeção SQLite-first do PostgreSQL; o PR [#94](https://github.com/vectora-company/engine/pull/94), filho do #93 e aberto no head `984e2d066dcc05b0a98e076361ada632751fc61a`, implementa rebuild
idempotente e verificação por contagens após perda da projeção. A decisão de
brunosrz no chat combina backup `pg_dump`/gerenciado do PostgreSQL completo com
reconstrução a partir do SQLite autoritativo. Consulte os artefatos da etapa:

- Plano de continuidade: [`docs/plans/03-plano-continuidade.md`](../plans/03-plano-continuidade.md).
- Plano de storage: [`docs/plans/04-plano-storage-ledger.md`](../plans/04-plano-storage-ledger.md).
- Spec: [`docs/specs/02-storage-memoria-contexto-rag.md`](../specs/02-storage-memoria-contexto-rag.md).
- Checklist: [`docs/checklists/03-auditoria-storage-s0-s5.md`](../checklists/03-auditoria-storage-s0-s5.md).
- Análise: [`docs/analyses/05-auditoria-storage-s0-s5.md`](../analyses/05-auditoria-storage-s0-s5.md).
- Análise geral: [`docs/analyses/00-analise-e-referencias-de-desenvolvimento.md`](../analyses/00-analise-e-referencias-de-desenvolvimento.md).
- Loop: [`docs/plans/05-loop-de-desenvolvimento.md`](../plans/05-loop-de-desenvolvimento.md) e checklist geral [`docs/checklists/00-checklist-de-implementacao.md`](../checklists/00-checklist-de-implementacao.md).

Pausar writers é requisito operacional durante verify/rebuild para a comparação
consistir em um ponto estável. Concorrência entre projetores e migrations com
rollback continuam no backlog; não aguardar CI remoto para avançar a stack.

## Regras operacionais do mantenedor

- Commit, push e PR são obrigatórios; só aprovar PR e criar issue exigem pedido.
- PRs são sempre empilhados: cada um tem como base a branch do anterior, e a
  próxima etapa começa sem esperar aprovação ou CI.
- Um PR aberto é o trabalho ativo: liste os PRs antes de escolher escopo.
- Decisões vitais são perguntadas no chat, não no GitHub.
- Storage segue a equivalência por par descrita na
  [spec 02](../specs/02-storage-memoria-contexto-rag.md).
