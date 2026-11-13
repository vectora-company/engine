# Plano do loop de desenvolvimento

Este plano define como conduzir trabalho contínuo na Engine sem perder contexto,
contratos ou critérios de conclusão. Use junto com o [checklist de
implementação](../checklists/00-checklist-de-implementacao.md) e a [análise e
referências](../analyses/00-analise-e-referencias-de-desenvolvimento.md). Para
iniciar um goal com instruções compactas, use o [prompt curto](00-prompt-loop-goal.md).
Para criar os artefatos de uma etapa, copie os [templates de plano, análise e
checklist](../templates/README.md).

O pacote operacional atualizado do goal está indexado em
[`docs/goal-loop/README.md`](../goal-loop/README.md); consulte-o para o checklist
do loop e os critérios sucintos de encerramento.

## Fonte de verdade

- `AGENTS.md`: regras do repositório e checks obrigatórios.
- `docs/specs/`: escopo e contratos propostos para cada capability; o índice em
  [`docs/specs/README.md`](../specs/README.md) aponta as referências locais.
- `docs/plans/`: ordem de execução, dependências, gates e estado das etapas.
- `docs/checklists/`: critérios verificáveis de preparação, implementação e
  entrega.
- `docs/analyses/`: investigação, fontes, evidências e decisões.
- `docs/templates/`: modelos para criar esses artefatos por capability sem
  deixar escopo, evidência ou critérios de saída implícitos.
- Fontes locais fora deste repositório: issues em
  `/media/brunosrz/externo/vectora-repos/issues/`, Vectora em
  `/media/brunosrz/externo/vectora-repos/vectora/` e GH Bot no checkout indicado
  pelo `AGENTS.md`. Use os caminhos mapeados nesse arquivo se o workspace mudar.
- Issues locais são requisitos originais quando disponíveis; não substituem as
  specs da Engine.
- Código e testes atuais: evidência do comportamento implementado, nunca
  substituídos por texto histórico.

## Início de cada sessão

1. Confirmar branch/base e working tree. O checkout pode ser compartilhado com
   outra sessão (por exemplo o Codex): rode `git branch --show-current` e
   `git status` antes de agir e não presuma que a branch é a da sessão anterior.
2. Listar os PRs abertos (`gh pr list --repo vectora-company/engine`). Um PR
   aberto é o trabalho ativo e a base da stack; nunca escolha escopo sem
   ter olhado a lista. O `origin` do checkout local pode apontar para um clone
   local desatualizado: confirme o tip real no GitHub antes de empilhar.
3. Para storage, conferir `podman ps -a` (os serviços de teste já existem como
   contêineres `vectora-engine-*`, só precisam de `podman start`).
4. Ler `AGENTS.md`, este plano, a análise, o checklist e a spec/plano ativos.
5. Revalidar no código e nos testes as afirmações documentadas; marcar findings
   antigos como resolvidos, atuais ou não reproduzidos.
6. Selecionar a menor etapa pendente e aprovada no plano ativo.
7. Se não existir etapa, spec, análise ou checklist suficiente, criar os
   documentos aplicáveis a partir dos templates antes de uma implementação
   ampla. Um artefato geral não substitui o checklist específico da etapa.

## Loop por etapa

1. Atualizar objetivo, escopo, consumidores, critérios de aceite e dependências
   no plano/checklist.
2. Resolver contratos e questões de segurança, dados, compatibilidade,
   cancelamento, concorrência e observabilidade conforme a análise.
3. Implementar incrementalmente, core Rust primeiro e bindings correspondentes
   depois.
