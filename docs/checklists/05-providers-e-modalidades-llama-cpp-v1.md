# Checklist: providers e modalidades de IA

Escopo definido no [plano por capability](../plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md),
na [spec](../specs/18-providers-e-modalidades-llama-cpp-v1.md) e na
[análise de protocolos](../analyses/06-auditoria-providers-modalidades.md).
Os estados descrevem implementação no checkout auditado, não adoção pelo
consumidor nem estado de merge. Atualize-os após conferir código e testes.

Estados: `feito`, `parcial`, `revisar` e `pendente`.

## Provedores e protocolos

| Estado  | Capability                                    | Verificação e limite                                                                                                                                                                |
| ------- | --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| parcial | OpenAI-compatible, 9router, OmniRouter e Kimi | Transporte e endpoint customizável existem. Validar cada serviço/rota/auth/headers/erros/modalidades; nome compatível não garante suporte integral.                                 |
| feito   | Anthropic Messages, Gemini e Cohere chat      | Adapters nativos no core; fixtures de request, resposta e stream nos bindings. Conferir suporte de mídia por operação antes de rotear.                                              |
| feito   | Ollama `/api/chat`                            | JSON/NDJSON, reasoning, ferramentas, options e auth opcional cobertos por fixtures; daemon real não foi verificado nesta execução.                                                  |
| parcial | OpenAI Chat Completions e Responses           | Protocolos separados. Responses serializa imagem e áudio WAV/MP3; capability de áudio permanece desconhecida sem metadata por modelo/override. Smokes reais pendentes.              |
| feito   | OpenRouter                                    | Perfil dedicado e transporte genérico; atribuição e campos de roteamento testados. Não transfere automaticamente suporte de modalidade entre modelos.                               |
| parcial | llama.cpp `llama-server` v1.0.0               | Perfil, discovery, chat, Decision Model, embeddings, rerank, TTS e realtime implementados conforme endpoints cobertos. Smoke integral com servidor/modelos reais não foi executado. |
| parcial | Laya standalone `laya-serve`                  | Adapter SystemOne de health/decision e bindings existem; seletor é opcional e Jev não é checkpoint Laya. Smoke com servidor/checkpoint real permanece pendente.                     |

## Operações por modalidade

| Estado   | Operação                   | Critério/gap                                                                                                                                                                                                                                                                                                |
| -------- | -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| feito    | Chat e stream              | Fixtures de protocolos; fallback explícito e sem troca após saída parcial.                                                                                                                                                                                                                                  |
| parcial  | Decision Model             | Choice/score/noul; mídia efêmera valida bytes/quantidade e modalidades do modelo com override do consumidor; smoke real ainda pendente.                                                                                                                                                                     |
| parcial  | Visão e imagens de entrada | Requests efêmeros com asset map e orçamento compartilhado; metadata `/v1/models` pode ser convertida explicitamente. Falta catálogo automático confiável e smoke real.                                                                                                                                      |
| parcial  | Entrada de áudio           | OpenAI Responses serializa WAV/MP3 como `input_audio`; capability permanece desconhecida até metadata por modelo ou override explícito. Falta smoke real.                                                                                                                                                   |
| pendente | Geração de imagem          | Sem contrato/provider geral implementado.                                                                                                                                                                                                                                                                   |
| parcial  | TTS                        | Áudio bufferizado e streaming pelo llama-server; verificar servidor/modelo real e completar conformidade operacional.                                                                                                                                                                                       |
| parcial  | STT/transcrição            | Realtime pelo llama-server existe. Batch OpenAI-compatible `/v1/audio/transcriptions` está no core e bindings; fixtures Rust/Python/Node cobrem sucesso, 429/Retry-After sem vazar corpo e tamanho máximo da resposta. Smoke de provider/modelo real pendente. STT local de sistema pertence ao consumidor. |
| parcial  | Vídeo                      | Tipos multimodais e entrada via API coberta não significam operações gerais de vídeo, ingestão ou geração.                                                                                                                                                                                                  |
| parcial  | Embeddings                 | Texto nos adapters declarados; multimodal via asset map do llama-server com orçamento compartilhado entre inputs. Falta smoke de modelo real e catálogo confiável.                                                                                                                                          |
| parcial  | Rerank                     | Cohere, Voyage, OpenRouter e llama-server textual; rerank efêmero de texto/imagem compartilha orçamento entre query e documentos. Falta smoke real.                                                                                                                                                         |

