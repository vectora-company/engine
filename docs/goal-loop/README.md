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

Providers e modalidades de IA, com referência normativa à release fixa
[`llama.cpp v1.0.0`](https://github.com/ggml-org/llama.cpp/releases/tag/v1.0.0).
O PR [#96](https://github.com/vectora-company/engine/pull/96) foi integrado em
`main` (`f4a8cf9d7e30d9fb667a7f443f6572067e536d81`); a branch atualizada parte
dessa base. A revisão documental está no PR [#97](https://github.com/vectora-company/engine/pull/97), head `616f558`; o perfil Rust foi aberto como PR filho [#98](https://github.com/vectora-company/engine/pull/98), head `5a463bc`. Os bindings foram abertos como PR [#100](https://github.com/vectora-company/engine/pull/100), filho do #98; Decision Model será a próxima etapa, filha do #100. Leia os artefatos:

- Plano: [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](../plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md).
- Spec: [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md).
- Checklist: [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](../checklists/05-providers-e-modalidades-llama-cpp-v1.md).
- Análise: [`docs/analyses/06-auditoria-providers-modalidades.md`](../analyses/06-auditoria-providers-modalidades.md).
- Plano geral: [`docs/plans/03-plano-continuidade.md`](../plans/03-plano-continuidade.md).

A trilha distingue chat, Decision Models, visão e geração de imagem, entrada e
geração de áudio/vídeo, embeddings e rerank. Compatibilidade OpenAI-compatible
não implica suporte a todas essas operações; cada API/provider/modelo precisa
de evidência e fixtures. Próxima etapa é o adapter HTTP do `llama-server`, sem
compilar/linkar o runtime C++ dentro da Engine.

## Regras operacionais do mantenedor

- Commit, push e PR são obrigatórios; só aprovar PR e criar issue exigem pedido.
- PRs são sempre empilhados: cada um tem como base a branch do anterior, e a
  próxima etapa começa sem esperar aprovação ou CI.
- Um PR aberto é o trabalho ativo: liste os PRs antes de escolher escopo.
- Decisões vitais são perguntadas no chat, não no GitHub.
- Storage segue a equivalência por par descrita na
  [spec 02](../specs/02-storage-memoria-contexto-rag.md).
