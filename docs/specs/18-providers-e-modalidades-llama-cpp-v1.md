# Providers e modalidades de IA — referência llama.cpp v1.0.0

**Estado:** contrato em revisão; implementação incremental ainda não iniciada.
**Referência fixa:** [`ggml-org/llama.cpp` v1.0.0](https://github.com/ggml-org/llama.cpp/releases/tag/v1.0.0), commit `b1b770d`.

## Objetivo

Oferecer contratos comuns e verificáveis para providers e modalidades de IA
usados pela Engine. Os protocolos são a unidade de implementação; nomes de
produto (OpenAI, OpenRouter, 9router, OmniRouter, Kimi, Ollama e `llama-server`)
são perfis/configurações sobre esses protocolos quando comprovadamente
compatíveis. Um perfil não anuncia todas as modalidades do provider.

Rust segue como implementação canônica. Python e TypeScript encaminham para o
mesmo core. A referência `llama.cpp` nesta capability significa as APIs HTTP do
`llama-server` da tag v1.0.0. A Engine não embute a biblioteca C/C++ nem passa a
compilar o runtime do llama.cpp como dependência.

## Inventário observado na Engine

| Provider/protocolo              | Estado observado no código                                                                                          | Limite que precisa ficar explícito                                                                                                                                                                 |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenAI-compatible               | Chat completion/stream genérico por `base_url`; embeddings genéricos                                                | Não há perfil OpenAI nomeado, catálogo de modalidades nem adapters de Responses, áudio ou imagem.                                                                                                  |
| Anthropic                       | Chat nativo e stream; conteúdo multimodal dentro do contrato de chat                                                | Não implica Messages API inteira, visão para qualquer modelo nem áudio/TTS/STT.                                                                                                                    |
| Google Gemini                   | Chat nativo e stream                                                                                                | Não há adapters independentes de imagem, áudio, vídeo, TTS ou STT nesta API.                                                                                                                       |
| Cohere                          | Chat nativo; embeddings e rerank                                                                                    | Cada operação tem cobertura própria; não presumir paridade multimodal.                                                                                                                             |
| OpenRouter                      | Embeddings e rerank específicos; chat pode usar OpenAI-compatible com URL customizada                               | Perfil de chat, metadados de gateway, capacidades por modelo e streaming multimodal não estão caracterizados em contrato dedicado.                                                                 |
| 9router e OmniRouter            | Sem enum/adapters próprios; podem ser configurados como endpoint OpenAI-compatible                                  | Compatibilidade, cabeçalhos, rotas, erros, metadados e modalidades precisam de fixtures por perfil antes de declarar suporte específico.                                                           |
| Ollama                          | Embeddings pelo protocolo Ollama                                                                                    | Não há cliente de chat Ollama dedicado. O endpoint `/v1` documenta somente um subconjunto da API OpenAI; validar cada operação e o comportamento local/cloud antes de anunciar paridade.           |
| llama.cpp `llama-server`        | Perfil Rust implementado nesta etapa: health, discovery `/v1/models` e chat/stream via transporte OpenAI-compatible | Bindings ainda não expõem o perfil; `systemone`, TTS e realtime seguem sem operações Engine. A arquitetura reportada pelo server é preservada como metadata, não como garantia inferida do modelo. |
| OpenAI                          | Parcialmente acessível pelo protocolo OpenAI-compatible; embeddings genéricos                                       | Sem adapters específicos de Responses, image generation, speech ou transcription.                                                                                                                  |
| Kimi                            | Sem adapter/profiling dedicado                                                                                      | Se usar endpoint compatível, deve entrar pela matriz OpenAI-compatible depois de validar diferenças; não inferir suporte pelo nome do modelo.                                                      |
| Cohere/Voyage/OpenRouter em RAG | Embeddings/rerank conforme enums atuais                                                                             | Ollama embeddings existe; não há garantia de matriz cruzada equivalente em todos os providers.                                                                                                     |

### Operações declaradas pelas APIs dos providers

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

## Base llama.cpp v1.0.0

Implementar contra a tag imutável, não contra `master`. O servidor v1.0.0
documenta:

| Operação                      | API do `llama-server` v1.0.0                                                                                                                                                                                                                                              | Contrato Engine proposto                                                                                                                                                                       |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Chat                          | `/v1/chat/completions`; Responses e Anthropic Messages também existem                                                                                                                                                                                                     | Reusar ChatClient/protocolo compartilhado, com perfil llama.cpp versionado, endpoint e peculiaridades em fixtures.                                                                             |
| Discovery                     | `/v1/models` contém `architecture.input_modalities` e `output_modalities`; `decisions` e `audio` têm significado operacional. O servidor pode responder `text` como fallback quando não tem metadados do modelo; `video` só aparece após carregar um modelo que o aceite. | Preservar arrays brutos e representar campos ausentes como `None`. A API não distingue o fallback `text` de uma capacidade textual real; não promover nenhum deles automaticamente a garantia. |
| Decision Model                | `/v1/systemone`, sem streaming; `state`, `files`/`images`, `questions`                                                                                                                                                                                                    | API `DecisionModel` separada; preservar opções e probabilidades, validação e erro; suportar multimodal apenas quando anunciado.                                                                |
| Imagem/áudio/vídeo de entrada | Partes `image_url`, `input_audio` e `input_video` em chat; formatos de entrada multimodais também em `/v1/embeddings`. Vídeo depende de FFmpeg; áudio de entrada aceita formatos do miniaudio.                                                                            | Mapear somente bytes/assets que o host autorizou e materializou. Não encaminhar URL remota nem `file://` para o servidor por padrão: isso delegaria acesso à rede/FS ao processo externo.      |
| TTS                           | `/v1/audio/speech` e `/tts`; saída WAV/PCM, streams de áudio; voz e sample rate dependem do modelo                                                                                                                                                                        | API `SpeechSynthesis`; resposta binária e metadados de formato/sample rate; não assumir MP3, 24 kHz ou vozes OpenAI.                                                                           |
| Áudio realtime / transcrição  | WebSocket `/v1/realtime`, PCM16 e eventos; sessão pode ser transcription ou conversation                                                                                                                                                                                  | API de sessão realtime/ASR com eventos tipados, cancelamento e backpressure. Não rotular como upload STT batch.                                                                                |
| Embeddings                    | `/v1/embeddings`, inclusive conteúdo multimodal para modelos habilitados                                                                                                                                                                                                  | Usar contrato de embeddings com extensão de entrada multimodal versionada; conservar ordem/dimensões.                                                                                          |
| Rerank                        | `--rerank` habilita `/rerank`, `/v1/rerank` e aliases; aceita formatos Jina/TEI e resultados/scores distintos. Há formato multimodal Jina para rerankers compatíveis.                                                                                                     | Reusar `RerankClient` apenas com fixtures da v1.0.0 para payload e parsing; preservar score bruto, índice e ordem declarada.                                                                   |
| Geração de imagem             | Não consta como operação do `llama-server` v1.0.0                                                                                                                                                                                                                         | Não suportada por este adapter; contrato Engine deve permitir provider distinto quando houver backend real.                                                                                    |

Referências de API fixadas em [`tools/server/README.md` na tag v1.0.0](https://github.com/ggml-org/llama.cpp/blob/v1.0.0/tools/server/README.md):
`/v1/chat/completions`, `/v1/models`, `/v1/systemone`, `/v1/audio/speech`,
`/v1/realtime`, embeddings e rerank. O servidor documenta diferenças que
impedem tratar seus endpoints como equivalentes às APIs OpenAI/Anthropic
originais: TTS aceita WAV/PCM, não MP3, e sample rate e vozes dependem do
modelo. A release registra suporte a Drama-3 TTS e novos modelos vision; isso
não transforma geração de imagem em endpoint suportado.

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

## Sequência de implementação

1. **Matriz e contrato:** documentar provider × operação × modalidade,
   endpoints, autenticação, diferenças, erros e cobertura atual. (PR de origem
   desta capability.)
2. **Perfil llama.cpp e discovery:** configuração REST, `/health`, `/v1/models`,
   validação de modalidades e chat multimodal compatível; fixtures com servidor
   fake e integração opcional contra v1.0.0. Implementado em Rust no PR #98 e
   exposto por wrappers Python/TypeScript no PR #99, ambos usando o mesmo
   cliente Rust; fixtures cobrem SSE, sem substituir integração real.
3. **Decision Models:** contrato independente e adapter `/v1/systemone`, com
   testes de estado/questions/files, multimodal e saída probabilística.
4. **Áudio:** TTS `/v1/audio/speech`, depois sessão WebSocket realtime/ASR;
   separar batch transcription de streaming ASR.
5. **Paridade de providers:** fixtures para OpenAI, OpenRouter, 9Router,
   OmniRouter (após identificar a instância), Kimi, Ollama e providers nativos;
   completar diferenças por operação/modelo, sem fingir que todos fornecem
   todas as modalidades.
6. **Outras operações multimodais:** entrada de áudio/vídeo e embeddings
   multimodais; image generation somente com provider e endpoint reais.
7. **Conformidade e maturidade:** mesma API Rust/Python/TypeScript, testes de
   contrato, mocks e integrações service-real opcionais; documentação de
   suporte por provider/modelo e gates de release.

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
