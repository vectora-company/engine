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

| Operação                                  | Estado atual                                                                   | Evidência/cobertura desejada                                                |
| ----------------------------------------- | ------------------------------------------------------------------------------ | --------------------------------------------------------------------------- |
| Chat texto/stream                         | Parcial implementado                                                           | OpenAI-compatible; nativos Anthropic/Gemini/Cohere; fixtures por protocolo. |
| OpenAI/OpenRouter/9router/OmniRouter/Kimi | Transporte genérico possível quando OpenAI-compatible; perfis não equivalem    | Rota, auth/headers, erros, streaming, uso e modalidade por endpoint.        |
| Ollama chat                               | Sem adapter dedicado                                                           | API Ollama distinta e perfil OpenAI `/v1` validado separadamente.           |
| llama.cpp chat/discovery                  | Perfil Rust com health, `/v1/models`, metadata e chat/stream; binding pendente | Conformidade Python/TypeScript, capability e multimodal por modelo.         |
| Decision Model                            | Ausente como API LLM                                                           | Contrato tipado separado; adapter `/v1/systemone`; probs e multimodal.      |
| Entrada de imagem/visão                   | Tipos e preparação existem; roteamento parcial                                 | Casos reais por protocolo/modelo e capability desconhecida fail-closed.     |
| Geração de imagem                         | Ausente                                                                        | Adapter próprio apenas quando endpoint estiver confirmado.                  |
| TTS                                       | Ausente                                                                        | `/v1/audio/speech`: WAV/PCM, voz, sample rate, stream e cancelamento.       |
| STT                                       | Ausente                                                                        | `/v1/realtime` é streaming ASR; batch transcription requer API separada.    |
| Entrada de vídeo                          | Tipo existe; cobertura de envio limitada                                       | Chat multimodal v1.0.0 e limites, mídia efêmera e teste por modelo.         |
| Geração de vídeo                          | Ausente                                                                        | Não confundir com vídeo de entrada.                                         |
| Embeddings                                | OpenAI-compatible/OpenRouter/Cohere/Voyage/Ollama                              | Extensão multimodal do llama.cpp em contrato versionado.                    |
| Rerank                                    | Cohere/Voyage/OpenRouter                                                       | Validar rota `llama-server` v1.0.0 e normalizar índices/scores.             |

## Gates para etapas de código

- [x] Core Rust: settings endpoint seguros, health `/health`, discovery `/v1/models` e chat/stream compartilhado.
- [x] Fixtures HTTP e parsing: cinco testes focados e 191 testes da lib passam; Clippy `-D warnings` para lib e testes e rustdoc passaram.
- [ ] Expor profile callable em Python e TypeScript, incluindo streaming e contexto de mídia.
- [ ] Executar integração opt-in com `llama-server` v1.0.0 fixado.
- [ ] Protocolo, operação e provider/profile são campos distintos e tipados.
- [x] Discovery preserva os arrays brutos e metadata extra; campos ausentes ficam `None`/unknown. O `text` fallback do servidor não vem marcado e não é promovido a garantia. Falha de rede permanece erro/unknown.
- [ ] Requisições multimodais usam bytes efêmeros e respeitam auth/autorização do host.
- [ ] Erros de provider preservam código/status sem vazar segredo ou payload.
- [ ] Timeouts, limites, cancelamento, backpressure e retry são definidos por operação.
- [ ] Fallback só pode ocorrer antes da primeira saída observável.
- [ ] Fixtures comuns exercitam Rust e bindings Python/TypeScript.
- [ ] Integração real fixa tag de servidor/modelo e é opt-in, sem baixar modelo grande no CI por padrão.
- [ ] APIs `systemone`, TTS, realtime ASR, geração de imagem e vídeo têm contratos separados.
- [ ] Documentação anuncia suporte por modalidade/provider/modelo com evidência.

## Gate do charter de primeira release

- [ ] A matriz de providers/modos planejados está completa e com lacunas explícitas.
- [ ] Core e bindings cobrem os contratos de release com conformidade.
- [ ] Instalação e empacotamento suportados são reproduzíveis.
- [ ] Gaps independentes de storage, contexto/RAG, segurança e adoção foram fechados ou deliberadamente excluídos do charter por decisão registrada.
