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

| Operação                                  | Estado atual                                                                | Evidência/cobertura desejada                                                |
| ----------------------------------------- | --------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| Chat texto/stream                         | Parcial implementado                                                        | OpenAI-compatible; nativos Anthropic/Gemini/Cohere; fixtures por protocolo. |
| OpenAI/OpenRouter/9router/OmniRouter/Kimi | Transporte genérico possível quando OpenAI-compatible; perfis não equivalem | Rota, auth/headers, erros, streaming, uso e modalidade por endpoint.        |
| Ollama chat                               | Sem adapter dedicado                                                        | API Ollama distinta e perfil OpenAI `/v1` validado separadamente.           |
| llama.cpp chat/discovery                  | Chat por URL genérica; discovery ausente                                    | Fixar v1.0.0, `/v1/models`, capability de cada modelo e chat multimodal.    |
| Decision Model                            | Ausente como API LLM                                                        | Contrato tipado separado; adapter `/v1/systemone`; probs e multimodal.      |
| Entrada de imagem/visão                   | Tipos e preparação existem; roteamento parcial                              | Casos reais por protocolo/modelo e capability desconhecida fail-closed.     |
| Geração de imagem                         | Ausente                                                                     | Adapter próprio apenas quando endpoint estiver confirmado.                  |
| TTS                                       | Ausente                                                                     | `/v1/audio/speech`: WAV/PCM, voz, sample rate, stream e cancelamento.       |
| STT                                       | Ausente                                                                     | `/v1/realtime` é streaming ASR; batch transcription requer API separada.    |
| Entrada de vídeo                          | Tipo existe; cobertura de envio limitada                                    | Chat multimodal v1.0.0 e limites, mídia efêmera e teste por modelo.         |
| Geração de vídeo                          | Ausente                                                                     | Não confundir com vídeo de entrada.                                         |
| Embeddings                                | OpenAI-compatible/OpenRouter/Cohere/Voyage/Ollama                           | Extensão multimodal do llama.cpp em contrato versionado.                    |
| Rerank                                    | Cohere/Voyage/OpenRouter                                                    | Validar rota `llama-server` v1.0.0 e normalizar índices/scores.             |

## Gates para etapas de código

- [ ] Protocolo, operação e provider/profile são campos distintos e tipados.
- [ ] Discovery preserva arrays/status e distingue metadados conhecidos do fallback `text` usado pelo servidor quando não consegue ler metadados; falha de rede permanece `unknown`.
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
