# Providers e modalidades de IA — referência llama.cpp v1.0.0

**Estado:** contrato e inventário por protocolo/operação. Consulte o plano e checklist desta capability para os gaps atuais. Fixture offline não equivale a integração com serviço/modelo real. Decision Model é a categoria; Jev é um modelo específico. SystemOne é o protocolo usado tanto pelo servidor Laya quanto pelo endpoint `/v1/systemone` do `llama-server`; adapters e capacidades de cada runtime continuam distintos.
**Referência fixa:** [`ggml-org/llama.cpp` v1.0.0](https://github.com/ggml-org/llama.cpp/releases/tag/v1.0.0), commit `b1b770d`.

## Objetivo

Oferecer contratos comuns e verificáveis para providers e modalidades de IA usados pela Engine. Protocolos são a unidade de implementação; nomes comerciais são perfis quando diferenças de endpoint, autenticação, payload ou resposta forem comprovadas. Compatibilidade de transporte não anuncia suporte universal de provider ou modelo.

Rust segue como implementação canônica. Python e TypeScript encaminham para o mesmo core. A referência `llama.cpp` significa as APIs HTTP do `llama-server` na tag v1.0.0; não adiciona o runtime C/C++ como dependência de build.

## Inventário observado na Engine

| Provider/protocolo              | Estado no código                                                                                                        | Limites                                                                                                                                            |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenAI-compatible               | Chat/stream e embeddings genéricos por endpoint                                                                         | 9router, OmniRouter e Kimi precisam de validação específica de serviço/rota/auth/erro antes de perfil dedicado.                                    |
| OpenAI                          | Chat Completions, Responses e embeddings compatíveis                                                                    | Não implica tools hospedadas, TTS/STT, vídeo ou geração de imagem.                                                                                 |
| OpenRouter                      | Perfil dedicado para chat, endpoint, Bearer, atribuição e extensões de roteamento; embeddings/rerank disponíveis        | Modalidades e políticas variam por modelo e request.                                                                                               |
| Anthropic, Gemini e Cohere      | Chat nativo; Cohere também embeddings/rerank                                                                            | A cobertura multimodal e de stream é por operação e protocolo.                                                                                     |
| Ollama                          | Chat nativo `/api/chat`, chat compatível e embeddings                                                                   | Fixtures cobrem JSON/NDJSON; serviço real não foi exercitado nesta execução.                                                                       |
| llama.cpp `llama-server` v1.0.0 | Health/discovery, chat, Decision Model, TTS, realtime, embeddings e rerank, inclusive operações de mídia cobertas       | Discovery preserva declarações do servidor, não prova capacidade efetiva do modelo; smoke real permanece opt-in.                                   |
| Laya `laya-serve`               | Cliente standalone para health e Decision Model via SystemOne; checkpoints Laya como `multilingual` e `typed-decisions` | `model` pode ser omitido para roteamento do servidor; Jev é um modelo, não o nome da API nem um checkpoint Laya. Serviço real não foi configurado. |
| Voyage                          | Embeddings e rerank                                                                                                     | Confirmar detalhes de score, dimensões e erros na matriz da operação.                                                                              |

O perfil OpenAI Chat Completions, OpenAI Responses e OpenRouter mantém protocolos selecionáveis separados. OpenAI Responses cobre apenas conteúdo e eventos mapeáveis pelo contrato comum; built-in tools e modalidades não implementadas permanecem fora do adapter.

#### Perfil de chat OpenRouter

O perfil `openrouter` usa por padrão `https://openrouter.ai/api/v1`, exige
`api_key`, aceita `base_url` customizada para proxy/região e envia Bearer.
`app_url` e `app_title` mapeiam para `HTTP-Referer` e
`X-OpenRouter-Title`; os headers genéricos podem substituir os valores gerados.
Na API Rust, `OpenAiCompatibleConfig::openrouter` e
`openrouter_attribution` oferecem o mesmo caminho.

A rota Chat Completions continua no contrato OpenAI-compatible. Campos nativos
do gateway como `provider`, `models`, `route`, `user` e `plugins` passam por
`ChatRequest.extensions` por request; não são defaults globais porque ordenação,
fallback, região e política de dados têm custo e impacto de privacidade. A
Engine não converte esses campos em garantias de disponibilidade, privacidade,
modalidade ou preço. Não se declara a Responses API coberta por este perfil.

Fonte oficial conferida em 15/11/2026: [API Chat Completions e headers de
atribuição](https://openrouter.ai/docs/api/api-reference/chat/create-a-chat-completion),
[campos de roteamento](https://openrouter.ai/docs/guides/routing/provider-selection).

### Perfil OpenAI Chat Completions

O perfil nomeado `openai` usa `/v1/chat/completions` no endpoint oficial
`https://api.openai.com/v1`, requer API key e aceita `base_url` para proxy ou
endpoint de teste, além de headers extras não reservados. Ele conserva o
contrato Chat Completions e permanece selecionável separadamente do perfil
`native` com `provider: openai_responses`. O cliente OpenAI-compatible genérico
e o perfil OpenRouter continuam disponíveis.

### OpenAI Responses API

O protocolo Responses é distinto de Chat Completions: requests usam `input`,
function tools são itens `function_call`/`function_call_output`, e SSE carrega
eventos semânticos (`response.output_text.delta`, lifecycle da resposta,
argumentos de função, usage e terminal status). A implementação
converte o histórico portátil completo, sem depender de `previous_response_id`,
e define `store: false` como default de privacidade, permitindo override
explícito no request. O core Rust já traduz input de texto/imagem, function
calls/results, opções e extensões, interpreta resposta/reasoning/usage e mapeia
eventos SSE tipados; testes unitários exercitam os contratos. Fixtures HTTP nos
bindings Python e TypeScript passaram após build nativo. O perfil é selecionado
como `native` com `provider: openai_responses`. O suporte declarado cobre texto,
imagem de entrada, function calling, reasoning/usage quando presentes e eventos
SSE mapeáveis; built-in tools, TTS/STT, vídeo e geração de imagem continuam fora
do adapter até possuírem contratos próprios. Referências oficiais: [Responses
API overview](https://developers.openai.com/api/reference/responses/overview),
[create response](https://developers.openai.com/api/reference/resources/responses/methods/create),
[streaming events](https://developers.openai.com/api/reference/resources/responses/streaming-events),
[function calling](https://developers.openai.com/api/docs/guides/function-calling)
e [image input](https://developers.openai.com/api/docs/guides/images-vision).

## Operações declaradas pelas APIs dos providers

Isto é o que a documentação oficial declara, não o que a Engine já suporta. A
capacidade permanece por operação e modelo, mesmo quando várias APIs usam a
mesma base URL.

| Provider/API                               | Operações documentadas                                                                                                          | Diferenças relevantes para a Engine                                                                                                                                                                                   |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenAI                                     | Chat/Responses, visão, geração/edição de imagem, embeddings, transcrição de arquivo, ASR/conversa realtime e TTS                | Responses, Images, Audio e Realtime têm rotas, eventos e lifecycle distintos; o perfil Chat Completions não os substitui.                                                                                             |
| Anthropic Messages                         | Chat/tool use e entrada de imagem                                                                                               | Visão é entrada; não implica geração de imagem, áudio ou vídeo. A Files API também tem lifecycle separado.                                                                                                            |
| Google Gemini                              | Chat/conteúdo multimodal, entrada de vídeo, imagem gerada, TTS e áudio interativo Live                                          | Operações e modelos usam contratos distintos; disponibilidade depende do model ID.                                                                                                                                    |
| Cohere                                     | Chat, embeddings e rerank                                                                                                       | Adapters atuais têm formatos/opções próprios; não inferir capacidades de mídia.                                                                                                                                       |
| OpenRouter                                 | Chat multimodal, image/video generation, TTS, STT, embeddings, rerank e áudio                                                   | `/chat/completions`, `/images`, `/videos`, `/audio/speech`, `/audio/transcriptions`, `/embeddings` e `/rerank` têm requests/responses diferentes. Algumas gerações são jobs; input de vídeo tem limites por provider. |
| 9Router                                    | A documentação do produto anuncia chat, embeddings, TTS, STT, imagem, visão, vídeo, busca e fetch via gateway OpenAI-compatible | A fonte consultada anuncia categorias, mas não descreve os schemas/versionamento de todas as rotas. Não criar adapter sem contrato/API versionados e fixtures.                                                        |
| OmniRouter em `api.omnirouter.ru`/`.cloud` | Messages, Responses, Chat, Decision Model, transcrição, geração de imagem e geração de áudio/música                             | Decision Model usa `/v4/ai/decision-model`; áudio/imagem usam jobs assíncronos, idempotency key e polling. Geração de música não é TTS.                                                                               |
| Kimi / Moonshot                            | API oficial compatível com OpenAI/Anthropic; Kimi Code documenta entrada de vídeo                                               | Modelos diferem: a página oficial de Kimi K2 afirma que essa versão não suporta visão. Não promover capacidade de um modelo para todo o provider.                                                                     |
| Ollama                                     | API nativa de chat/visão e embeddings, além de endpoint de compatibilidade OpenAI `/v1` (subconjunto)                           | API nativa e modo `/v1` são perfis distintos; manter diferenças e capabilities por modelo. A documentação de embeddings descreve `/api/embed` como embeddings de texto, com vetores L2-normalizados.                  |

Fontes primárias conferidas: [OpenAI Images/Vision](https://developers.openai.com/api/docs/guides/images-vision), [OpenAI Transcription](https://developers.openai.com/api/docs/guides/transcription), [OpenAI Realtime](https://developers.openai.com/api/docs/guides/realtime-conversations), [Gemini video](https://ai.google.dev/gemini-api/docs/video-understanding), [Gemini TTS](https://ai.google.dev/gemini-api/docs/speech-generation), [Gemini image generation](https://ai.google.dev/gemini-api/docs/image-generation), [Anthropic Vision](https://docs.anthropic.com/en/docs/build-with-claude/vision), [OpenRouter modality APIs](https://openrouter.ai/blog/insights/every-modality-one-api/), [9Router docs](https://docs.9router.com/), [OmniRouter API](https://omnirouter.ru/docs/api-reference/), [Moonshot/Kimi API context](https://moonshotai.github.io/Kimi-K2/), [Ollama Vision](https://docs.ollama.com/capabilities/vision), [Ollama Embeddings](https://docs.ollama.com/capabilities/embeddings) e [Ollama OpenAI compatibility](https://docs.ollama.com/api/openai-compatibility).

O adapter Ollama nativo implementa `/api/chat` com resposta JSON e stream
NDJSON. Opções canônicas `temperature`/`max_tokens` viram `options.temperature`
e `options.num_predict`; campos Ollama como `think`, `format`, `keep_alive` e
`options` adicionais seguem em `ChatRequest.extensions`. Imagens inline em
base64 são convertidas para `images`; URLs remotas de imagem não são aceitas
por essa rota. A API OpenAI-compatible permanece alternativa portátil. Fontes:
[Ollama chat](https://docs.ollama.com/api/chat) e
[compatibilidade OpenAI](https://docs.ollama.com/api/openai-compatibility).

`OmniRouter` não identifica serviço único: a pesquisa encontrou também
[`omnirouter.li`](https://omnirouter.li/docs), com API OpenAI-compatible e
jobs de vídeo/voiceover que diferem dos contratos de `api.omnirouter.ru`. A
escolha do alvo afeta base URL, auth, endpoints, jobs e schema; não criar um
perfil chamado apenas `omnirouter` até brunosrz confirmar qual serviço pretende.

O inventário é baseado em `crates/vectora-engine/src/chat/`,
`crates/vectora-engine/src/llm/` e nos dois bindings na revisão do SHA pai desta
spec. Deve ser atualizado junto com cada implementação e fixture.

## Capacidades que não podem ser confundidas

- **Chat e Decision Model são APIs distintas.** Uma saída textual que classifica
  não substitui uma decisão nativa com perguntas tipadas e probabilidades.
- **Visão é entrada; geração de imagem é saída.** Uma API de chat que recebe
  imagem não implementa geração/edição de imagem.
- **Áudio de entrada, STT, TTS e conversa realtime são operações distintas.**
  Devem ter requests, respostas, cancelamento, limites e streams explícitos.
- **Vídeo de entrada** pode ser encaminhado como mídia ou processado por frames;
  vídeo gerado é outra capacidade. Não usar `ContentPart::Video` como prova de
  suporte pelo provider.
- **Embeddings e rerank** mantêm seus contratos e respostas de vetor/ordenação;
  não são variantes de chat.
- **Disponibilidade não equivale a suporte:** cada provider/modelo declara
  modalidade por operação, com origem `provider_profile`, `model_metadata`,
  `operator_override` ou `unknown`. Unknown deve falhar fechado para payloads
  que possam ser rejeitados ou caros.

## Perfil standalone Laya `laya-serve`

O servidor standalone Laya expõe `/health` e a API SystemOne em
`/v1/systemone`; não há `GET /v1/models`. O campo `model` é opcional: quando
informado, identifica um checkpoint/modelo do próprio Laya, como `english`,
`multilingual` ou `typed-decisions`; quando omitido, o servidor pode aplicar
seu roteamento. Jev é um modelo específico e um ID Jev não é checkpoint Laya.
O `llama-server` também implementa SystemOne, mas usa descoberta de modelos e
suas próprias capacidades de mídia; compartilhar protocolo não torna os dois
servidores equivalentes em runtime.

A rota `/v1/systemone` do `laya-serve` aceita estado textual; não recebe
`image`, `audio` nem `video`. Assets efêmeros multimodais não devem ser enviados
por esse cliente. Laya retorna `answers` e `usage`, mais campos como `model`,
`routing`, metadata de confiança e detalhes de truncamento. O adapter valida e
normaliza os campos comuns, tolera metadata adicional e não persiste nem
reencaminha `routing` automaticamente, pois pode carregar dados derivados do
estado. Choice e score trazem probabilidades arredondadas independentemente a
quatro casas; a validação permite erro de soma proporcional ao número de
opções/níveis. Para score, os mapas `probabilities` e `legend` são validados por
índice e comparados com `criteria`. A API não transmite resultados.

No llama-server, `EphemeralDecisionRequest.files` aceita data URLs de imagem e
áudio. `MediaExecutionContext` pode limitar bytes totais e quantidade de
arquivos; o limite efetivo de bytes nunca excede o teto global do client. O
binding valida e decodifica os arquivos antes de descobrir o modelo ou enviar
`/v1/systemone`; essa rota recebe bytes inline somente no envelope efêmero.
Após discovery, modalidades de imagem/áudio são validadas contra o modelo
selecionado, usando o resolver comum: override explícito do consumidor vence a
declaração do modelo. Metadata ausente permanece fail-closed.

Contrato upstream: [`laya/docs/http-api.md`](https://github.com/NandhaKishorM/laya/blob/main/docs/http-api.md).
Compatibilidade de schema é exercitada por fixtures; execução com checkpoint
real é um smoke opt-in, separado de integração com o GGUF no llama.cpp. A CLI de referência está especificada em [`docs/specs/20-engine-cli.md`](20-engine-cli.md); ela mantém adapters separados para SystemOne no servidor Laya e no llama-server usado para Laya-GGUF.

## Base llama.cpp v1.0.0

Implementar contra a tag imutável, não contra `master`. O servidor v1.0.0
documenta:

| Operação                      | API do `llama-server` v1.0.0                                                                                                                                                                                                                                              | Contrato Engine proposto                                                                                                                                                                                                                                                            |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Chat                          | `/v1/chat/completions`; Responses e Anthropic Messages também existem                                                                                                                                                                                                     | Reusar ChatClient/protocolo compartilhado, com perfil llama.cpp versionado, endpoint e peculiaridades em fixtures.                                                                                                                                                                  |
| Discovery                     | `/v1/models` contém `architecture.input_modalities` e `output_modalities`; `decisions` e `audio` têm significado operacional. O servidor pode responder `text` como fallback quando não tem metadados do modelo; `video` só aparece após carregar um modelo que o aceite. | Preservar arrays brutos e representar campos ausentes como `None`. A API não distingue o fallback `text` de uma capacidade textual real; não promover nenhum deles automaticamente a garantia.                                                                                      |
| Decision Model                | `/v1/systemone`, sem streaming; `state`, `files`/`images`, `questions`                                                                                                                                                                                                    | API `DecisionModel` separada; preservar opções e probabilidades, validação e erro; suportar multimodal apenas quando anunciado.                                                                                                                                                     |
| Imagem/áudio/vídeo de entrada | Partes `image_url`, `input_audio` e `input_video` em chat; formatos de entrada multimodais também em `/v1/embeddings`. Vídeo depende de FFmpeg; áudio de entrada aceita formatos do miniaudio.                                                                            | Mapear somente bytes/assets que o host autorizou e materializou. Não encaminhar URL remota nem `file://` para o servidor por padrão: isso delegaria acesso à rede/FS ao processo externo.                                                                                           |
| TTS                           | `/v1/audio/speech` e `/tts`; saída WAV/PCM, streams de áudio; voz e sample rate dependem do modelo                                                                                                                                                                        | API `SpeechSynthesis`; resposta binária e metadados de formato/sample rate; não assumir MP3, 24 kHz ou vozes OpenAI.                                                                                                                                                                |
| Áudio realtime / transcrição  | WebSocket `/v1/realtime`, PCM16 e eventos; sessão pode ser transcription ou conversation                                                                                                                                                                                  | API de sessão realtime/ASR com eventos tipados, cancelamento e backpressure. Não rotular como upload STT batch.                                                                                                                                                                     |
| Embeddings                    | `/v1/embeddings`, inclusive conteúdo multimodal para modelos habilitados                                                                                                                                                                                                  | Usar contrato de embeddings com extensão de entrada multimodal versionada; conservar ordem/dimensões.                                                                                                                                                                               |
| Rerank                        | `--rerank` habilita `/reranking`, `/rerank`, `/v1/rerank` e `/v1/reranking`; aceita formatos Jina/TEI, top_n e pares com score. Jina também admite texto/imagem para rerankers compatíveis.                                                                               | Rerank textual usa `documents` no formato Jina e o contrato comum; preserva índice/ordem e score em [0,1]. A extensão multimodal é efêmera, aceita texto e imagens via asset map e requer capacidade declarada pelo modelo; URLs remotas e bytes inline persistíveis são recusados. |
| Geração de imagem             | Não consta como operação do `llama-server` v1.0.0                                                                                                                                                                                                                         | Não suportada por este adapter; contrato Engine deve permitir provider distinto quando houver backend real.                                                                                                                                                                         |

Referências de API fixadas em [`tools/server/README.md` na tag v1.0.0](https://github.com/ggml-org/llama.cpp/blob/v1.0.0/tools/server/README.md):
`/v1/chat/completions`, `/v1/models`, `/v1/systemone`, `/v1/audio/speech`,
`/v1/realtime`, embeddings e rerank. O servidor documenta diferenças que
impedem tratar seus endpoints como equivalentes às APIs OpenAI/Anthropic
originais: TTS aceita WAV/PCM, não MP3, e sample rate e vozes dependem do
modelo. A release registra suporte a Drama-3 TTS e novos modelos vision; isso
não transforma geração de imagem em endpoint suportado.

### Embeddings de texto no llama-server

O perfil llama-server expõe embeddings de texto pelo mesmo endpoint v1.0.0.
Mantém o contrato `EmbeddingRequest` com versão, modelo, input type e lote de
textos; `input_type` e `dimensions` não são enviados como opções inventadas do
servidor. Reordena a resposta pelo `index`, rejeita cardinalidade/índices/
dimensões inválidos e vetores vazios ou não finitos. Usage é normalizado para
tokens de entrada; a direção/normalização do vetor permanece a definida pelo
llama-server/modelo. Lotes vazios retornam sem rede.

### Embeddings multimodais no llama-server

O servidor v1.0.0 aceita `input` como string, lista de strings ou lista de
objetos `{ "content": [...] }`; cada objeto de conteúdo pode conter texto,
`image_url`, `input_audio` ou `input_video`. A operação retorna um embedding
por elemento; exige pooling diferente de `none` e normaliza vetores pela norma
Euclidiana. Áudio/vídeo dependem do suporte real do modelo/projetor; vídeo usa
FFmpeg.

O core implementa `EphemeralLlamaServerEmbeddingRequest` em Rust: cada input é
uma sequência ordenada de texto e refs do asset store. O adapter exige que
`/v1/models` anuncie a modalidade solicitada, materializa o asset pelo
`MediaExecutionContext`, confere tamanho, MIME e digest, serializa imagem como
`image_url` com data URL e áudio/vídeo como `input_audio.data`/
`input_video.data`. URL remota, path, `ImageUrl` caller-supplied e `Inline` são
rejeitados antes da chamada de embeddings. Teto global e limite por provider/
modelo se aplicam. Respostas preservam exatamente um vetor por input e validam
índices, dimensão e finitude. A API não promete `dimensions` customizável nem
outros modos de normalização, pois não estão no contrato documentado.

Python e TypeScript encaminham essa chamada pelo mesmo core e exigem um contexto
efêmero do consumidor. O orçamento configurado em `MediaExecutionContext` é
compartilhado entre todos os inputs da mesma chamada; excedentes não são enviados
e tornam a operação não suportada para aquele request. Não persistem os bytes.

### Rerank multimodal no llama-server

O request efêmero reúne query e todos os documentos no mesmo preparo com
`MediaExecutionContext`. Os limites agregados cobrem assets materializados na
query e nos documentos em conjunto; se um excedente cair para referência, a
operação falha antes do `POST /rerank`, sem enviar mídia parcial.

Fontes fixadas: [API embeddings da tag v1.0.0](https://github.com/ggml-org/llama.cpp/blob/v1.0.0/tools/server/README.md#post-v1embeddings-openai-compatible-embeddings-api), [formatos de conteúdo multimodal](https://github.com/ggml-org/llama.cpp/blob/v1.0.0/tools/server/README.md#post-v1chatcompletions-openai-compatible-chat-completions-api).

## Contrato compartilhado

1. Configuração identifica `protocol`, `endpoint`, auth/headers e timeouts;
   `provider_profile` é opcional e não muda semântica sem uma capability
   verificada. O profile `llama_cpp` fixa as rotas e preserva a metadata
   `architecture` recebida sem promovê-la a garantia do modelo. Credenciais
   nunca são serializadas em Debug, logs ou fixtures.
2. Cada operação possui versão, request/response tipados, erro discriminado,
   timeout total e de conexão, limites de corpo/stream, cancelamento e política
   de retry próprios.
3. Fallback só ocorre antes do primeiro evento ou byte entregue ao consumidor;
   nunca reinicia geração parcial.
4. Bytes de mídia ficam em contexto de execução efêmero, nunca em requests
   persistíveis. A autorização de ler URLs, arquivos ou assets pertence ao host.
5. Capabilities são descobertas ou configuradas explicitamente por modelo.
   Campos ausentes permanecem `unknown`; arrays são metadata declarada pelo
   servidor, não garantias verificadas. Como o fallback `text` não vem marcado,
   mantê-lo como valor bruto sem inferir que é a única modalidade. Uma resposta
   de erro real pode refinar estado somente conforme regra versionada e auditável.
6. Os mesmos fixtures exercitam core Rust e bindings Python/TypeScript. Serviço
   real `llama-server` v1.0.0 complementa, mas não substitui, testes deterministas.

## Superfícies implementadas e gaps

- **Chat/protocolos:** adapters OpenAI-compatible, OpenAI Chat Completions e Responses, OpenRouter, Anthropic, Gemini, Cohere, Ollama e llama-server; fixtures cobrem os adapters declarados. 9router, OmniRouter e Kimi dependem de validação do serviço concreto.
- **Llama-server v1.0.0:** health/discovery, chat, Decision Model, embeddings, rerank, TTS bufferizado/streaming e realtime. A lista de modalidades devolvida pelo servidor é metadata, não garantia de suporte do modelo. A Engine oferece conversor explícito para o resolver multimodal; não consulta nem atualiza esse catálogo automaticamente.
- **Laya standalone:** SystemOne para health/Decision Model de texto, com checkpoints Laya próprios ou roteamento do servidor; distinto do runtime llama-server. Smoke real requer serviço/checkpoint configurados.
- **Mídia:** requests persistíveis e efêmeros separados; bytes resolvidos pelo asset map. Ver [spec de mídia](17-midia-no-chat.md) para ownership, MIME, digest e limites.
- **Ausente ou incompleto:** geração de imagem, batch STT genérico, operações gerais de vídeo, catálogo confiável de modalidades/limites e matriz de smoke tests reais.

## Ordem por dependência

1. Manter a matriz provider × operação atualizada e validar cada endpoint usado por consumidores.
2. Fechar conformidade de erros, autenticação, cancelamento, stream, redaction e limites entre Rust/Python/TypeScript.
3. Completar catálogo de modalidade/limites; ampliar limites de duração e
   resolução e cobrir operações multimodais adicionais que exponham assets.
4. Implementar operações ausentes somente após contrato e endpoint verificáveis; começar por geração de imagem, batch STT e vídeo conforme a prioridade da EPIC.
5. Executar smoke tests opt-in com versões fixadas e consolidar gates de distribuição/adoção no plano geral.

## Fora do escopo presumido

- Linkar ou compilar `llama.cpp` como biblioteca dentro do crate Rust.
- Recriar UI, download/gestão de modelos, MCP host, permissões de arquivos ou
  ferramentas do llama.app dentro da Engine.
- Declarar compatibilidade universal de OpenAI-compatible, igualdade numérica
  entre providers, ou suportar uma modalidade porque um modelo específico a
  suporta.
- Fazer o host da Engine baixar modelos ou iniciar processos automaticamente.

## Critérios de aceite da capability

- Matriz de suporte atualizada pelo código e fixtures para cada provider e
  operação; lacunas visíveis nos dois bindings.
- llama.cpp v1.0.0 fixado e coberto em `llama-server`; chat, discovery,
  `systemone`, TTS/realtime ASR e entradas multimodais têm contratos próprios.
- Imagem gerada, batch STT e qualquer API ausente na v1.0.0 ficam marcadas como
  não suportadas por esse adapter até existir implementação e evidência.
- Falhas de auth, modalidade, payload, tamanho, cancelamento e stream são
  testadas sem credenciais externas e com integração real opt-in.
- Nenhuma implementação de regra ou transporte paralela em Python/TypeScript.

## TTS no llama-server

O cliente Rust implementa `/v1/audio/speech` e `/tts`. A rota compatível
oferece WAV/PCM16; a rota nativa também suporta PCM float32 e controles
específicos. Bytes binários, Content-Type e sample rate são preservados;
`speaker_ref_b64` só pode ser enviado pela chamada efêmera. `instructions` é
ignorado pelo upstream e `speed` aceita somente `1.0`. Streaming entrega chunks
limitados, metadados, timeout e cancelamento, sem reescrever o header WAV de
comprimento desconhecido. Fixtures cobrem as rotas e consumo incremental;
integração com modelo real requer smoke test configurado.

## Sessão realtime de áudio

`/v1/realtime` é WebSocket e tem contrato de ciclo de vida próprio. O cliente da Engine suporta `intent=transcription` e a sessão default de conversa, PCM16 mono em chunks base64, `input_audio_buffer.commit` final/não final, `session.update` para sample rate entre 8–192 kHz, eventos JSON preservados e fechamento/cancelamento pelo host. Sem override, a taxa é model-native e precisa ser lida de `session.created`; a Engine não presume 24 kHz. O host pode enviar eventos nativos de conversa, como mensagens, `response.create` e `response.cancel`. Eventos desconhecidos, inclusive `error`, preservam campos para o consumidor. Requests de controle são efêmeros; batch STT não é inferido dessa API. A cobertura está registrada no plano e checklist desta capability.
