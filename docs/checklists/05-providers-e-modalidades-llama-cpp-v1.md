# Checklist — providers e modalidades de IA (llama.cpp v1.0.0)

## Preparação desta etapa documental

- [x] Confirmar o merge do PR #96; não há PR aberto e esta trilha parte de `main`.
- [x] Confirmar `main` atual e checkout permitido da Área de trabalho.
- [x] Ler adapters, settings, bindings, plano anterior e contratos de mídia.
- [x] Consultar a release e README do `llama-server` na tag v1.0.0.
- [x] Inventariar providers e operações existentes separando compatibilidade genérica de adapter dedicado.
- [x] Marcar cada fato como implementação, perfil genérico, limitação ou proposta.
- [x] Revisar os diffs e as referências upstream fixadas na tag v1.0.0.
- [x] Revisar conteúdo e links, inclusive os índices e documentos do goal loop; manter o PR documental sobre `main`.

## Matriz de capacidade (estado observado)

| Operação                                  | Estado atual                                                                         | Evidência/cobertura desejada                                                                                                                                 |
| ----------------------------------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Chat texto/stream                         | Parcial implementado                                                                 | OpenAI-compatible; nativos Anthropic/Gemini/Cohere; fixtures por protocolo.                                                                                  |
| OpenAI/OpenRouter/9router/OmniRouter/Kimi | OpenRouter chat named profile iniciado; demais perfis ainda genéricos ou sem adapter | Rota, auth/headers, erros, streaming, uso e modalidade por endpoint; gateway routing por request.                                                            |
| Ollama chat                               | Sem adapter dedicado                                                                 | API Ollama distinta e perfil OpenAI `/v1` validado separadamente.                                                                                            |
| llama.cpp chat/discovery                  | Perfil Rust mais APIs health/models/chat/stream nos bindings                         | Conformidade por fixture; integração real e modalidades por modelo pendentes.                                                                                |
| Decision Model                            | `/v1/systemone` implementado no core e bindings (PR #103)                            | Perguntas choice/score/noul, capability decisions, request persistível e envelope efêmero multimodal, probabilidades validadas e fixtures nos três runtimes. |
| Entrada de imagem/visão                   | Tipos e preparação existem; roteamento parcial                                       | Casos reais por protocolo/modelo e capability desconhecida fail-closed.                                                                                      |
| Geração de imagem                         | Ausente                                                                              | Adapter próprio apenas quando endpoint estiver confirmado.                                                                                                   |
| TTS                                       | Síntese bufferizada `/v1/audio/speech` e nativa `/tts` implementadas no PR #104      | WAV/PCM16/PCM float e bytes com Content-Type/sample rate; stream e cancelamento ainda pendentes.                                                             |
| STT                                       | Ausente                                                                              | `/v1/realtime` é streaming ASR; batch transcription requer API separada.                                                                                     |
| Entrada de vídeo                          | Tipo existe; cobertura de envio limitada                                             | Chat multimodal v1.0.0 e limites, mídia efêmera e teste por modelo.                                                                                          |
| Geração de vídeo                          | Ausente                                                                              | Não confundir com vídeo de entrada.                                                                                                                          |
| Embeddings                                | OpenAI-compatible/OpenRouter/Cohere/Voyage/Ollama                                    | Extensão multimodal do llama.cpp em contrato versionado.                                                                                                     |
| Rerank                                    | Cohere/Voyage/OpenRouter                                                             | Validar rota `llama-server` v1.0.0 e normalizar índices/scores.                                                                                              |

## Gates para etapas de código

- [x] Core Rust: settings endpoint seguros, health `/health`, discovery `/v1/models` e chat/stream compartilhado.
- [x] Fixtures HTTP e parsing: cinco testes focados e 191 testes da lib passam; Clippy `-D warnings` para lib e testes e rustdoc passaram.
- [x] Publicar o adapter Rust no PR empilhado [#98](https://github.com/vectora-company/engine/pull/98), base `docs/llama-cpp-v1-provider-review`, head `5a463bc`.
- [x] Expor health, discovery, completion, stream e variantes efêmeras Python/TypeScript no PR filho [#100](https://github.com/vectora-company/engine/pull/100); fixtures passam nos dois bindings.
- [x] Atualizar stubs `_native.pyi`; Ruff e `ty check bindings/python utils` passam.
- [ ] Executar integração opt-in com `llama-server` v1.0.0 fixado.
- [ ] Cobrir modalidades multimodais no adapter llama-server além das rotas-base.
- [ ] Protocolo, operação e provider/profile são campos distintos e tipados.
- [x] Discovery preserva os arrays brutos e metadata extra; campos ausentes ficam `None`/unknown. O `text` fallback do servidor não vem marcado e não é promovido a garantia. Falha de rede permanece erro/unknown.
- [ ] Requisições multimodais usam bytes efêmeros e respeitam auth/autorização do host.
- [ ] Erros de provider preservam código/status sem vazar segredo ou payload.
- [ ] Timeouts, limites, cancelamento, backpressure e retry são definidos por operação.
- [ ] Fallback só pode ocorrer antes da primeira saída observável.
- [x] Fixtures HTTP exercitam Rust e bindings Python/TypeScript com metadata de capability, auth, perguntas, resposta probabilística e envelope efêmero.
- [ ] Integração real fixa tag de servidor/modelo e é opt-in, sem baixar modelo grande no CI por padrão.
- [x] `systemone` tem contrato separado e fixture de rota/probabilidades no core Rust.
- [x] `systemone` é exposto e verificado nos bindings Python e TypeScript (fixture local).
- [x] Síntese TTS bufferizada e `/tts` têm formato binário separado do chat; áudio de referência só usa envelope efêmero.
- [ ] Streaming TTS e realtime ASR têm contratos de sessão separados; geração de imagem e vídeo também.
- [ ] Documentação anuncia suporte por modalidade/provider/modelo com evidência.

## Gate do charter de primeira release

- [ ] A matriz de providers/modos planejados está completa e com lacunas explícitas.
- [ ] Core e bindings cobrem os contratos de release com conformidade.
- [ ] Instalação e empacotamento suportados são reproduzíveis.
- [ ] Gaps independentes de storage, contexto/RAG, segurança e adoção foram fechados ou deliberadamente excluídos do charter por decisão registrada.

## Etapa TTS bufferizada — PR #104, filha do #103

- [x] Implementar `SpeechRequest`, `NativeSpeechRequest`, request efêmero para
      speaker reference e resposta binária com formato, Content-Type e sample rate.
- [x] Implementar `/v1/audio/speech` WAV/PCM e `/tts` WAV/PCM16/PCM float;
      validar discovery de saída `audio`, voz em `/props`, limite de mídia e
      resposta HTTP limitada.
- [x] Expor os métodos em Python (`bytes`) e TypeScript (`Uint8Array`) sobre Rust.
- [x] Fixtures Rust 8/8 focados; Node 41 passaram, 11 skipped por serviços
      externos não configurados; `cargo check` dos crates Rust/Python/Node e
      TypeScript typecheck passaram.
- [x] Build maturin e fixture pytest focado passaram; pytest do binding Python: 55 passaram, 1 skipped.
- [ ] Integração opt-in contra llama-server v1.0.0/modelo TTS real.
- [ ] Implementar stream TTS/realtime ASR, desconexão e cancelamento na etapa filha do [PR #104](https://github.com/vectora-company/engine/pull/104).

## Sessão realtime/ASR — filha do PR #104

- [x] Usar `tokio-websockets` 0.13.3, a versão estável já presente no lockfile, para `ws`/`wss` com autenticação e headers do profile.
- [x] Validar model ID/capability de entrada audio; intents transcription/conversation.
- [x] Adicionar send PCM16 mono (chunks), commit final/não final, sample-rate update validado e evento raw forward-compatible.
- [x] Implementar timeout, payload máximo, backpressure e fechamento da sessão.
- [x] Fixture Rust local exercita handshake autenticado, rate update, áudio base64, commit e eventos delta/completed.
- [x] Expor start/send/event/receive/close em Python e TypeScript pelo core Rust.
- [x] Build NAPI release, typecheck e Node tests: 41 passaram, 11 foram pulados por falta dos serviços externos opcionais.
- [x] `maturin develop` e pytest: 55 passaram, 1 foi pulado.
- [x] Workspace Rust: 205 testes da lib, 3 fixtures e teste nativo Python passaram; Clippy `-D warnings` passou após corrigir `manual_is_multiple_of`.
- [x] Hooks completos executados; todos passaram exceto Clippy na primeira rodada, corrigido e revalidado. Prettier/SVGO tocaram arquivos preexistentes fora do escopo; esses arquivos foram restaurados.
- [ ] Integração opt-in com llama-server v1.0.0 e modelo realtime compatível.
- [ ] Publicar PR #105 como filho do PR [#104](https://github.com/vectora-company/engine/pull/104); continuar etapas seguintes sem aguardar CI.

## Streaming TTS — branch filha do PR #105

- [x] Implementar stream binário de `/v1/audio/speech` com `stream_format: "audio"` e `/tts` com `stream: true`.
- [x] Preservar bytes, Content-Type e sample rate sem acumular toda a resposta.
- [x] Expor iterador assíncrono de chunks de áudio em Rust/Python/TypeScript com backpressure e cancelamento.
- [x] Limitar tamanho total e chunks; sinalizar falha/truncamento sem afirmar áudio completo.
- [x] Criar fixture HTTP com chunks atrasados e falha no meio; verificar consumo incremental nos dois bindings.
- [x] Workspace Rust 208 testes + 3 fixtures; Clippy `-D warnings`; pytest Python 56 pass/1 skip; NAPI build + TypeScript typecheck; Node 42 pass/11 skips; Ruff/format/ty.
- [x] Atualizar docs, publicar o PR [#106](https://github.com/vectora-company/engine/pull/106) como filho do #105; GitHub confirma `OPEN`/`MERGEABLE`.

## Ollama chat nativo — PR #107, filho do PR #106

- [x] Confirmar contrato oficial `/api/chat` JSON/NDJSON e diferenças em relação ao subconjunto OpenAI-compatible.
- [x] Implementar tradução do contrato de chat, `options`, extensões nativas, imagens inline, tools, reasoning e usage no core Rust.
- [x] Adicionar suporte a api_key opcional no endpoint nativo local; manter auth Bearer quando configurada.
- [x] Adicionar testes unitários de payload e resposta/stream no core.
- [x] Adicionar fixtures de integração nos bindings Python e TypeScript.
- [x] Rodar workspace Rust (210 testes + 3 fixtures), Clippy, pytest (57/1 skip), NAPI/typecheck, Node (43/11 skips), Ruff/format/ty; todos passaram. Ollama real não está ativo localmente.
- [x] Atualizar plano, spec, análise e checklists; hooks pre-commit completos passaram.
- [x] Publicar o PR [#107](https://github.com/vectora-company/engine/pull/107), filho do #106; GitHub confirma `OPEN`/`MERGEABLE`.
- [ ] Testar com daemon/modelo Ollama real quando o serviço estiver disponível; fixtures HTTP cobrem o protocolo sem depender do daemon.

## OpenRouter chat — PR #108, filho do PR #107

- [x] Confirmar endpoint Chat Completions, headers de atribuição e campos de roteamento nas docs oficiais.
- [x] Definir perfil nomeado `openrouter`, URL oficial padrão e suporte a `base_url` customizada; exigir chave no perfil.
- [x] Definir atribuição opcional de app via `HTTP-Referer`/`X-OpenRouter-Title`, com override por header explícito.
- [x] Encaminhar `provider`, `models`, `route`, `user` e `plugins` via extensions por request, sem default global de privacidade/custo.
- [x] Adicionar testes de contrato Rust e fixtures HTTP em Python e TypeScript.
- [x] Suites: workspace Rust 212 + 3 fixtures + binding Python nativo; pytest 58/1 skip; Node 44/11 skips; typecheck, Clippy, Ruff/format/ty/Prettier passaram.
- [x] Hooks pre-commit completos passaram; PR [#108](https://github.com/vectora-company/engine/pull/108), filho do #107, está `OPEN`/`MERGEABLE`.
- [ ] Integração real depende de endpoint/credencial OpenRouter e não foi executada; fixtures verificam o contrato sem rede externa.
