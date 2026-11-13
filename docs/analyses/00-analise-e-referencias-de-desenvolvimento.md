# Análise e referências para desenvolvimento

Este documento define como investigar uma capability da Engine antes de
implementar ou alterar seus contratos. Ele complementa `AGENTS.md`; não substitui
as regras específicas de uma spec ou de um plano.

## Ordem das fontes

1. `AGENTS.md` e instruções locais dos diretórios que serão alterados.
2. A spec da Engine em `docs/specs/` para a capability em questão.
3. O plano ativo em `docs/plans/` e seu checklist de execução.
4. As referências citadas pela spec, incluindo as cópias locais em
   `../issues/` quando estiverem disponíveis.
5. Implementação atual da Engine e seus testes para verificar o comportamento
   realmente existente.
6. Consumidores relevantes: `../gh-bot/` como primeiro ambiente experimental
   integrado pela binding TypeScript local e `../vectora/` como referência de
   comportamento e consumidor Python posterior. Consulte
   `../gh-bot/docs/engine-integration.md` para o fluxo de desenvolvimento
   conjunto; não condicione esse fluxo à publicação npm.
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

## Etapa multimodal empilhada sobre o PR #77 — 2026-11-13

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
continuava ativando o formato em blocos somente para `ImageUrl`. A condição foi
corrigida para reconhecer o PDF inline. Verificação local concluída: `cargo fmt
--all -- --check`, `cargo test -p vectora-engine --lib chat:: --locked` (44
passaram), Clippy de todos os targets/features com `-D warnings`, Prettier e
`git diff --check` passaram. O PR ainda não foi criado neste registro; a entrega
continua pendente no checklist específico.

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
payload para as modalidades mapeadas; o preparador ainda não está ligado ao
envio real e faltam limite, retry, evidência/evento de roteamento e suporte de
resultado multimodal de tool.

A fronteira de produto deve ser preservada: STT local, permissões do macOS,
configuração UI, geração de prévias e empacotamento SCons estão implementados no
Vectora e não são automaticamente contratos portáveis do core. Devem ser
reavaliados individualmente se outro consumidor demonstrar necessidade e houver
um contrato comum. O plano de continuidade foi atualizado para as próximas
etapas sem copiar essa implementação Python para a Engine.

### Decisão vital pendente — integração do `MediaStore` no envio

A análise do fluxo atual confirma que `prepare_content` existe como função
pura síncrona, mas `OpenAiCompatibleClient` e `NativeChatClient` recebem apenas
`ChatRequest`; nenhum deles recebe `MediaStore` nem `CapabilityResolver`. Portanto,
os adapters não conseguem reidratar `MediaRef` no envio. A stack do Vectora
confirma que a decisão deve acontecer por modelo/turno e que payload, evidência
e UI precisam refletir a mesma rota.

**Pergunta para brunosrz:** qual fronteira pública deve fornecer o `MediaStore`
e a política/capabilities ao envio real?

- **A — contexto efêmero por chamada (recomendação):** adicionar um contexto de
  execução separado de `ChatRequest`, entregue a `complete`/`stream`. Mantém
  referências serializáveis no histórico e evita compartilhar store mutável entre
  chamadas; exige desenhar callbacks/trait assíncrono compatível com Rust, Python
  e TypeScript.
- **B — dependência no cliente:** configurar `MediaStore` e resolver no cliente.
  Simplifica chamadas, mas fixa ownership/lifecycle no cliente e dificulta
  chamadas concorrentes com stores ou permissões diferentes.
- **C — consumidor prepara antes do adapter:** não muda a API de envio, porém
  deixa a reidratação, escolha por turno e evidência fora do core, contrariando a
  meta de decisão e payload coerentes da Engine.

A resposta também determina se a leitura do store pode bloquear (o trait atual é
síncrono) ou precisa ser async para stores de arquivo/rede e bindings não
bloqueantes. Não incluí bytes no `ChatRequest`: isso mistura dados efêmeros com
histórico persistível. Trabalho bloqueado: integrar preparação, limite, retry e
evento de roteamento aos clientes. Permanecem independentes e já concluídos: o
mapeamento de payload do PR #79 e a documentação da evidência Vectora. As
opções e a pergunta estão registradas no PR #80, sem menção ao mantenedor no
GitHub; a resposta é aguardada aqui no chat.

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
