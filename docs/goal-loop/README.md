# Goal loop da Engine

Este diretório reúne o processo recorrente de trabalho. O objetivo duradouro é
entregar, fase a fase, a EPIC da primeira release completa da Engine. A ordem,
as dependências e os critérios pertencem ao
[`docs/plans/03-plano-continuidade.md`](../plans/03-plano-continuidade.md),
organizado por capacidades. PRs e branches são estado operacional transitório:
consulte GitHub no início de cada sessão e não os use como estrutura do roadmap.

## Como retomar

1. Leia [`AGENTS.md`](../../AGENTS.md), confirme o checkout e preserve as
   alterações locais existentes.
2. Consulte os PRs abertos e suas bases atuais no GitHub; determine a base
   vigente da stack naquele momento, sem presumir um número ou nome de branch.
3. Leia o plano da EPIC e selecione a próxima capability incompleta, respeitando
   dependências e decisões já registradas.
4. Leia os documentos específicos dessa capability: spec, plano, checklist e
   análise. Os caminhos estão no índice geral
   [`docs/README.md`](../README.md) e no plano da EPIC.
5. Implemente e verifique por etapas. Atualize plano, checklist, análise e spec
   com evidências atuais; crie novos Markdown a partir de
   [`docs/templates/README.md`](../templates/README.md) e vincule cada arquivo
   pelo caminho completo a partir da raiz do repositório nos índices e
   documentos gerais que o utilizam.
6. Faça commit, push e abra um PR empilhado na base atual. Inicie a próxima
   etapa sem esperar CI ou aprovação. Não aprove PRs nem crie issues sem pedido.

## Processo e critérios

- [Plano operacional](01-plano-operacional.md): preparação, seleção e execução
  de uma etapa.
- [Checklist do loop](02-checklist-do-loop.md): gates de implementação e
  entrega.
- [Fontes e evidências](03-fontes-e-evidencias.md): como distinguir contrato,
  proposta, código e comportamento verificado.
- [Critérios de encerramento](04-criterios-de-encerramento.md): conclusão da
  EPIC, decisão vital no chat ou bloqueio externo total.
- [`docs/plans/05-loop-de-desenvolvimento.md`](../plans/05-loop-de-desenvolvimento.md):
  protocolo detalhado do goal loop.
- [`docs/checklists/00-checklist-de-implementacao.md`](../checklists/00-checklist-de-implementacao.md)
  e [`docs/analyses/00-analise-e-referencias-de-desenvolvimento.md`](../analyses/00-analise-e-referencias-de-desenvolvimento.md):
  acompanhamento geral da Engine.
- [`docs/checklists/07-checklist-completo-da-engine.md`](../checklists/07-checklist-completo-da-engine.md):
  inventário transversal de capacidades, cobertura e lacunas.
- [`docs/analyses/04-sabatina-engine-ecossistema.md`](../analyses/04-sabatina-engine-ecossistema.md):
  referência viva para reconciliar arquitetura e consumidores.

## Organização do trabalho

O roadmap agrupa trabalho por fases da EPIC. Cada capability deve manter seus
artefatos específicos próximos do domínio e ligados entre si:

- Providers e modalidades: [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md),
  [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](../plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md),
  [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](../checklists/05-providers-e-modalidades-llama-cpp-v1.md)
  e [`docs/analyses/06-auditoria-providers-modalidades.md`](../analyses/06-auditoria-providers-modalidades.md).
- Storage de grafos: [`docs/specs/19-storage-de-grafos-falkordb.md`](../specs/19-storage-de-grafos-falkordb.md),
  [`docs/plans/07-plano-storage-grafos-falkordb.md`](../plans/07-plano-storage-grafos-falkordb.md),
  [`docs/checklists/06-storage-grafos-falkordb.md`](../checklists/06-storage-grafos-falkordb.md)
  e [`docs/analyses/07-auditoria-storage-grafos-falkordb.md`](../analyses/07-auditoria-storage-grafos-falkordb.md).
- CLI e harness local: [`docs/specs/20-engine-cli.md`](../specs/20-engine-cli.md),
  [`docs/plans/08-plano-engine-cli.md`](../plans/08-plano-engine-cli.md),
  [`docs/checklists/08-engine-cli.md`](../checklists/08-engine-cli.md)
  e [`docs/analyses/08-auditoria-engine-cli.md`](../analyses/08-auditoria-engine-cli.md).
- Mídia no chat: [`docs/specs/17-midia-no-chat.md`](../specs/17-midia-no-chat.md)
  e [`docs/checklists/04-midia-no-chat.md`](../checklists/04-midia-no-chat.md).
- Storage, ledger e equivalência entre backends:
  [`docs/specs/02-storage-memoria-contexto-rag.md`](../specs/02-storage-memoria-contexto-rag.md),
  [`docs/plans/04-plano-storage-ledger.md`](../plans/04-plano-storage-ledger.md)
  e [`docs/analyses/05-auditoria-storage-s0-s5.md`](../analyses/05-auditoria-storage-s0-s5.md).

Ao abrir uma nova capability, inclua seus caminhos completos no índice
[`docs/README.md`](../README.md), em
[`docs/plans/03-plano-continuidade.md`](../plans/03-plano-continuidade.md) e
nos documentos gerais do loop que orientam essa capability. Mantenha status
volátil de PR somente no registro específico da etapa ou no estado dinâmico
consultado no GitHub; não replique uma cronologia de PRs nos documentos gerais.

## Consumidores

O GH Bot é o primeiro consumidor experimental. Consulte o checkout mapeado em
[`AGENTS.md`](../../AGENTS.md) e o
[guia de integração da Engine](https://github.com/vectora-company/gh-bot/blob/main/docs/engine-integration.md).
Use localmente os bindings da Engine, implemente capacidades ausentes no core
Rust e integre-as sem duplicar runtime em TypeScript. Valide em conta e
repositórios experimentais, mantendo credenciais e recursos de produção
isolados.
