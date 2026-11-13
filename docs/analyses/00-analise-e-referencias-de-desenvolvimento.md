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
essas capacidades. Esta etapa Engine fecha somente o defeito de formato do
payload para as modalidades mapeadas. No snapshot inicial, o preparador ainda
não estava ligado ao envio real; esse estado foi superado pela integração Rust
no PR #81 e pelos bindings no PR #82. Continuam pendentes limites por
provider/modelo, retry específico e resultado multimodal de tool.

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
(`dc1aa5e3e9357903d85c8bf82bfcb6b9e4cf59d1`), a stack documental aberta até o PR #83 e a implementação local do #82. A revisão
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

A branch `feat/chat-media-provider-limits-docs83`, filha do PR documental #83,
implementa `provider_limits` como lista explícita de `{provider, model, limits}`
nos contextos Python e TypeScript. O resolver Rust seleciona limites por
modalidade e par; cada asset continua limitado pelo menor valor entre o teto
global (`max_media_bytes`, padrão de 20 MiB) e o override correspondente.
Ausência de override não presume o limite do provider, e o escopo não inclui
limite agregado da requisição.

Verificação local: 18 testes `chat::media::tests` passaram; maturin compilou a
extensão Python; o teste Python específico passou após fixar `PYTHONPATH` para o
binding deste checkout; `pnpm build`, `tsc` e o teste Node de mídia passaram.
Um primeiro teste TS falhou porque a fachada não encaminhava `provider_limits`
ao addon; a serialização foi corrigida e o mesmo teste passou.

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
