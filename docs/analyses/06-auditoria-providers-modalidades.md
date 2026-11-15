# Auditoria de providers e modalidades (referência llama.cpp v1.0.0)

**Data:** 13 de novembro de 2026. **Checkout:** `/home/brunosrz/Área de trabalho/engine`.
**Base consultada:** `main` remoto `f4a8cf9d7e30d9fb667a7f443f6572067e536d81`.
Esta auditoria é inventário documental; não altera o código de produção. O PR #96 foi integrado em `main` antes desta análise.

## Fontes consultadas

- Código local: `crates/vectora-engine/src/chat/{openai_compatible,native_providers,fallback,media,types}.rs`, `crates/vectora-engine/src/llm/{embeddings,rerank}.rs` e bindings Python/TypeScript.
- Release upstream fixada: [`llama.cpp v1.0.0`](https://github.com/ggml-org/llama.cpp/releases/tag/v1.0.0), commit de release `b1b770d`.
- API do servidor fixada: [`tools/server/README.md` na tag v1.0.0](https://github.com/ggml-org/llama.cpp/blob/v1.0.0/tools/server/README.md).
- Catálogo do app: [`llama.app/models`](https://llama.app/models), consultado para confirmar que o ecossistema publica modelos de chat, multimodais, decisão e embeddings; o catálogo não prova cobertura da Engine nem do server para todo modelo.

## O que o código realmente oferece

| Área           | Implementado                                                                                                                                                    | Não inferir                                                                                                                                |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Chat           | OpenAI-compatible completo no escopo atual; adapters nativos Anthropic, Google Gemini e Cohere; fallback; `LlamaServerClient` implementado em Rust nesta branch | Adapter dedicado OpenAI, OpenRouter chat, 9router, OmniRouter, Kimi ou Ollama. URL customizada apenas permite tentar protocolo compatível. |
| Embeddings     | OpenAI-compatible, OpenRouter, Cohere, Voyage e Ollama                                                                                                          | OpenAI genérico significa conformidade integral; nenhum perfil por modelo garante dimensão/modalidade.                                     |
| Rerank         | Cohere, Voyage e OpenRouter                                                                                                                                     | llama.cpp ou equivalência de campos/scores em todos os adapters.                                                                           |
| Mídia no chat  | Tipos image/audio/video/document/file, asset map, preparação e envio em partes dos protocolos                                                                   | Cada provider/modelo aceita todos os formatos; geração de imagem, áudio ou vídeo.                                                          |
| Decision Model | Dispatch/policy do harness retorna decisões de autorização de tool                                                                                              | Inferência por modelo com perguntas tipadas/probabilidades (`/v1/systemone`). São conceitos distintos.                                     |
| TTS/STT        | Não há cliente LLM para TTS/STT                                                                                                                                 | Helpers de mídia no consumidor Vectora pertencem à Engine ou estão prontos para binding.                                                   |

O estado em `main` permanece o snapshot anterior ao adapter. O PR #98
implementa `LlamaServerClient` em Rust com health, discovery, metadata de modelo
preservada e chat/stream compartilhado. O PR filho #100 expõe health, models,
completion e stream pelos bindings Python/TypeScript, incluindo requests
efêmeros; fixtures locais cobrem SSE nos dois. Isso não prova integração com
servidor llama-server real nem suporte de cada modalidade/modelo.

## Providers nomeados: documentos primários e gaps no core

| Provider/perfil  | Capacidades documentadas na API                                                                                                   | Estado na Engine                                                                                                                                                                     |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| OpenAI           | Chat/Responses, input vision, image generation/edit, embeddings, file/realtime transcription, TTS e realtime audio                | Apenas tentativa por protocolo OpenAI-compatible em chat/embeddings; falta perfil e adapters dedicados por operação.                                                                 |
| Anthropic        | Messages, tools e visão por image blocks                                                                                          | Chat nativo presente; imagem é entrada em Messages. Sem geração de mídia, áudio/vídeo ou embedding Anthropic no core.                                                                |
| Google Gemini    | Conteúdo multimodal incluindo vídeo, image generation, TTS e Live audio                                                           | Chat nativo presente. Não há operações Engine para imagem, TTS, STT ou Live.                                                                                                         |
| Cohere           | Chat, embeddings e rerank                                                                                                         | As três operações estão implementadas em superfícies separadas; sem implicar outras modalidades.                                                                                     |
| OpenRouter       | Chat multimodal, geração de imagem/vídeo, TTS, STT, embeddings, rerank e áudio                                                    | Embeddings/rerank dedicados; chat pode usar URL genérica. Faltam adapters dedicados para os demais endpoints.                                                                        |
| 9Router          | A página oficial anuncia OpenAI-compatible chat, embeddings, TTS, STT, image, vision, video, search e fetch                       | Sem perfil nem API/schema por operação no código. A fonte pública consultada não bastou para implementar requests robustos.                                                          |
| OmniRouter `.ru` | Chat/Messages/Responses, Decision Model, transcrição, geração de imagem e áudio/música                                            | Sem adapters. Decision Model é endpoint específico; imagem/áudio são jobs idempotentes assíncronos.                                                                                  |
| Kimi/Moonshot    | API OpenAI/Anthropic-compatible; Kimi Code documenta video upload, enquanto a fonte oficial Kimi K2 diz que K2 não suporta vision | Sem perfil. Capability específica do modelo não pode ser atribuída a todo Kimi.                                                                                                      |
| Ollama           | API nativa para chat/vision e embeddings de texto; API OpenAI-compatible documentada como subconjunto                             | Embeddings nativos implementados; sem chat Ollama dedicado ou perfil/capability discovery. O `/api/embed` normaliza vetores; o subconjunto `/v1` tem diferenças que exigem fixtures. |

A evidência detalhada e URLs oficiais estão em
[`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md).
`OmniRouter` é um nome ambíguo: `api.omnirouter.ru`/`.cloud` e `omnirouter.li`
documentam APIs diferentes. Uma integração dedicada fica pendente de escolha
no chat; o trabalho independente em `llama-server` não depende dela.

## O que a tag llama.cpp v1.0.0 documenta

- `/v1/chat/completions`, `/v1/responses` e `/v1/messages` Anthropic-compatível;
  a própria documentação evita prometer conformidade integral com os originais.
- `/v1/models` declara `architecture.input_modalities` e
  `architecture.output_modalities`; `decisions` e `audio` têm semântica
  especial. Se os metadados do modelo faltam, a documentação diz que o servidor
  devolve `text` nos dois arrays; `video` só é informado depois que o modelo
  carrega. Isso deve ser registrado como resposta/fallback do servidor, não como
  prova de que o modelo só aceite texto.
- `/v1/systemone` é API TypeSafe sem stream, com perguntas tipadas e input
  multimodal opcional. Deve virar contrato `DecisionModel` separado do chat.
- `/v1/audio/speech` converte para `/tts`; WAV/PCM, sample rate e vozes são do
  modelo, e parâmetros OpenAI podem ser ignorados/rejeitados.
- `/v1/realtime` suporta sessão WebSocket para ASR/transcrição e conversa de
  áudio, com PCM16, eventos e controle de sessão; não é transcrição batch de
  arquivo.
- Chat aceita partes `image_url`, `input_audio` e `input_video` quando o modelo
  e a configuração suportam; áudio é decodificado por miniaudio, vídeo por
  FFmpeg e pode carregar soundtrack. Embeddings aceitam entrada multimodal em
  extensão que não faz parte da API OpenAI Embeddings padrão.
- A release inclui modelos/capacidades específicas (inclusive TTS Drama-3 e
  arquiteturas vision); suporte do runtime a um modelo não implica que todas as
  rotas/API tenham a mesma modalidade.
- Geração de imagem não aparece como endpoint do servidor v1.0.0; a capacidade
  requer outro provider real. Não inventar route.

## Decisões técnicas recomendadas para implementação

1. Implementar `llama-server` por HTTP sobre reqwest, sem dependência C++ nativa;
   isso oferece portabilidade de bindings e usa precisamente a interface do
   produto upstream.
2. Manter `protocol` distinto de `provider_profile`. Protocol codecs são
   compartilhados; perfis registram endpoints/defaults/headers e diferenças
   confirmadas com fixtures.
3. Usar discovery de `/v1/models` preservando os arrays e o estado. Quando o
   servidor indicar fallback `text` porque não pôde ler metadados, não promover
   essa resposta a prova de suporte nem de exclusão de mídia. Falha de discovery
   deixa capability remota desconhecida; mídia não é enviada às cegas.
4. Modelar separadamente `DecisionModel`, speech synthesis, transcription/realtime
   audio, image generation e multimodal chat. Não inflar `ChatRequest` com APIs
   que têm lifecycle e resposta diferentes.
5. Usar teste HTTP mock para todos os contratos e servidor real v1.0.0 com
   modelo pequeno apenas em ambiente configurado; no CI padrão não baixar pesos.

Essas escolhas são proposta de implementação coerente com as regras locais. Se
um detalhe alterar contrato público, segurança ou dependências, registrar opções
e perguntar a `brunosrz` no chat antes da parte dependente.

## Questões ainda abertas

- Matriz exata de providers e modalidades exigidas no charter de primeira
  release: o pedido inclui os providers nomeados e as modalidades gerais, mas
  os endpoints distintos e gaps reais precisam de aceites por operação.
- Contrato de metadados/capability para modelos remotos que não oferecem
  `/v1/models` estendido: perfil do provider, override do host e discovery
  possuem níveis distintos de confiança.
- Contrato de batch STT e image generation: llama.cpp v1.0.0 não fornece os
  endpoints equivalentes a batch transcription / geração de imagem; escolher
  providers específicos exige definir escopo e paridade.
- Runtime lifecycle para WebSocket realtime: ownership da sessão entre core e
  host e limites de buffers precisam ser fixados antes do adapter final.

## Limites da verificação

Não foi iniciado `llama-server`, baixado modelo, nem chamada externa a provider.
O inventário prova a presença/ausência de adapters no código local e os
endpoints declarados no upstream v1.0.0; não prova interoperabilidade real nem
capacidade de cada modelo.

## Atualização — Decision Model llama-server (2026-11-15)

A etapa seguinte da stack começa na branch `feat/llama-server-decision-model`, filha do PR #102. O core define `DecisionRequest`, perguntas `choice`/`score`/`noul` e resposta que preserva campos de probabilidade; `/v1/systemone` é não-streaming conforme a API fixada na spec. A validação aceita somente data URLs base64 de imagem/áudio no campo `files`, com limite de mídia; usa auth, timeout e limite de corpo do perfil. A resposta exige exatamente os IDs perguntados. Fixture HTTP Rust passou para rota, bearer token, choice, probabilidades e usage. Wrappers/fixtures Python e TypeScript foram iniciados; build e testes dos bindings ainda pendentes.

## Atualização — TTS bufferizado llama-server (2026-11-15)

O PR [#104](https://github.com/vectora-company/engine/pull/104), filha do PR #103, head `8763968` (`MERGEABLE`), implementa duas superfícies
conforme a API fixada em [`llama.cpp v1.0.0`](https://github.com/ggml-org/llama.cpp/blob/v1.0.0/tools/server/README.md): `/v1/audio/speech` (WAV/PCM16, voz opcional e speed=1.0) e `/tts` (WAV, PCM float32 ou PCM16; parâmetros nativos e speaker-reference). O retorno é binário, preserva Content-Type e extrai sample rate do header de áudio ou RIFF/WAVE. O formato de request persistível não contém o áudio de referência; `speaker_ref_b64` está isolado no envelope efêmero com Debug redigido e limite de bytes. A rota com voz explícita consulta `/props`; discovery exige model ID e saída `audio`.

Fixtures Rust passaram (8 testes focados, incluindo ambas as rotas); `cargo check` passou para core e crates Python/Node; TypeScript typecheck passou; Node executou 41 testes aprovados e 11 skips relacionados a serviços externos sem endpoint. Build maturin e fixture pytest focado passaram; pytest do binding Python passou com 50 testes e 1 skip. Não há integração com um llama-server/modelo real neste host. Streaming TTS, realtime WebSocket/ASR e cancelamento ficam pendentes para o PR seguinte, sem afirmar batch STT. O store pnpm deste checkout retorna `/home/brunosrz/.local/share/pnpm/store/v11`; os pacotes e build são do checkout em Área de trabalho.

## Atualização — sessão WebSocket realtime (2026-11-15)

A branch `feat/llama-server-realtime-audio`, filha do PR #104, implementa sessão `/v1/realtime` segundo llama.cpp v1.0.0. O contrato acomoda transcription e conversa; PCM16 mono usa sample rate nativo do modelo por padrão, ou override validado. Commit/evento controlam o buffer, fechamento cancela a sessão, e eventos JSON desconhecidos são preservados. Fixture Rust cobre handshake autenticado, query, atualização de rate, PCM16, commit e eventos de transcrição.

Verificação local concluída: workspace Rust com 205 testes da lib, 3 fixtures e teste do crate Python aprovados; Clippy `-D warnings` passou depois de corrigir a lint `manual_is_multiple_of`; build NAPI release e TypeScript typecheck passaram; Node registrou 41 pass e 11 skips de serviços opcionais sem configuração; `maturin develop` e pytest passaram (55 pass, 1 skip); Ruff, format e `ty` passaram. Uma rodada de hooks globais reportou arquivos SVG e Markdown/TypeScript preexistentes reformatados pelo hook; arquivos fora do escopo foram restaurados. Não há teste contra llama-server/modelo realtime real. O `node_modules` usado está dentro deste checkout em Área de trabalho, com store configurado em `/home/brunosrz/.local/share/pnpm/store/v11`; nenhum build foi executado em `/media`.

O PR #102 agora retorna `MERGEABLE` no GitHub, então o conflito mostrado na captura não está presente no estado remoto consultado. O PR #105 está pronto para publicação como filho do #104; o trabalho prossegue sem aguardar CI. Limites mantidos: teste de interoperabilidade com modelo real, batch STT e streaming TTS não fazem parte do endpoint realtime implementado. Estado operacional e próximos passos em [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](../plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md), checklist em [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](../checklists/05-providers-e-modalidades-llama-cpp-v1.md) e spec em [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md).

## Etapa ativa: streaming TTS (15/11/2026)

A branch `feat/llama-server-streaming-tts`, filha do PR #105, já implementa
stream binário nas duas APIs do llama-server v1.0.0. As chamadas são
incrementais, com chunks limitados, timeout e limite total; metadados de
content type/sample rate são mantidos, e falhas após bytes parciais são
reportadas ao consumidor. Wrappers Python e TypeScript expõem iteradores
assíncronos com backpressure/cancelamento através de canal limitado. Testes
Rust cobrem resposta incremental, erro após áudio parcial e limites. Workspace
Rust e Clippy passaram; pytest passou (56/1 skip), Node passou (42/11 skips) e
build NAPI/typecheck passaram. A criação do PR ainda está pendente; integração
com modelo TTS real não foi executada. Consulte
[`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](../plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md),
[`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md) e
[`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](../checklists/05-providers-e-modalidades-llama-cpp-v1.md).

A documentação fixa duas superfícies upstream para gerar áudio incrementalmente: `/v1/audio/speech` aceita `stream_format: "audio"` e rejeita `sse`; `/tts` aceita `stream: true`. WAV streamed tem header com tamanho desconhecido (0xFFFFFFFF), portanto a Engine deve preservar os bytes e sinalizar Content-Type/sample rate sem acumular ou reescrever o arquivo. O contrato do PR #104 só retorna áudio bufferizado; o trabalho seguinte será um iterador de chunks com backpressure e cancelamento em Rust/Python/TypeScript, empilhado sobre [#105](https://github.com/vectora-company/engine/pull/105). Fonte normativa: [llama.cpp v1.0.0 HTTP server README](https://github.com/ggml-org/llama.cpp/blob/v1.0.0/tools/server/README.md), seções `/tts` e `/v1/audio/speech`.
