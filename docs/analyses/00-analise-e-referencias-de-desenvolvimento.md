# Análise e referências para desenvolvimento

Este documento define como investigar uma capability da Engine antes de
implementar ou alterar seus contratos. Ele complementa `AGENTS.md`; não substitui
as regras específicas de uma spec ou de um plano.

## Ordem das fontes

1. `AGENTS.md` e instruções locais dos diretórios que serão alterados.
2. A spec da Engine em `docs/specs/` para a capability em questão.
3. O plano ativo em `docs/plans/` e seu checklist de execução.
4. As referências citadas pela spec, incluindo as cópias locais em
   `/media/brunosrz/externo/vectora-repos/issues/` quando estiverem disponíveis.
5. Implementação atual da Engine e seus testes para verificar o comportamento
   realmente existente.
6. Consumidores relevantes: GH Bot como primeiro ambiente experimental da
   binding TypeScript local e Vectora como referência de comportamento e
   consumidor Python. Consulte o [guia de integração do GH Bot](https://github.com/vectora-company/gh-bot/blob/main/docs/engine-integration.md)
   para o fluxo conjunto; não condicione o desenvolvimento à publicação npm.
   Os caminhos locais dependem do clone efetivamente disponível; não presuma
   que diretórios irmãos de qualquer checkout tenham o consumidor.
7. Documentação primária das bibliotecas e protocolos externos citados.

## Revalidação de sessão e PR stack

Antes de escolher escopo, liste os PRs abertos com `gh pr list`; o PR no topo é
o trabalho ativo e base do próximo PR.
Confira `git branch --show-current`, `git status` e o SHA local porque o
checkout pode ser compartilhado. Faça commit, push e PR em cada etapa; PRs
novos são empilhados sobre a branch do PR anterior e a próxima etapa começa
sem aguardar aprovação ou CI. Decisões vitais são perguntadas a `brunosrz` aqui
no chat, nunca por marcação em GitHub.

Para storage, inspecione os serviços já instalados com `podman ps -a`, confirme
imagem/versão em execução e configure as variáveis `VECTORA_TEST_*`. Teste
condicional sem variável de serviço não executa integração e deve ser anotado
como `não executado`. Antes de parar/reiniciar um contêiner, confira
`AutoRemove`; não registre credenciais. CI remoto não é evidência desta etapa.

Em storage, aplique primeiro a base funcional equivalente entre os pares
definidos na spec 02. Só registre especialização se a incompatibilidade estiver
verificada para as versões concretas dos backends e sua justificativa estiver
visível em `capabilities()`; uma diferença observada hoje pode ser lacuna, não
uma decisão de produto.

Issues locais e auditorias são material de requisitos e contexto. Não são prova
de que algo já foi implementado, nem autorização para copiar comportamento de
produto para a Engine. Issues abertas no GitHub não substituem os documentos
locais definidos como referência.

## Perguntas obrigatórias da investigação

Antes de abrir uma API ou modificar persistência, registrar respostas para os
itens aplicáveis:

- Qual capability está em escopo e qual explicitamente fica fora?
- Quais consumidores existem hoje e quais estão planejados? Quais linguagens e
  versões dos bindings precisam acompanhar o contrato?
- Quais são os tipos de entrada/saída, invariantes, versões e regras de
  compatibilidade?
- Como erros, timeout, retry, cancelamento, concorrência, ownership e
  idempotência se comportam?
- Quem autentica e autoriza? O que pertence ao host consumidor e o que a Engine
  valida ou persiste?
- Quais dados são duráveis, qual é a fonte da verdade e como funcionam recovery,
  replay e migração?
- Quais logs, eventos, métricas e evidências tornam o comportamento
  diagnosticável sem registrar segredos?
- Qual fallback e rollback mantêm a capability segura durante uma migração
  incremental?
- Quais cenários de conformidade, teste e benchmark sustentam cada afirmação de
  comportamento ou desempenho?

Quando um item não se aplica, anotar por que. Não preencher lacunas com
suposições silenciosas.

## Registro por etapa

Cada etapa de implementação de um plano deve apontar para:

1. spec e requisitos de origem;
2. análise do estado atual, com caminhos e símbolos relevantes;
3. decisão de contrato e alternativas descartadas, quando houver;
4. checklist de arquivos/camadas a alterar e critérios de aceite;
5. testes/checks e evidências obtidas;
6. riscos, limitações, rollback e itens ainda pendentes.

Atualize os documentos junto com o código. O checklist precisa corresponder ao
estado real: diferencie `feito`, `pendente`, `bloqueado` e `não aplicável`, sempre
com uma nota curta para itens bloqueados ou não aplicáveis.

Atualize plano, checklist e análise da capability em cada etapa, com o PR,
a evidência obtida e o trabalho pendente. Ao criar um Markdown, no mesmo PR
vincule-o no índice `docs/README.md`, no prompt
`docs/plans/00-prompt-loop-goal.md`, nos documentos do goal que o usam e nos
artefatos da capability que dependem dele. Use o caminho completo a partir
da raiz do repositório no texto do link e um destino relativo válido.
Para a etapa ativa de mídia, o checklist específico é
[`docs/checklists/04-midia-no-chat.md`](../checklists/04-midia-no-chat.md).

## Registro inicial da etapa multimodal sobre o PR #77 — 2026-11-13

O código-base do PR #77 (`fix/storage-vector-collection-dimensions`) declara
PDF como suportado para `openai` em `chat/media.rs::known_provider`, embora o
adapter `chat/openai_compatible.rs` use Chat Completions e transforme
`ContentPart::Inline` em texto. Os adapters Anthropic e Gemini também só
serializavam `Inline` como texto em `native_providers.rs`. Isso fazia a decisão
`native` divergir do payload de rede.

A correção em desenvolvimento na branch empilhada mapeia PDF para o bloco
`document` base64 da Anthropic, `inlineData` no Gemini e `file.file_data` no
OpenAI Chat Completions. O áudio WAV/MP3 usa `input_audio` no OpenAI; o Gemini
usa `inlineData` para áudio e vídeo. OpenAI Responses continua fora do escopo
porque não há adapter para essa API. Fontes primárias: [PDF na Anthropic](https://docs.anthropic.com/en/docs/build-with-claude/pdf-support), [Part e `inlineData` na API Gemini](https://ai.google.dev/api/generate-content), [formatos suportados no Chat Completions](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create) e [PDF nos dois endpoints OpenAI](https://developers.openai.com/api/docs/guides/file-inputs).

O teste de payload inicialmente falhou porque a seleção de conteúdo Anthropic
ativava o formato em blocos somente para `ImageUrl`; a condição foi corrigida
para reconhecer PDF inline. A verificação local registrada nessa etapa incluiu
fmt, testes `chat::` (44), Clippy, Prettier e `git diff --check`. A frase original
sobre PR ainda não criado descrevia apenas aquele instante: #79 foi aberto, e a
integração do preparo e dos bindings está nos PRs abertos #81 e #82. Consulte o
[plano ativo](../plans/03-plano-continuidade.md) e o
[checklist de mídia](../checklists/04-midia-no-chat.md) para o estado atual.

### Evidência atualizada do Vectora consumidor

A stack de mídia do Vectora contém 19 PRs empilhados (#479, #480, #481, #482,
#483, #486, #487, #488, #490–#494, #496, #497, #499–#502); confirmei no GitHub
que todos estão `MERGED`, incluindo o #502, integrado em 2026-11-13 às 04:41:07
UTC. PRs intermediários ausentes na sequência (#485, #489, #495 e #498) não são
contados como entregas dessa stack. Ver [PRs do Vectora](https://github.com/vectora-company/vectora/pulls).

A implementação do produto resolveu a jornada multimodal do chat e acrescentou
lições concretas para o core: roteamento nativo por modelo com catálogo para
OpenRouter/9router; evidência de roteamento usada tanto pelo backend quanto pela
UI; limites por provider; apenas o turno atual reidrata bytes; tentativa única
em texto somente para rejeição específica de mídia; imagens de ferramentas MCP
como assets; conversão local de áudio/PDF/vídeo; transcrição offline e helper
Speech do macOS com CI que gera fala. Isso não significa que a Engine já tenha
essas capacidades. A integração Rust no PR #81 e os bindings no PR #82
ligaram o preparador ao envio real; o PR #84 adiciona overrides por
provider/modelo/modalidade. O PR #85, filho do #84, implementa retry textual único após rejeição
multimodal classificada. O PR #86, filho do #85, implementa resultados de tool
tipados. Permanecem pendentes catálogo automático confiável, limites agregados
e fixtures comuns.

A fronteira de produto deve ser preservada: STT local, permissões do macOS,
configuração UI, geração de prévias e empacotamento SCons estão implementados no
Vectora e não são automaticamente contratos portáveis do core. Devem ser
reavaliados individualmente se outro consumidor demonstrar necessidade e houver
um contrato comum. O plano de continuidade foi atualizado para as próximas
etapas sem copiar essa implementação Python para a Engine.

### Decisão resolvida — oferecer os três modos de integração

brunosrz decidiu oferecer as três formas de usar o preparo de mídia. A
implementação deve compartilhar o mesmo pipeline Rust e manter `ChatRequest`
sem bytes:

1. **Preparo manual pelo consumidor:** chamar `prepare_content` e enviar as
   partes resultantes pelo método atual. Expor o mesmo helper nos bindings.
2. **Contexto efêmero por chamada:** métodos `complete/stream` aceitam um
   `MediaExecutionContext` para store e resolver, sem persistir essa dependência.
3. **Contexto padrão no cliente:** builder/config do cliente mantém contexto
   padrão; contexto explícito por chamada tem precedência. Dados já preparados
   (`Inline`/imagem resolvida) não são preparados duas vezes.

O `MediaStore` é async; cada asset tem limite padrão configurável de 20 MiB.
O core Rust integra os três modos em OpenAI-compatible/native/fallback e expõe
evidência sem bytes. Os bindings Python/TS recebem mapa temporário
`asset_id → bytes` e oferecem preparo manual, contexto por chamada e padrão.
No Vectora, `AssetStore.read` autoriza por owner/workspace/thread e devolve os
bytes; `media_hydration` limita a reidratação ao turno atual e a imagens recentes
de tools. Referência consultada no Vectora SHA
[`9bbd4dd`](https://github.com/vectora-company/vectora/tree/9bbd4dd49b2f7a468a5431da63c8b8c0d3d3401b):
[`backend/services/assets.py`](https://github.com/vectora-company/vectora/blob/9bbd4dd49b2f7a468a5431da63c8b8c0d3d3401b/backend/services/assets.py)
e [`backend/services/media_hydration.py`](https://github.com/vectora-company/vectora/blob/9bbd4dd49b2f7a468a5431da63c8b8c0d3d3401b/backend/services/media_hydration.py).
Verificações locais desta etapa: build e testes Node, typecheck, build editável e
suíte Python passaram; detalhes no checklist específico.

## Decisões que exigem o mantenedor

Pare antes de implementar a parte dependente quando uma resposta alterar de
modo material:

- contratos públicos, formato de dados ou estratégia de migração;
- segurança, trust boundary, permissões ou execução de efeitos externos;
- suporte anunciado, dependências obrigatórias ou compatibilidade;
- semântica de consistência, retenção, recuperação ou perda de dados;
- escopo do produto, responsabilidade do host ou dependência entre produtos.

Documente a pergunta, opções, impactos e trabalho dependente em `docs/`; peça a
decisão de `brunosrz` aqui no chat. Conclua primeiro o trabalho independente e
encerre o loop somente se não houver outra etapa independente aprovada. Não
transforme uma preferência presumida em decisão arquitetural.

## Revisão documental completa — 2026-11-13

Revisados os 55 arquivos Markdown versionados do repositório. Snapshots de auditorias,
matrizes e planos antigos foram preservados como históricos quando identificam
claramente SHA/data; documentos vigentes foram corrigidos para refletir `main`
(`dc1aa5e3e9357903d85c8bf82bfcb6b9e4cf59d1`), a stack documental aberta até o PR #83 e a implementação então local do #82. A revisão
foi publicada no PR #83, empilhado sobre o #82. Corrigidos, entre outros, o estado dos
payloads nativos, os três modos dos bindings, o status dos PRs e referências
quebradas para o guia do GH Bot. A meta de primeira release ampla foi registrada
no plano como proposta a decompor, sem declarar capabilities planejadas prontas.

Evidência desta revisão: busca de links relativos em todos os Markdown — todos
os destinos resolvem; o hook Prettier rodou em todos os 55 arquivos e passou na
segunda execução; `git diff --check` — passou. Nenhum teste de código
foi executado, pois o escopo deste PR é documental. Ver o andamento no
[`docs/checklists/00-checklist-de-implementacao.md`](../checklists/00-checklist-de-implementacao.md)
e o objetivo/escopo proposto no
[`docs/plans/03-plano-continuidade.md`](../plans/03-plano-continuidade.md).

## Overrides de limite multimodal — 2026-11-13

A branch `feat/chat-media-provider-limits-docs83`, publicada como PR #84
(head `d4ccd3d7ffe8994311acdd274bc4553c7f6b69fc`) sobre o PR documental #83,
implementa `provider_limits` como lista explícita de `{provider, model, limits}`
nos contextos Python e TypeScript. O resolver Rust seleciona limites por
modalidade e par; cada asset continua limitado pelo menor valor entre o teto
global (`max_media_bytes`, padrão de 20 MiB) e o override correspondente.
Ausência de override não presume o limite do provider, e o escopo não inclui
limite agregado da requisição.

Verificação local: 18 testes `chat::media::tests`, a suíte do workspace via
pre-commit, Clippy, fmt e rustdoc passaram; maturin compilou a
extensão Python; o teste Python específico passou após fixar `PYTHONPATH` para o
binding deste checkout; `pnpm build`, `tsc` e o teste Node de mídia passaram.
Um primeiro teste TS falhou porque a fachada não encaminhava `provider_limits`
ao addon; a serialização foi corrigida e o mesmo teste passou. O PR #84 foi integrado e empilhado sobre #83.

Limites oficiais variam com protocolo e método de envio, portanto esta etapa
fornece overrides ao host e não codifica números universais: a documentação
OpenAI separa Images/Vision e File Inputs; Anthropic documenta limites de
requisição e páginas para PDFs; Gemini separa dados inline de Files API. Fontes
primárias: [OpenAI Images and vision](https://developers.openai.com/api/docs/guides/images-vision),
[OpenAI File inputs](https://developers.openai.com/api/docs/guides/file-inputs),
[Anthropic PDF support](https://docs.anthropic.com/en/docs/build-with-claude/pdf-support),
[Gemini Files API](https://ai.google.dev/gemini-api/docs/files),
[Gemini Audio understanding](https://ai.google.dev/gemini-api/docs/audio) e
[Gemini Video understanding](https://ai.google.dev/gemini-api/docs/video-understanding).
A matriz concreta por modelo/endpoint e as restrições agregadas seguem pendentes.

## Retry após rejeição de mídia — 2026-11-13

O PR [#85](https://github.com/vectora-company/engine/pull/85), filho do #84,
implementa na branch `feat/chat-media-rejection-retry` e classifica como
`MultimodalIncompatible` apenas respostas HTTP 400 com marcadores conhecidos de
incompatibilidade de imagem, áudio, vídeo ou PDF. HTTP 401 continua
`Authentication`; falhas de rede, timeout, 413 e erros genéricos não entram no
retry textual.

Clientes OpenAI-compatible e nativos tentam uma vez no mesmo provider/modelo,
preparando o request com as partes em texto/referência. Em streaming a repetição
só ocorre ao abrir o stream, antes de retorná-lo ao host. O wrapper fallback
recebe a mesma regra e só avança de candidato depois de falha do retry.
Verificação local direcionada: 51 testes `chat::` passaram, incluindo
classificação, retry complete/stream OpenAI-compatible e retry de completion no
wrapper fallback. `cargo clippy -p vectora-engine --lib --locked -- -D warnings`,
`cargo fmt --all -- --check`, `git diff --check` e os hooks completos de
pre-commit (incluindo Clippy, suíte Rust workspace e rustdoc) passaram no SHA
`0767636ee88567b19ddce93f1e8d0b15cace396f`.

Contrato e limite: a classificação ainda depende de frases reconhecidas em
mensagens de providers heterogêneos; não há catálogo oficial por endpoint. O
retry não cobre tamanho, autenticação, falha de transporte nem erros SSE após
o stream ter sido entregue. O registro `media_decisions.reason` e o evento
`media_routed` anotam o rebaixamento sem incluir bytes.

## Fronteira de requests persistíveis e envio efêmero — decisão aplicada

brunosrz escolheu no chat a opção 3: separar o request persistível do envelope
transitório de envio, preservando os três modos de integração. `PersistableChatRequest`
rejeita `Inline` e URLs `data:` em JSON; `EphemeralChatRequest` exige envelope
explícito, decodifica base64, confere o tipo pelos bytes, normaliza MIME e impõe
20 MiB por padrão (ou o limite menor configurado no contexto). O limite é
checado antes do transporte. O envelope é temporário e não faz parte do histórico.

Python e TypeScript expõem complete e stream efêmeros para OpenAI-compatible,
providers nativos e fallback. Preparo manual envia o resultado por esse envelope;
contexto por chamada e contexto padrão continuam usando mapas de assets e
referências. O contrato Chat v1 não foi alterado; o envelope de envio tem sua
própria versão 1. A distinção evita o acoplamento acidental de bytes a requests
persistidos sem retirar a forma manual de uso.

Testes locais: testes do core cobrem recusa no persistível, normalização MIME,
limite, categoria e data URL. Testes das bindings verificam que a entrada normal
recusa bytes e que a entrada efêmera exige envelope e limite. Os comandos de
verificação e resultados finais estão registrados no plano de continuidade e no
PR [#90](https://github.com/vectora-company/engine/pull/90), filho do
[#89](https://github.com/vectora-company/engine/pull/89). O PR [#90](https://github.com/vectora-company/engine/pull/90), filho do #89, foi integrado; o commit de implementação é `d56e864286153b6bee1f8cc67e1ffda4c1ba2afb`; os checks remotos ainda não apareceram.

## Diagnóstico da outbox SQLite-first nas bindings — PR #91

A Fase 2, item 6 do plano ativo foi implementada na branch
`feat/storage-outbox-bindings`, PR [#91](https://github.com/vectora-company/engine/pull/91),
filho do PR #90. Python e TypeScript expõem flush limitado (1–1000) e contagem
de eventos locais ainda não projetados; TypeScript converte a contagem para
`bigint`. Sem PostgreSQL configurado, flush válido retorna zero e preserva a
pendência SQLite. Também foi corrigido o núcleo para validar o limite antes do
retorno antecipado sem URL.

Verificação local no commit `4f7e980`: workspace Rust 172 testes + 3 fixtures,
Python 53 testes, Node 39 passaram e 10 dependentes de serviços foram ignorados;
Clippy `-D warnings`, rustdoc e hooks completos passaram. PostgreSQL real estava
disponível para os testes direcionados do ledger. Restam fixtures comuns de
conformidade para KV/filas/vetores e as etapas posteriores de equivalência
PostgreSQL descritas em `docs/plans/03-plano-continuidade.md`.

## Checkpoints dos stores SQLite na projeção PostgreSQL — PR #92

A branch `feat/postgres-lifecycle-projection`, PR
[#92](https://github.com/vectora-company/engine/pull/92), filha do #91, inclui
`checkpoint_json` no outbox (schema SQLite v6) e a flag persistente
`projection_enabled`. `ResilientLedger` com URL PostgreSQL habilita a projeção e
faz backfill do histórico; stores SQLite-only não acumulam cópias. Com a
projeção habilitada, admission e stores lifecycle/loop/policy enfileiram evento

- snapshot atomicamente. `PostgresLedger::project` grava ambos numa transação e
  remove o item da outbox após confirmação. Testes cobrem backfill e retry; um
  teste service-real confirma evento e checkpoint no PostgreSQL.

Verificação atualizada após a correção da fixture v1: workspace Rust 175 testes

- 3 fixtures + 1 teste nativo Python; `storage::` 76/76 com PostgreSQL 18.6,
  Redis 8.10.3, NATS 2.15.3 e Qdrant 1.18.0; 34 testes SQLite focados passaram.
  Clippy `-D warnings`, rustdoc e formatação passaram. Python 53 e Node 39 (10 sem
  endpoints ignorados) são da rodada anterior; bindings não foram alterados.
  `VECTORA_TEST_PODMAN_RESTART` não foi habilitado para Redis/NATS/Qdrant.

A projeção ainda não espelha a linha consultável de `ve_run_reservations`;
backup/verify do PostgreSQL, APIs de leitura e concorrência entre projetores
continuam pendentes. Ver [`docs/plans/03-plano-continuidade.md`](../plans/03-plano-continuidade.md),
[`docs/plans/04-plano-storage-ledger.md`](../plans/04-plano-storage-ledger.md) e
[`docs/checklists/03-auditoria-storage-s0-s5.md`](../checklists/03-auditoria-storage-s0-s5.md).

## Reclassificação MIME na reidratação — implementação local

A reidratação compara a assinatura dos bytes já verificados (tamanho e SHA-256)
com a modalidade do `ContentPart`. Se a modalidade coincide, o payload e a
`MediaDecision` usam o MIME detectado, não o MIME fornecido pelo host. Se diverge,
a parte é rebaixada para texto/referência; bytes de PDF não são enviados como
imagem/áudio, e previews só são enviados quando os bytes identificam imagem.
Documentos nativos só seguem quando a assinatura identifica PDF.

Verificação local: 22 testes `chat::media::tests` passaram, incluindo casos de
MIME de imagem adulterado, MIME genérico em PDF e modalidade divergente. Format, Clippy, testes Rust workspace e rustdoc passaram nos hooks; os 22 testes
focados passaram. A mudança foi publicada no PR [#88](https://github.com/vectora-company/engine/pull/88), filho do #87, head `18c2eca`.

## Partes multimodais em `ToolResult` — decisão e implementação

brunosrz escolheu substituir diretamente `output: String` por `parts`, sem
manter v1 nem conversores legados. A implementação atual no PR [#86](https://github.com/vectora-company/engine/pull/86), branch
`feat/chat-tool-result-parts` filha do PR #85,
define partes `text` e `image` por `MediaRef`, expõe a conversão para mensagem
de tool pelo núcleo Rust nos bindings, e mapeia imagem a Anthropic/Gemini/OpenAI
Chat Completions. Cohere mantém forma textual porque não declara suporte para
imagens em resultado de tool. O loop sobe para contrato v3 e `ToolResult` para
v2; formatos antigos são rejeitados.

Verificação local: 163 testes Rust da lib passaram, incluindo rejeição do
contrato antigo e conversão para mensagem; `cargo clippy --workspace
--all-targets --all-features --locked -- -D warnings` e `cargo check --workspace
--all-targets --all-features --locked` passaram; build, typecheck e 37 testes Node
passaram (10 testes de serviços foram ignorados sem endpoints); a suíte Python
passou. Todos os hooks de pre-commit passaram, incluindo ty após atualizar os stubs de `_native.pyi`. O PR #86 foi aberto com head `b188a94`.

Os limites do loop contam a representação JSON serializada (texto e metadados da
referência); os bytes da imagem ficam no store do host e obedecem aos limites de
reidratação do contexto de mídia. A implementação não adiciona store persistente
à Engine.

## Reservation/admission na projeção PostgreSQL — branch filha #93

A branch `feat/postgres-admission-projection`, filha do PR #92, adiciona schema
SQLite v8, outbox dedicada para reservation standalone, metadados de admission
na outbox de eventos, backfill único e leitura de reservation/admission no
`PostgresLedger`. Admission + reservation + evento + checkpoint são gravados na
mesma transação PostgreSQL; reservation standalone é projetada idempotentemente.

Verificação local após implementação: 182 testes Rust + 3 fixtures + 1 teste nativo Python;
`storage::` 83/83 contra PostgreSQL 18.6, Redis 8.10.3, NATS 2.15.3 e Qdrant
1.18.0. Testes de migração v1/v6, backfill, projection e leitura de admission passaram
com PostgreSQL real. Clippy `-D warnings`, rustdoc, formatação e hooks completos
passaram. Ainda faltam backup/verify PostgreSQL, concorrência entre projetores
e migrations PostgreSQL com rollback.

## Decisão respondida: backup e recuperação da projeção PostgreSQL

O SQLite continua a autoridade e cópia local; perda de conexão PostgreSQL mantém
writes locais em outbox e reconecta, conforme testes reais. Porém, se o banco
PostgreSQL inteiro for perdido/recriado depois de uma projeção concluída, abrir
`ResilientLedger` não refaz automaticamente o histórico: os marcadores de
backfill permanecem concluídos e itens confirmados foram removidos da outbox.
Não há hoje API de rebuild/verificação integral.

| Opção                   | Contrato                                                                                                                                                                                | Vantagem                                                                                    | Custo/limite                                                                                                                                                                      |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| PostgreSQL nativo       | Operador usa `pg_dump`/`pg_restore` ou backup gerenciado; Engine documenta e valida esquema/conectividade.                                                                              | Formato e recuperação maduros, suporta bancos grandes e inclui objetos nativos.             | Depende de tooling e operação do host; não confere sozinho a equivalência SQLite↔PostgreSQL nem restaura só a projeção em banco compartilhado.                                    |
| Backup lógico da Engine | Export/import versionado das tabelas Engine, com snapshot consistente e verificação de conteúdo.                                                                                        | Portável e seletivo; pode verificar/reconstruir a projeção a partir do SQLite autoritativo. | Cria formato e compatibilidade próprios, exige streaming, permissões e contrato de conflito/restore; não substitui o backup do banco completo.                                    |
| Combinado (recomendado) | Backup físico/lógico nativo do PostgreSQL para desastre do servidor; Engine oferece `verify_projection` e `rebuild_projection` replayando SQLite, sem inventar outro formato de backup. | Usa recovery nativo e mantém a projeção reconstruível pela fonte de verdade local.          | Requer definir escopo do verificador (contagens/hash/eventos/checkpoints/reservas), bloqueio/consistência durante rebuild e semântica quando o banco contém dados fora da Engine. |

Decisão de brunosrz no chat: combinar backup PostgreSQL nativo (`pg_dump`/backup gerenciado) com API Engine `verify_projection`/`rebuild_projection` que reenvia o SQLite autoritativo, sem formato de backup próprio. O PR [#94](https://github.com/vectora-company/engine/pull/94), integrado em `main` (`f4a8cf9d7e30d9fb667a7f443f6572067e536d81`), implementa rebuild por scope e verificação das cinco contagens de agregados/versões, eventos, checkpoints, reservas e admissions. Writers devem ser pausados durante a operação; a comparação não representa snapshot distribuído e não cobre conteúdo extra que esteja fora das scopes do SQLite. O teste service-real PostgreSQL apaga e reconstrói uma scope exclusiva com eventos, checkpoint, reservation/admission e reservation standalone. Na suíte `storage::`, 81 passaram e 3 falharam contra o contêiner NATS 2.10 local, inferior ao mínimo 2.11 do contrato TTL; essa falha ambiental não está relacionada ao PR. Referências de implementação e estado:
[`docs/plans/03-plano-continuidade.md`](../plans/03-plano-continuidade.md),
[`docs/plans/04-plano-storage-ledger.md`](../plans/04-plano-storage-ledger.md),
[`docs/checklists/00-checklist-de-implementacao.md`](../checklists/00-checklist-de-implementacao.md) e
[`docs/analyses/05-auditoria-storage-s0-s5.md`](05-auditoria-storage-s0-s5.md).

O PR [#95](https://github.com/vectora-company/engine/pull/95), filho do #94, integrado em `main` (`f4a8cf9d7e30d9fb667a7f443f6572067e536d81`), cobre as condições negativas da verificação: linha extra na tabela de versões retorna `ProjectionMismatch`; payload divergente para evento existente retorna `ProjectionConflict` e mantém a outbox local pendente. Os dois testes passaram contra PostgreSQL real.

O PR [#96](https://github.com/vectora-company/engine/pull/96), integrado em `main` (`f4a8cf9d7e30d9fb667a7f443f6572067e536d81`), registra 16 rodadas service-real com dois handles de `ResilientLedger` independentes processando a mesma outbox de duas sequências de um agregado. Nenhum evento foi duplicado, ambas as operações terminaram sem erro e não restaram pendências. Isso não equivale ainda a concorrência entre processos independentes; essa cobertura e admissions concorrentes seguem pendentes.

## Providers e modalidades — fotografia após o PR #96

Artefatos ativos: [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](../plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md), [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md), [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](../checklists/05-providers-e-modalidades-llama-cpp-v1.md) e [`docs/analyses/06-auditoria-providers-modalidades.md`](06-auditoria-providers-modalidades.md).

`main` está em `f4a8cf9d7e30d9fb667a7f443f6572067e536d81`. O PR [#96](https://github.com/vectora-company/engine/pull/96) foi integrado; a revisão documental está no PR [#97](https://github.com/vectora-company/engine/pull/97), head `616f558`. O código-base tem chat OpenAI-compatible com streaming, adapters nativos Anthropic/Google/Cohere, fallback, embeddings OpenAI-compatible/OpenRouter/Cohere/Voyage/Ollama e rerank Cohere/Voyage/OpenRouter. Não existe no `main` perfil/adaptor llama.cpp, discovery de modelos, Decision Model inferencial, TTS, ASR/STT ou geração de imagem. O perfil Rust com health/discovery e chat/stream compartilhado está no PR filho [#98](https://github.com/vectora-company/engine/pull/98), head `5a463bc`; os bindings são expostos pelo PR filho [#100](https://github.com/vectora-company/engine/pull/100), com fixtures locais nos dois runtimes; integração real permanece pendente. Os tipos e o `MediaExecutionContext` com mapa temporário de assets não provam suporte de transporte para todos os providers.

A auditoria específica [`docs/analyses/06-auditoria-providers-modalidades.md`](06-auditoria-providers-modalidades.md) fixa a release [`llama.cpp v1.0.0`](https://github.com/ggml-org/llama.cpp/releases/tag/v1.0.0) e a [API HTTP do llama-server nessa tag](https://github.com/ggml-org/llama.cpp/blob/v1.0.0/tools/server/README.md). Ela registra diferenças relevantes: `/v1/systemone` não faz streaming; `/v1/audio/speech` produz WAV/PCM com voz e sample rate dependentes do modelo; `/v1/realtime` é WebSocket para áudio ao vivo, não batch STT; chat/embeddings podem aceitar entrada de imagem, áudio e vídeo; `input_video` depende de FFmpeg; rerank tem formatos Jina/TEI distintos. A API de discovery pode retornar `text` como fallback quando faltam metadados e não deve ser confundida com confirmação de modalidade. Geração de imagem não tem endpoint documentado nessa versão do servidor.

`llama.app` distingue também Decision Models e modelos de embeddings de chat; seu catálogo serve como contexto do produto local e não prova cobertura do `llama-server` nem da Engine. Fontes e limites por operação estão no plano ativo e nos quatro artefatos com caminhos completos listados em `docs/README.md`. Nenhuma compatibilidade de OpenRouter, 9router, OmniRouter, OpenAI, Kimi ou Ollama foi inferida pelo nome: transporte OpenAI-compatible exige fixtures por rota, headers, erros, stream e modalidade.
