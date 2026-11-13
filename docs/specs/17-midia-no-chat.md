# Contrato de mídia no chat

**Estado:** planejada. Nenhuma parte deste contrato está implementada na Engine.
A referência de comportamento é o backend Python do Vectora (entrada multimodal da
0.1.25), onde o contrato já roda.

Este documento define como o chat da Engine representa mídia (imagem, áudio,
vídeo, documento e arquivo genérico) em mensagens e em resultados de tools, como
decide o que cada provider recebe e o que o ledger guarda. Ele não migra o chat do
Vectora para a Engine: a adoção é capability por capability, com fallback e
rollback.

## Situação atual da Engine

- `ContentPart` (`crates/vectora-engine/src/chat/types.rs`) tem só `Text` e
  `ImageUrl`. Não há áudio, documento, vídeo, arquivo, tipo MIME nem referência de
  mídia.
- O resultado de uma tool é uma `String`; os adapters nativos achatam o resultado
  em texto, e imagens de resultado de tool se perdem fora do caminho
  OpenAI-compatível.
- Não existe tabela de capability por modalidade. A incompatibilidade só é
  detectada pelo texto do erro (`ChatError::MultimodalIncompatible`).
- Não existe store de mídia; o único ponteiro de artefato é
  `VerificationEvidence.artifact_ref`.

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

`ToolResult.output` deixa de ser só `String`: passa a ser uma lista de partes
(texto e imagem; os demais tipos de mídia de tool entram como forma textual).
O texto continua sendo o que o loop e o ledger leem; a imagem é uma `MediaRef`.

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
consumidor pode sobrescrever.

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

Se o provider recusar a requisição por causa de mídia (erro classificado como
`MultimodalIncompatible`, tamanho excedido ou tipo não aceito), a Engine repete a
chamada **uma vez** para o mesmo modelo com as partes rebaixadas para a forma
textual, e registra a repetição no run. Erros sem relação com mídia não são
repetidos. Não há troca de modelo: fallback entre modelos é o contrato do chat
fallback (spec 06).

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
