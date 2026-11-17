# Contrato de mídia no chat

**Estado:** contrato de conteúdo multimodal para chat e tools. Implementação e gaps são classificados por protocolo no [checklist de mídia](../checklists/04-midia-no-chat.md); não inferir integração total a partir dos tipos do contrato. O Vectora tem capacidades de produto próprias (STT local, conversão de arquivos, UI e empacotamento) que não fazem parte desta API.

Este documento define como a Engine representa imagem, áudio, vídeo, documento e arquivo genérico em mensagens/resultados de tools, decide o payload para cada provider e separa conteúdo persistível de envio efêmero. A adoção por consumidores permanece incremental.

## Situação atual da Engine

- `ContentPart` representa texto, imagem, áudio, vídeo, documento e arquivo por referência; bytes inline são restritos ao envelope efêmero.
- `ToolResult` contém partes tipadas; adapters convertem as modalidades que o protocolo do provider suporta.
- O preparo existe nos três modos dos bindings: manual, contexto por chamada e contexto padrão do cliente. O mapa temporário `asset_id → bytes` é fornecido pelo host.
- O contexto valida digest, MIME, limite por asset e escopo de reidratação. O limite padrão é 20 MiB por asset; overrides por provider/modelo/modalidade não excedem o teto global.
- Limites agregados opcionais de bytes/assets cobrem chat, requests efêmeros inline, Decision Model, embeddings e rerank multimodais do llama-server conforme a [spec 18](18-providers-e-modalidades-llama-cpp-v1.md). Catálogo automático confiável e limites de duração/resolução seguem pendentes.
- A Engine não faz STT local, OCR, renderização de PDF, extração de frames ou gestão de assets persistentes; o consumidor mantém essas responsabilidades.

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
`max_media_bytes` governa cada asset. O contexto também pode impor tetos
opcionais `max_total_media_bytes` e `max_media_parts` por envio. Na reidratação
do asset map, os tetos são compartilhados por todas as mensagens processadas no
turno atual e resultados de tools; contam bytes brutos lidos e assets tentados,
não o tamanho codificado do request HTTP. Exceder um teto agregado converte a
parte excedente em texto/referência sem ler seus bytes. Para bytes já inline em
`EphemeralChatRequest`, exceder os tetos falha antes da chamada ao provider.
Ausência de override por modalidade usa só o teto por asset e não presume
limite universal do provider. Estado e precedência:
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
indisponível: nome]`) e a decisão registra o motivo. Na reidratação, a assinatura
  dos bytes também prevalece sobre o MIME declarado. Quando a modalidade detectada
  coincide com a parte, o payload usa o MIME real; se diverge, a mídia cai para
  texto/referência. Documentos nativos exigem PDF detectado, e previews precisam
  ser imagens reconhecidas antes de serem enviados.
- **Só o turno atual reidrata.** Mídia de mensagens antigas segue como forma
  textual ou nota, para limitar custo e superfície. O consumidor pode ampliar para
  as N imagens mais recentes de resultados de tools.

## Fronteira de requests persistíveis e envio efêmero

A API separa o envelope persistível `PersistableChatRequest` do envelope de
transporte `EphemeralChatRequest`. O primeiro rejeita `ContentPart::Inline` e
`ImageUrl.url` com `data:` durante serialização e desserialização. O segundo
contém `contract_version: 1` e um `ChatRequest`; só os métodos `*_ephemeral` dos
bindings aceitam esse envelope, validam o limite global/configurado e conferem
tipo e MIME pelos bytes antes do envio. O teto padrão é 20 MiB por asset e o
contexto pode reduzi-lo. O MIME declarado é normalizado pela assinatura dos
bytes; modalidade divergente, base64 inválido e payload acima do limite falham
antes da chamada ao provider.

Os três modos seguem disponíveis nos bindings Python e TypeScript: preparo
manual com `prepare_chat_content`/`prepareChatContent` seguido de envio pelo
método efêmero; contexto `MediaExecutionContext` passado por chamada; e contexto
padrão configurado no client. Os seis métodos efêmeros cobrem complete/stream
para adapter OpenAI-compatible, adapters nativos e fallback. Requests normais
continuam aceitando referências e são seguros para persistência. O contrato
público Chat v1 não mudou: o envelope efêmero tem versão independente.

A escolha de contrato aprovada por brunosrz separa o request persistível do
envelope efêmero de envio; os três modos de integração usam essa mesma
fronteira.

## Resultado de tool com partes

`ToolResult` usa `parts: Vec<ToolResultPart>` (contrato v2), em ordem. Cada parte
é `{ "type": "text", "text": "…" }` ou `{ "type": "image", "media_ref": MediaRef }`. A imagem referencia um asset do host;
bytes não entram no resultado serializado. `ToolResult::to_chat_message` e os
helpers Python/TypeScript retornam uma `MessageRole::Tool` com as partes
correspondentes. O loop modelo/tool usa contrato v3; snapshots v1/v2 e resultados
v1 com `output: String` são rejeitados. Não há conversão nem compatibilidade com
o campo anterior.

O limite do loop mede os bytes serializados das partes (texto e referência). O
limite do asset binário é aplicado ao reidratar por `MediaExecutionContext`; o
host continua responsável por autorizar o `MediaStore`. Na reidratação, a assinatura
dos bytes prevalece sobre o MIME declarado. A Engine usa o MIME detectado quando
a modalidade coincide; se os bytes indicarem outra modalidade, não os envia com
o tipo declarado e rebaixa a parte para texto/referência. Para documentos nativos,
somente PDF é aceito; previews precisam classificar como imagem.

### Decisão resolvida — substituição direta do contrato

brunosrz decidiu substituir `output: String` por partes tipadas, sem preservar
v1 ou manter camada de compatibilidade. Essa mudança afeta resultados, snapshots
e serialização do loop e foi implementada como quebra de contrato antes da
primeira release da Engine.

Mapeamento por provider:

| Provider / protocolo                                        | Texto do resultado                    | Imagem do resultado                                                                                  |
| ----------------------------------------------------------- | ------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Anthropic Messages                                          | `tool_result` com `content` em partes | Bloco `image` dentro do `tool_result`.                                                               |
| Google Gemini                                               | `functionResponse`                    | Parte inline (`inlineData`) no mesmo turno do `functionResponse`.                                    |
| OpenAI Responses                                            | `function_call_output`                | Conteúdo de imagem é encaminhado somente quando representável pelo protocolo e capability declarada. |
| OpenAI Chat Completions e compatíveis (OpenRouter, 9router) | mensagem `tool`                       | Mensagem de `user` seguinte com a imagem, pois a mensagem `tool` aceita só texto.                    |
| Cohere Chat v2                                              | mensagem `tool`                       | Sem imagem em resultado de tool: a imagem vira forma textual com a referência.                       |

Quando o modelo não declara `tool_result_images`, a imagem do resultado vira a
forma textual com a referência, e a decisão é registrada.

## Capability por modalidade

Cada modelo resolve um conjunto de capabilities: `vision`, `pdf`, `audio_input`,
`video_input` e `tool_result_images`, cada uma `suportado`, `não suportado` ou
`desconhecido`. A resolução é em camadas, da mais específica para a mais geral:

1. override explícito do consumidor para o modelo;
2. metadado específico do modelo/gateway fornecido pelo consumidor, incluindo
   conversão de `architecture.input_modalities` do llama-server;
3. perfil genérico conhecido do provider;
4. padrão conservador: capacidades não declaradas permanecem `desconhecido`.

Um perfil genérico nunca pode elevar uma modalidade que a metadata específica
do modelo declara como não suportada. `llamaServerModelMediaCapabilities` e
`llama_server_model_media_capabilities` convertem um registro de `/v1/models`
para `gateway_metadata` nos bindings. A conversão é explícita e não faz
discovery, cache ou atualização automática; metadata ausente permanece
desconhecida, e `tool_result_images` não é inferido de suporte a visão.

`desconhecido` é tratado como não suportado para efeito de envio nativo, e o
consumidor pode sobrescrever. A capability precisa corresponder ao protocolo
real do adapter: OpenAI Chat Completions usa `file.file_data` para PDF e
`input_audio` para WAV/MP3. OpenAI Responses tem adapter próprio; cada parte só
é enviada nos formatos mapeados pelo código e pelas fixtures.

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
