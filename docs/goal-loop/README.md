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

Continuação da spec 17: PR #83 revisou a documentação inicial; #84 adicionou
limites por provider/modelo; #85 implementou retry textual único; e #86 substitui
`ToolResult.output` por partes tipadas. Esses PRs estão abertos e empilhados em
ordem. A mídia ainda não está integrada em `main`; catálogo automático e limites
agregados seguem pendentes. Storage e runtime não devem ser anunciados como estáveis.

- Plano de continuidade (plano ativo): [`docs/plans/03-plano-continuidade.md`](../plans/03-plano-continuidade.md).
- Spec da mídia: [`docs/specs/17-midia-no-chat.md`](../specs/17-midia-no-chat.md).
- Checklist desta etapa: [`docs/checklists/04-midia-no-chat.md`](../checklists/04-midia-no-chat.md).
- Storage — plano: [`docs/plans/04-plano-storage-ledger.md`](../plans/04-plano-storage-ledger.md).
- Storage — spec: [`docs/specs/02-storage-memoria-contexto-rag.md`](../specs/02-storage-memoria-contexto-rag.md).
- Storage — checklist: [`docs/checklists/03-auditoria-storage-s0-s5.md`](../checklists/03-auditoria-storage-s0-s5.md).
- Storage — análise: [`docs/analyses/05-auditoria-storage-s0-s5.md`](../analyses/05-auditoria-storage-s0-s5.md).

Atualize o plano, checklist, spec e análise da capability em cada etapa, com
branch/PR, SHA e evidência obtida; não descreva trabalho local como publicado.
A branch `feat/chat-media-rejection-retry` está publicada no PR #85, filha de
`feat/chat-media-provider-limits-docs83` (#84). O PR #86, `feat/chat-tool-result-parts`,
é filho do #85 e implementa a decisão de substituir diretamente `ToolResult` sem
manter v1; a decisão e o contrato constam em `docs/specs/17-midia-no-chat.md`.
A revisão documental está no PR [#87](https://github.com/vectora-company/engine/pull/87),
filho do PR #86. A reclassificação MIME na reidratação está no PR [#88](https://github.com/vectora-company/engine/pull/88), filho do #87; o head é `18c2eca`. A decisão sobre a fronteira de `Inline`/`data:` está documentada no PR [#89](https://github.com/vectora-company/engine/pull/89), filho do #88; veja as opções em `docs/analyses/00-analise-e-referencias-de-desenvolvimento.md`. A implementação aguarda resposta no chat. Ao criar um Markdown, no mesmo PR
vincule-o no índice `docs/README.md`, no prompt
`docs/plans/00-prompt-loop-goal.md`, nos documentos do goal que o usam e nos
artefatos da capability que dependem dele. Use o caminho completo a partir
da raiz do repositório no texto do link e um destino relativo válido.

## Regras operacionais do mantenedor

- Commit, push e PR são obrigatórios; só aprovar PR e criar issue exigem pedido.
- PRs são sempre empilhados: cada um tem como base a branch do anterior, e a
  próxima etapa começa sem esperar aprovação ou CI.
- Um PR aberto é o trabalho ativo: liste os PRs antes de escolher escopo.
- Decisões vitais são perguntadas no chat, não no GitHub.
- Storage segue a equivalência por par descrita na
  [spec 02](../specs/02-storage-memoria-contexto-rag.md).
