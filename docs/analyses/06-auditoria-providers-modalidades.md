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
preservada e chat/stream compartilhado. O PR filho #99 expõe health, models,
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
