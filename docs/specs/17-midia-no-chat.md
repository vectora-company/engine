# Contrato de mídia no chat

**Estado:** parcialmente implementada no código dos PRs abertos #79–#82; ainda
não integrada em `main`. Tipos, classificação por bytes, capabilities,
decisões por parte, payloads nativos e preparo Rust estão implementados para os
protocolos descritos abaixo. Retry textual único em incompatibilidade multimodal
está implementado no PR aberto [#85](https://github.com/vectora-company/engine/pull/85),
filho do #84. O PR #82 expõe preparo manual, contexto por
chamada e contexto padrão nos bindings Python e TypeScript. O mapa temporário é
`asset_id → bytes`; autorização e persistência dos assets continuam sob
responsabilidade do host. O limite global padrão é 20 MiB por asset e pode ser
configurado; a preparação atual cobre o turno de usuário mais recente e os
resultados subsequentes de tools, e devolve evidências sem bytes. Overrides de limites por provider/modelo/modalidade já são aceitos no contexto
de execução e limitados pelo teto global por asset; descoberta automática dos
limites, limites agregados de request e resultados de tool em partes seguem
pendentes. A stack [#479–#502](https://github.com/vectora-company/vectora/pulls)
do Vectora foi integrada em 19 PRs, mas não prova paridade da Engine. UI, STT
local, prévias e empacotamento permanecem no Vectora. Veja o estado e os critérios
em [`docs/checklists/04-midia-no-chat.md`](../checklists/04-midia-no-chat.md).

Este documento define como o chat da Engine representa mídia (imagem, áudio,
vídeo, documento e arquivo genérico) em mensagens e em resultados de tools, como
decide o que cada provider recebe e o que o ledger guarda. Ele não migra o chat do
Vectora para a Engine: a adoção é capability por capability, com fallback e
rollback.

## Situação atual da Engine

- `ContentPart` (`crates/vectora-engine/src/chat/types.rs`) inclui texto,
  imagem por URL, mídia por referência e mídia inline na cópia do request.
- O resultado de uma tool é uma `String`; os adapters nativos achatam o resultado
  em texto. O contrato atual não representa imagens de tool como partes tipadas;
  esse suporte segue pendente em todos os adapters.
- O core resolve modalidades por provider e aceita overrides de limites de bytes
  por provider/modelo/modalidade. Ainda não consulta catálogo automático nem
  aplica limites totais de request, duração ou resolução.
- `MediaStore` assíncrono e `MediaExecutionContext` existem no core. Os bindings
  recebem um mapa efêmero de bytes indexado por `asset_id`; no Vectora o host
  alimenta esse mapa após `AssetStore.read` validar usuário/workspace/thread.
  `VerificationEvidence.artifact_ref` atende a outro contrato e não substitui
  o store de mídia do consumidor.

## Integração com o envio

Os bindings Python e TypeScript expõem três modos: preparar partes manualmente
com `prepare_chat_content`/`prepareChatContent`; fornecer `MediaExecutionContext`
na chamada `complete/stream`; ou configurar o contexto padrão no cliente. O
contexto explícito prevalece sobre o padrão. O mapa temporário é
`asset_id → bytes`, e o host só deve passar assets que já autorizou. Partes
preparadas não são processadas de novo. `ChatRequest` permanece sem bytes
persistidos; o limite padrão configurável é 20 MiB por asset; somente a mensagem
de usuário atual e resultados posteriores de tools são reidratados. Completion
e stream reportam decisões sem bytes. `provider_limits` aceita entradas com
`provider`, `model` e `limits` (`image_bytes`, `audio_bytes`, `video_bytes`,
`document_bytes`, `file_bytes`). O menor valor entre o limite da modalidade e
`max_media_bytes` governa cada asset; ausência de override usa só o teto global
e não presume limite universal do provider. Estado e precedência:
[`docs/analyses/00-analise-e-referencias-de-desenvolvimento.md`](../analyses/00-analise-e-referencias-de-desenvolvimento.md).

## Responsabilidades

- O consumidor fornece os bytes e o armazenamento da mídia (um store endereçado por
  conteúdo) por uma interface tipada. A Engine não define onde os arquivos vivem,
  não faz download de URL e não decide quem pode ler um asset: a autorização do
  acesso é do consumidor.
- A Engine classifica, decide a forma de envio por provider, monta o request e
  registra a decisão. Ela não executa STT, OCR, renderização de PDF nem
  extração de quadros de vídeo: essas conversões são capacidades do consumidor, que
  devolve a forma textual ou as imagens derivadas já prontas.
- STT, TTS e geração de imagem ou vídeo continuam fora deste contrato.

## Parte de conteúdo com mídia

`ContentPart` ganha as variantes `Image`, `Audio`, `Video`, `Document` e `File`.
Cada uma leva:

| Campo       | Tipo             | Observação                                                                          |
| ----------- | ---------------- | ----------------------------------------------------------------------------------- |
| `media_ref` | `MediaRef`       | Referência do asset; sem bytes.                                                     |
| `text`      | `Option<String>` | Forma textual: texto extraído, transcrição ou nota com nome e tipo.                 |
| `previews`  | `Vec<MediaRef>`  | Imagens derivadas (páginas de PDF, quadros de vídeo), quando o consumidor as gerou. |

`MediaRef`:

| Campo        | Tipo     | Observação                                                           |
| ------------ | -------- | -------------------------------------------------------------------- |
| `asset_id`   | `String` | Identificador opaco no store do consumidor.                          |
| `sha256`     | `String` | Digest do conteúdo; conferido ao ler o asset.                        |
| `mime_type`  | `String` | Tipo detectado pelos bytes, não o declarado pelo cliente.            |
| `size_bytes` | `u64`    | Tamanho original.                                                    |
| `name`       | `String` | Nome para exibição e para a forma textual; nunca usado como caminho. |

`ImageUrl` continua existindo para o caminho OpenAI-compatível com URL ou `data:`
já resolvida pelo chamador; a variante `Image` com `MediaRef` é a forma canônica
persistida.

Regras:

- **Classificação pelos bytes.** A assinatura do conteúdo prevalece sobre MIME e
  extensão declarados. Tipos conhecidos: `image`, `audio`, `video`, `document`
  (PDF e formatos Office/ODF), `text` e `file` (qualquer outro). Nenhum tipo é
  recusado por não ser conhecido; o que não tem forma nativa nem textual vira
  referência com nome, tipo e tamanho.
- **Sem binário no histórico.** O ledger, os checkpoints e o histórico guardam
  `MediaRef` e a forma textual. Os bytes entram (inline ou por upload do provider)
  somente na cópia do request montada para enviar ao provider e não são
  persistidos.
- **Reidratação com verificação.** Antes de enviar, a Engine pede os bytes ao
  store do consumidor e confere `sha256` e tamanho. Digest divergente ou asset
  ausente nunca é erro silencioso: a parte vira uma nota textual (`[anexo
indisponível: nome]`) e a decisão registra o motivo.
- **Só o turno atual reidrata.** Mídia de mensagens antigas segue como forma
  textual ou nota, para limitar custo e superfície. O consumidor pode ampliar para
  as N imagens mais recentes de resultados de tools.

## Resultado de tool com partes

`ToolResult.output` é atualmente uma `String`; a extensão multimodal está
planejada, mas sua forma pública ainda depende de decisão de compatibilidade.
A proposta funcional é representar texto e imagem (`MediaRef`) como partes e
manter uma forma textual legível pelo loop e pelo ledger.

### Decisão pendente — compatibilidade de `ToolResult`

Antes de alterar o contrato Rust/ Python/TypeScript, escolha uma estratégia:

1. **Mudança direta:** substituir `output: String` por `parts: Vec<ToolResultPart>`.
   Modelo mais simples, mas quebra consumidores Rust e formatos serializados
   existentes.
2. **Extensão aditiva:** manter `output: String` e acrescentar `content` opcional
   com partes tipadas. Mantém compatibilidade de leitura, mas exige definir qual
   campo é canônico e como texto e partes permanecem consistentes.
3. **Contrato versionado novo:** manter `ToolResult` v1 e criar v2 com partes;
   migração explícita e sem ambiguidade, ao custo de duas APIs e conversores
   durante a transição.

A escolha afeta API pública, serialização, persistência/replay e os dois bindings;
por isso a implementação fica parada até a decisão. Nenhuma alternativa está
selecionada.

Mapeamento por provider:

| Provider / protocolo                                        | Texto do resultado                    | Imagem do resultado                                                               |
| ----------------------------------------------------------- | ------------------------------------- | --------------------------------------------------------------------------------- |
| Anthropic Messages                                          | `tool_result` com `content` em partes | Bloco `image` dentro do `tool_result`.                                            |
| Google Gemini                                               | `functionResponse`                    | Parte inline (`inlineData`) no mesmo turno do `functionResponse`.                 |
| OpenAI Responses                                            | `function_call_output`                | Item de entrada com imagem logo após o `function_call_output`.                    |
| OpenAI Chat Completions e compatíveis (OpenRouter, 9router) | mensagem `tool`                       | Mensagem de `user` seguinte com a imagem, pois a mensagem `tool` aceita só texto. |
| Cohere Chat v2                                              | mensagem `tool`                       | Sem imagem em resultado de tool: a imagem vira forma textual com a referência.    |

Quando o modelo não declara `tool_result_images`, a imagem do resultado vira a
forma textual com a referência, e a decisão é registrada.

## Capability por modalidade

Cada modelo resolve um conjunto de capabilities: `vision`, `pdf`, `audio_input`,
`video_input` e `tool_result_images`, cada uma `suportado`, `não suportado` ou
`desconhecido`. A resolução é em camadas, da mais específica para a mais geral:

1. override do consumidor para o modelo;
2. tabela de modelos e protocolos conhecidos;
3. metadado de modalidades do gateway (OpenRouter, 9router), quando o consumidor
   o fornece;
4. padrão conservador: só `vision` quando o provider é conhecido por aceitar
   imagem; todo o resto `desconhecido`.

`desconhecido` é tratado como não suportado para efeito de envio nativo, e o
consumidor pode sobrescrever. A capability precisa corresponder ao protocolo
real do adapter: OpenAI Chat Completions usa `file.file_data` para PDF e
`input_audio` para WAV/MP3; OpenAI Responses API ainda não tem adapter na
Engine.

## Decisão por parte

Para cada parte de mídia a Engine escolhe um modo, nesta ordem:

1. **`native`**: o provider recebe o arquivo no formato do protocolo, quando o
   modelo declara a capability (PDF nativo para Anthropic, Gemini e OpenAI; áudio e
   vídeo nativos para Gemini; imagem onde há `vision`).
2. **`image`**: o modelo lê imagem mas não o arquivo; o consumidor fornece
   `previews` (páginas de PDF, quadros de vídeo) e elas seguem como imagens. A
   Engine só usa `previews` já existentes.
3. **`text`**: a forma textual do arquivo (texto extraído, transcrição ou nota).
4. **`reference`**: sem forma textual; nome, tipo e tamanho, para o modelo saber
   que o anexo existe.

A função que prepara o conteúdo por provider devolve, junto do conteúdo, um
registro por parte: `{ part_index, name, mime_type, size_bytes, sha256, mode,
reason, model }`. O host grava esse registro como evidência do run. O digest e o
nome aparecem; o conteúdo, não.

## Recuperação

Se a resposta HTTP do provider for classificada como `MultimodalIncompatible` por
uma mensagem reconhecida de incompatibilidade multimodal, a Engine rebaixa as
partes para sua forma textual e repete **uma vez no mesmo provider/modelo**. A
regra vale para chamada completa e para abertura de stream antes de o stream ser
entregue ao host. Depois que o stream foi aberto, nenhum erro de evento pode
provocar repetição, pois saída parcial pode já ter sido observada. Erros de
autenticação, conexão, timeout, tamanho excedido e falhas genéricas não acionam
essa repetição. O erro final continua tipado. O fallback entre modelos permanece
uma decisão separada do contrato de chat fallback (spec 06). A completion ou o
evento `media_routed` registra que houve rebaixamento e nova tentativa.

## Erros

- `MediaUnavailable`: asset ausente ou com digest divergente; vira nota textual,
  não aborta o turno.
- `MediaTooLarge`: acima do limite do consumidor ou do provider; rebaixa para o
  modo seguinte.
- `MultimodalIncompatible`: já existente; dispara a recuperação.

Nenhum erro inclui bytes, URL assinada ou caminho local.

## Segurança

- O MIME declarado pelo cliente nunca decide o tipo; a assinatura dos bytes decide.
- O nome do arquivo é dado, nunca caminho: a Engine não o usa para abrir arquivo.
- Conteúdo de mídia (texto extraído, transcrição, legenda de imagem) é dado não
  confiável: não concede permissão, não altera política e não é tratado como
  instrução da aplicação.
- URLs de mídia remotas não são baixadas pela Engine; o consumidor resolve e aplica
  a própria proteção contra acesso a rede interna.

## Conformidade

Os casos abaixo são o piso para marcar o contrato como implementado, nos três
bindings (Rust, Python, TypeScript), com as mesmas fixtures:

- Classificação por bytes: PNG com extensão `.txt`, PDF com MIME de imagem, arquivo
  desconhecido (vira `file`).
- Persistência: o histórico serializado não contém base64 nem bytes; reler com
  digest adulterado produz a nota e o registro de decisão.
- Mapeamento por provider: o corpo enviado para cada protocolo da tabela de tools,
  com e sem `tool_result_images`.
- Decisão: um mesmo anexo em modelos com capabilities diferentes escolhe
  `native`, `image`, `text` e `reference`, com o registro correspondente.
- Recuperação: recusa por mídia repete uma vez com a forma textual; recusa sem
  relação com mídia não repete.
- Reidratação só do turno atual.

## Fora do escopo

- Migrar o chat do Vectora para a Engine ou duplicar o comportamento em shadow mode.
- Definir o store de mídia do consumidor, sua política de retenção ou limpeza.
- STT, TTS, OCR, renderização de PDF, extração de quadros e geração de mídia.
- Streaming de áudio ou vídeo em tempo real.