4. Verificar conforme o checklist; corrigir regressões atribuíveis à etapa.
5. Revisar diff, documentar evidências e atualizar o estado real do plano.
6. Commitar, fazer push e abrir o PR. É obrigatório e não depende de pedido:
   cada etapa vira um PR cuja base é a branch do PR anterior da stack (ver
   [Stack de PRs](#stack-de-prs)). Comece a próxima etapa já em uma nova branch
   empilhada; não espere aprovação nem o resultado de CI do PR anterior.

Não avance para trabalho fora do plano ativo, não faça alterações paralelas em
consumidores sem escopo e não declare paridade ou performance sem medições
reproduzíveis.

Atualize plano, checklist e análise da capability em cada etapa, com o PR,
a evidência obtida e o trabalho pendente. Ao criar um Markdown, no mesmo PR
vincule-o no índice `docs/README.md`, no prompt
`docs/plans/00-prompt-loop-goal.md`, nos documentos do goal que o usam e nos
artefatos da capability que dependem dele. Use o caminho completo a partir
da raiz do repositório no texto do link e um destino relativo válido.
O checklist específico de mídia atual é
[`docs/checklists/04-midia-no-chat.md`](../checklists/04-midia-no-chat.md).

## Stack de PRs

- Cada PR novo usa como base a branch do PR anterior (não `main`), para que o
  mantenedor aprove em cascata. Exemplo (stack verificada em 2026-11-13): `main` ← #73 ← #76 ←
  #77 ← #79 ← #80 ← #81 ← #82 ← #83 ← #84 ← #85 ← #86 ← #87 ← #88 ← #89 ← #90 ← #91 ← #92 ← #93 ← #94. Quando um PR já estiver integrado, o seguinte passa a ter
  como base a branch seguinte da stack ou `main`.
- Faça push para `https://github.com/vectora-company/engine.git` e crie o PR com
  `gh pr create --base <branch do PR anterior> --head <branch nova>`. Título e
  descrição em português; a descrição lista a pilha, o que muda, a evidência
  local e os limites. Siga as regras de metadados em `AGENTS.md`.
- Commit, push e abertura de PR são obrigatórios. Aprovar PRs e criar issues
  só ocorre quando `brunosrz` pedir.
- Depois do push, siga para o próximo PR da stack. Falha de CI no PR anterior
  não interrompe o trabalho: corrija no PR afetado quando fizer sentido e
  continue a stack.
- O hook de pre-commit executa a suíte Rust completa; o tempo depende do cache e
  do host. Registre quais verificações rodaram e quais foram omitidas. O Prettier pode
  reformatar arquivos e abortar o commit. Rode
  `uv run pre-commit run prettier --files <arquivos .md>` antes, repita o
  `git add` se o hook reformatar, e execute commit, push e `gh pr create`
  encadeados em background.

## Equivalência de storage

SQLite/PostgreSQL, LanceDB/Qdrant, Redis/NATS KV, Redis Streams/JetStream e
Redis Pub/Sub/NATS Core formam pares. A base funcional é equivalente: antes de
implementar algo em um backend, consulte a matriz de lacunas em
[`04-plano-storage-ledger.md`](04-plano-storage-ledger.md) e implemente nos dois
lados do par com a mesma suíte de conformidade. Uma capability só fica em um
lado quando a incompatibilidade técnica está verificada na documentação do
projeto e registrada em `capabilities()` (política na
[spec 02](../specs/02-storage-memoria-contexto-rag.md)).

## Execução atual — 13 de novembro de 2026

O PR [#91](https://github.com/vectora-company/engine/pull/91) expõe `flush_outbox`/`pending_count`; #92 projeta evento + checkpoint dos stores SQLite lifecycle/loop/policy para PostgreSQL. O PR [#93](https://github.com/vectora-company/engine/pull/93), integrado, filho do #92, implementa reservation standalone, admission e leituras PostgreSQL. Workspace Rust: 182 testes + 3 fixtures + 1 teste nativo; `storage::` 83/83 contra PostgreSQL 18.6, Redis 8.10.3, NATS 2.15.3 e Qdrant 1.18.0; os testes service-real de reservation/admission passaram. Clippy `-D warnings`, rustdoc, formatação e hooks completos passaram. CI remoto não foi aguardado. A decisão vital foi respondida no chat: `pg_dump`/backup gerenciado para desastre do servidor e verify/rebuild da projeção a partir do SQLite. PR [#94](https://github.com/vectora-company/engine/pull/94), head `984e2d066dcc05b0a98e076361ada632751fc61a` (integrado), implementa recuperação por scope, replay idempotente e comparação de contagens, exigindo pausa dos writers; depois seguem concorrência dos projetores e migrations com rollback. Nesta etapa, o teste PostgreSQL real reconstruiu eventos, checkpoint, reservation/admission e reserva standalone após apagar a scope isolada; Python 53/53 e TypeScript 39/39 passaram (10 testes de serviço ignorados sem endpoints). `storage::` teve 81 aprovações e 3 falhas restritas ao contêiner NATS 2.10, incompatível com o mínimo 2.11 do TTL; PostgreSQL/Redis/Qdrant passaram.

A decisão vital sobre bytes inline foi respondida no chat: separar o tipo
persistível do envelope efêmero. Implementação publicada no PR
[#90](https://github.com/vectora-company/engine/pull/90), filho do #89, na
branch `feat/chat-ephemeral-media-request`, commit de implementação
`d56e864286153b6bee1f8cc67e1ffda4c1ba2afb` (OPEN/CLEAN; verificações remotas
não apareceram no momento do registro). O trabalho e seus critérios ficam em
[`docs/plans/03-plano-continuidade.md`](03-plano-continuidade.md),
[`docs/checklists/00-checklist-de-implementacao.md`](../checklists/00-checklist-de-implementacao.md),
[`docs/analyses/00-analise-e-referencias-de-desenvolvimento.md`](../analyses/00-analise-e-referencias-de-desenvolvimento.md),
[`docs/specs/17-midia-no-chat.md`](../specs/17-midia-no-chat.md) e
[`docs/checklists/04-midia-no-chat.md`](../checklists/04-midia-no-chat.md).
Verificação local: workspace Rust 171 testes + 3 fixtures, Clippy e rustdoc;
Python 52 testes; TypeScript build/typecheck e 38 testes (10 testes que exigem
URLs de serviço ignorados); hooks completos passaram. O loop continua pelo
próximo item independente do plano ativo sem aguardar aprovação ou CI remoto.

## Critérios para encerrar o loop

Encerre o loop quando ocorrer uma destas condições:

### Plano ativo concluído

Todos os critérios de aceite e itens obrigatórios do plano ativo estão feitos,
verificados e documentados. Informe os resultados, limitações e estado dos PRs.

### Decisão vital aguardando `brunosrz`

Uma decisão muda contrato público, arquitetura, segurança, dados, compatibilidade,
escopo ou comportamento externo e não pode ser inferida com segurança. Registre
em `docs/analyses/` a pergunta, opções, impactos e trabalho bloqueado e conclua
todo o trabalho independente. Então faça a pergunta a `brunosrz` **no chat**
(não em issue nem em comentário de PR, sem marcá-lo no GitHub) e pare. Retome
somente após a resposta.

### Bloqueio externo sem avanço independente

Uma credencial, serviço, dependência ou ambiente externo impede o trabalho
restante e não há etapa independente útil. Registre o bloqueio, evidências e
condição para retomada; pare até a condição mudar.

Não encerre por conveniência, falta de contexto, PR anterior aguardando
aprovação ou checks de CI em execução ou falhando: reconstrua o contexto pelos
arquivos acima e siga a stack. Não continue após um critério de encerramento até receber nova
orientação ou a decisão necessária.

O PR [#95](https://github.com/vectora-company/engine/pull/95), filho do #94 e integrado, adiciona testes service-real que confirmam `ProjectionMismatch` para linha extra em `ve_ledger_versions` e `ProjectionConflict` para payload divergente. Ambos passaram no PostgreSQL local; Clippy passou. Próxima etapa: concorrência entre projetores.

O PR [#96](https://github.com/vectora-company/engine/pull/96), aberto sobre `main` após integração do #95, testa 16 rodadas service-real com dois projetores independentes drenando simultaneamente duas sequências do mesmo agregado. Todos os resultados foram idempotentes, sem pendências; Clippy passou. Migrations PostgreSQL versionadas e rollback continuam pendentes.
