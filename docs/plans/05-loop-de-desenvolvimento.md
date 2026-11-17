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

Estado corrente em 16/11/2026: PRs até #112 estão integrados em `main`
(`4140ddeffa646c4dfd2f0e6a7402a1b00e337099`). A capability ativa é o cliente
direto `laya-serve`, em `feat/laya-decision-model-client`; `scons tests` passou
(Rust 227 + 3 fixtures, pytest 95/1 skip, Node 48/11 skips). A etapa foi
publicada como PR [#113](https://github.com/vectora-company/engine/pull/113),
aberto e `MERGEABLE` sobre `main`. Atualize plano, spec, checklist e análise
específicos e este índice a cada etapa. O
estado verificável fica em
[`docs/goal-loop/README.md`](../goal-loop/README.md).

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

Para a etapa FalkorDB solicitada em 2026-11-14, consulte [`docs/specs/19-storage-de-grafos-falkordb.md`](../specs/19-storage-de-grafos-falkordb.md), [`docs/plans/07-plano-storage-grafos-falkordb.md`](07-plano-storage-grafos-falkordb.md), [`docs/analyses/07-auditoria-storage-grafos-falkordb.md`](../analyses/07-auditoria-storage-grafos-falkordb.md) e [`docs/checklists/06-storage-grafos-falkordb.md`](../checklists/06-storage-grafos-falkordb.md). As decisões de API, client 0.11.2 e MSRV 1.95 foram respondidas no chat; implementação inicial foi publicada no PR #102, filho do #101. A auditoria posterior detectou opções de timeout/limite ignoradas pelos bindings. Follow-up ativo em `feat/falkordb-binding-timeouts`, filha do #110; as opções já passaram integração Python/Node contra FalkorDB 6.0.3. Hooks completos e `scons tests` passaram (217 Rust + 3 fixtures, pytest 95, Node 48/10 skips opcionais; integração FalkorDB real Rust/Python/Node). Commit `039798c`, push e PR [#111](https://github.com/vectora-company/engine/pull/111) concluídos; PR filho do #110, base `feat/openai-chat-profile`, `MERGEABLE` na criação.
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

O PR [#96](https://github.com/vectora-company/engine/pull/96) foi integrado em `main` (`f4a8cf9d7e30d9fb667a7f443f6572067e536d81`); suas 16 rodadas de concorrência entre projetores PostgreSQL passaram localmente. Migrations PostgreSQL versionadas e rollback continuam pendentes. A trilha de providers e modalidades de IA continua documentada em [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](06-plano-provedores-e-modalidades-llama-cpp-v1.md), [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md), [`docs/analyses/06-auditoria-providers-modalidades.md`](../analyses/06-auditoria-providers-modalidades.md) e [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](../checklists/05-providers-e-modalidades-llama-cpp-v1.md). A revisão está empilhada nos PRs [#97](https://github.com/vectora-company/engine/pull/97) → [#98](https://github.com/vectora-company/engine/pull/98) → [#100](https://github.com/vectora-company/engine/pull/100); este último expõe o perfil llama-server em Python/TypeScript e passou os hooks locais completos. A etapa de Decision Model `/v1/systemone` começou na branch `feat/llama-server-decision-model`, filha do PR #102; core Rust e wrappers dos dois bindings foram implementados; fixtures passaram; PR [#103](https://github.com/vectora-company/engine/pull/103) publicado como filho do #102 e `MERGEABLE`. Próximo trabalho independente é TTS como filha do #103. A etapa FalkorDB foi documentada no PR #101, filha do #100, e implementada no #102. A API das duas camadas e a dependência/MSRV 1.95 foram decididas; a implementação filha do #101 passou hooks completos (196 testes Rust + 3 fixtures), 92 testes service-real de storage, Python 55 e Node 41 (10 skips sem endpoints de outros serviços). TLS e cancelamento remoto seguem sem evidência. Simulação `scons update --simulate`: Cargo indicou 50 atualizações possíveis compatíveis com Rust 1.95 (9 crates FalkorDB adicionados ao lock), uv indicou Ruff 0.17.5 e ty 0.0.104, pnpm não reportou desatualizados. FalkorDB 6.0.3 passou testes service-real Rust/Python/Node com auth, timeout, limites e restart/AOF; TLS e cancelamento remoto permanecem sem evidência. Atualização real de dependências em 2026-11-15: Cargo atualizou 51 pacotes compatíveis com Rust 1.95; uv atualizou Ruff e ty; pnpm já estava atual. Simulação posterior não identificou updates aplicáveis, com 15 crates atrás do latest no MSRV vigente. `scons lint` e `scons tests` passaram; o teste FalkorDB foi ignorado na rodada pós-update por falta de endpoint, após ter passado contra FalkorDB 6.0.3 antes da atualização.

## Atualização do loop — providers llama.cpp e TTS (2026-11-15)

O PR #102 foi corrigido para usar o store pnpm selecionado localmente pelo checkout (`/home/brunosrz/.local/share/pnpm/store/v11`); o PR #103, Decision Model, foi aberto sobre #102. A branch `feat/llama-server-tts` foi publicada no [PR #104](https://github.com/vectora-company/engine/pull/104), filha de #103, head `8763968`, `MERGEABLE`. O PR implementa TTS bufferizado nas rotas compatível `/v1/audio/speech` e nativa `/tts`, mantendo áudio de referência apenas em request efêmero. A análise, spec, checklist e plano de providers têm os detalhes e evidências em [`docs/analyses/06-auditoria-providers-modalidades.md`](../analyses/06-auditoria-providers-modalidades.md), [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md), [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](../checklists/05-providers-e-modalidades-llama-cpp-v1.md) e [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](06-plano-provedores-e-modalidades-llama-cpp-v1.md). Fixtures Rust e Node, typecheck TS e cargo check dos crates alterados passaram; build maturin e pytest Python (55 passaram, 1 skipped) passaram. Integração llama-server real permanece pendente. A próxima etapa WebSocket/realtime ASR começa empilhada sobre #104. Preserve `bindings/typescript/native/index.d.ts`, alteração local do usuário, fora do commit.

## Atualização do loop — realtime audio, 15 de novembro de 2026

A branch `feat/llama-server-realtime-audio`, filha do PR #104, implementa `/v1/realtime` segundo llama.cpp v1.0.0. Inclui transcription/conversation, PCM16 mono, sample rate nativo ou override, commit, eventos preservados, timeout, limites, backpressure e cancelamento por fechamento. Verificações concluídas: Rust workspace 205 testes + 3 fixtures + teste nativo Python, Clippy `-D warnings`, NAPI release, TypeScript typecheck, Node 41 pass/11 skips, maturin e pytest Python 55 pass/1 skip, Ruff, `ty` e format. A primeira rodada de hooks revelou Clippy `manual_is_multiple_of`, corrigido e revalidado; SVGO/Prettier reformataram arquivos preexistentes fora do escopo, restaurados depois.

Estado atualizado em 15/11/2026: PR #102 segue `MERGEABLE` no último status verificado. Checkout exclusivo `/home/brunosrz/Área de trabalho/engine`; `bindings/typescript/node_modules` é local ao checkout. O PR [#107](https://github.com/vectora-company/engine/pull/107), filho do [#106](https://github.com/vectora-company/engine/pull/106), implementa Ollama nativo. O PR [#108](https://github.com/vectora-company/engine/pull/108), filho do #107, adiciona perfil dedicado OpenRouter Chat Completions com endpoint padrão/customizado, API key, attribution e routing por request; suites completas e hooks passaram, e está `OPEN`/`MERGEABLE`. Branch ativa `feat/openai-responses-profile`, filha do #108, prepara OpenAI Responses em protocolo separado; contrato registrado, implementação de código ainda não iniciada. Os PRs anteriores já têm resultados locais registrados em seus checklists. Preserve `bindings/typescript/native/index.d.ts` local fora do stage. Fixtures Python e Node e Rust workspace (210 testes + 3 fixtures + binding nativo Python), pytest (57 pass/1 skip), Node (43 pass/11 skips), TypeScript typecheck, NAPI release, Clippy, Ruff, `ty` e formatação passaram. Preserve `bindings/typescript/native/index.d.ts` local fora do stage. Detalhes e links completos em [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](06-plano-provedores-e-modalidades-llama-cpp-v1.md), [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md), [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](../checklists/05-providers-e-modalidades-llama-cpp-v1.md) e [`docs/analyses/06-auditoria-providers-modalidades.md`](../analyses/06-auditoria-providers-modalidades.md). Preserve [`bindings/typescript/native/index.d.ts`](../../bindings/typescript/native/index.d.ts), alteração local do usuário, fora do commit. O estado e critérios completos estão em [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](06-plano-provedores-e-modalidades-llama-cpp-v1.md), [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md), [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](../checklists/05-providers-e-modalidades-llama-cpp-v1.md) e [`docs/analyses/06-auditoria-providers-modalidades.md`](../analyses/06-auditoria-providers-modalidades.md).

Atualização posterior do loop (15/11/2026): a captura de tela do #102 mostrava conflito antigo; GitHub agora confirma #101, #102 e #103 `CLEAN`/`MERGEABLE`. O checkout de trabalho permanece `/home/brunosrz/Área de trabalho/engine`, e `bindings/typescript/node_modules` existe nesse checkout. Na branch `feat/openai-responses-profile`, a subetapa 5c implementa OpenAI Responses no core Rust e bindings com `store: false` por padrão, histórico, tools, imagem, reasoning/usage e SSE. Rust workspace (215), Clippy, maturin, pytest e Node (45 pass/11 skips) passaram; detalhes no [plano específico](06-plano-provedores-e-modalidades-llama-cpp-v1.md), [spec](../specs/18-providers-e-modalidades-llama-cpp-v1.md), [checklist](../checklists/05-providers-e-modalidades-llama-cpp-v1.md) e [análise](../analyses/06-auditoria-providers-modalidades.md). Ruff/ty, Prettier/hooks e revisão final do diff ainda faltam antes de publicar o próximo PR filho do #108. A alteração local de `bindings/typescript/native/index.d.ts` e `bindings/python/uv.lock` seguem fora de escopo.

Atualização posterior (15/11/2026): a subetapa Responses foi publicada no [PR #109](https://github.com/vectora-company/engine/pull/109), filho do #108. OpenAI Chat Completions foi publicado como PR [#110](https://github.com/vectora-company/engine/pull/110), filho do #109; testes/hooks locais passaram. A auditoria FalkorDB encontrou que bindings ignoravam timeouts/limites do core. O follow-up `feat/falkordb-binding-timeouts` foi publicado como PR [#111](https://github.com/vectora-company/engine/pull/111), filho do #110, com `GraphStoreSettings` compartilhada, deadlines client/server e limites encaminhados. Hooks e `scons tests` completos passaram: 217 Rust + 3 fixtures, pytest 95 e Node 48/10 skips opcionais, com FalkorDB 6.0.3 real. GitHub reportou `MERGEABLE` ao criar o PR. Em seguida, teste TLS real encontrou um panic por providers Rustls ambíguos (`aws-lc-rs` e `ring`); a correção na branch filha `test/falkordb-tls-integration` seleciona `ring` só se o host não tiver instalado provider. TLS em FalkorDB 6.0.3 passou em Rust/Python/Node com CA local confiável. Cancelamento remoto segue sem garantia do client; ver [plano FalkorDB](07-plano-storage-grafos-falkordb.md), [spec](../specs/19-storage-de-grafos-falkordb.md), [checklist](../checklists/06-storage-grafos-falkordb.md) e [análise](../analyses/07-auditoria-storage-grafos-falkordb.md). Detalhes: [`docs/plans/07-plano-storage-grafos-falkordb.md`](07-plano-storage-grafos-falkordb.md), [`docs/specs/19-storage-de-grafos-falkordb.md`](../specs/19-storage-de-grafos-falkordb.md), [`docs/checklists/06-storage-grafos-falkordb.md`](../checklists/06-storage-grafos-falkordb.md) e [`docs/analyses/07-auditoria-storage-grafos-falkordb.md`](../analyses/07-auditoria-storage-grafos-falkordb.md). Mantenha a alteração local [`bindings/typescript/native/index.d.ts`](../../bindings/typescript/native/index.d.ts) e `bindings/python/uv.lock` fora do stage.