## Contratos transversais

- [x] Rust implementa a lógica canônica; bindings encaminham ao core.
- [x] Fixtures offline cobrem adapters e perfis implementados sem credenciais.
- [x] Requests persistíveis rejeitam bytes inline e URLs `data:`; envio efêmero
      valida asset, digest, MIME e limite.
- [x] Erros de provider não expõem corpos de resposta nem credenciais.
- [x] Fallback não troca provider após emitir saída parcial.
- [ ] Publicar matriz provider × endpoint × modalidade × modelo com evidência
      atualizada e declarar explicitamente o que não é suportado.
- [x] Orçamento agregado configurável de bytes/quantidade aplicado a chat,
      Decision Model efêmero, embeddings e rerank multimodais no llama-server;
      core Rust e bindings Python/TypeScript validados.
- [x] Perfil genérico de provider não sobrepõe metadata específica do modelo;
      registros `/v1/models` podem ser convertidos para `gateway_metadata` nos
      bindings Python e TypeScript. Conversão opt-in; discovery/cache automáticos
      e catálogo completo seguem pendentes.
- [x] Decision Model efêmero do llama-server valida imagem/áudio contra metadata
      do modelo e respeita override explícito do consumidor antes do envio.
- [x] OpenAI Responses mapeia áudio WAV/MP3 efêmero para `input_audio`; sem
      metadata/override a capability permanece desconhecida nos dois perfis.
      MIME fora do contrato vira nota textual sem enviar bytes, como Chat
      Completions. Python e Node testam reidratação pelo asset map e envio;
      smoke real por modelo permanece pendente.
- [x] Separar categoria Decision Model, modelo Jev, API/protocolo SystemOne e
      runtime Laya; respostas de fixture não usam SystemOne como nome de modelo.
      Seletor de checkpoint Laya é opcional, sem default `jev-1`.
- [ ] Definir e aplicar limites de duração/resolução e propagar orçamento a
      outras operações com entrada de mídia quando suportadas.
- [ ] Executar smoke tests opt-in com versões fixadas para llama-server,
      Laya, Ollama e providers remotos autorizados; registrar apenas os
      resultados da execução atual.
- [ ] Completar conformidade compartilhada de erros, cancelamento, stream,
      autenticação e redaction entre Rust, Python e TypeScript.
- [x] Batch transcription: multipart OpenAI `/v1/audio/transcriptions`, asset
      efêmero validado e resposta tipada em Rust/Python/TypeScript.
- [x] Fixtures Rust/Python/Node, build, typecheck, lint e suíte local.
- [x] Cliente batch: `429` preserva `Retry-After` sem expor corpo do provider;
      `max_response_bytes` é respeitado no body da resposta (testes HTTP locais Rust).
- [ ] Smoke opt-in contra endpoint/modelo real; avaliar erros e limites de
      resposta sem credenciais no log.

## Verificação por mudança

- [x] Rust: fmt, Clippy, testes unitários/fixtures.
- [ ] Integração real configurada quando endpoint autorizado estiver disponível.
- [x] Python: build da binding, pytest, Ruff e `ty`.
- [x] TypeScript: build, typecheck, Oxlint e testes Node.
- [ ] Revisar payloads/fixtures para não conter credenciais, bytes reais ou
      dados sensíveis.
- [x] Atualizar plano, spec, análise, este checklist e índices afetados.
