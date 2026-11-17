# Auditoria de providers e modalidades

## Escopo e método

Esta análise separa APIs upstream, adapters existentes e operações ainda não
implementadas. A unidade de suporte é provider × protocolo × operação × modelo;
nomes de marcas ou endpoints compatíveis não provam equivalência. O código Rust
é a fonte da lógica, e Python/TypeScript são conferidos como bindings.

Referência local: [`llama.cpp v1.0.0`](https://github.com/ggml-org/llama.cpp/tree/v1.0.0).
Fontes de protocolo específicas ficam nos trechos da
[spec 18](../specs/18-providers-e-modalidades-llama-cpp-v1.md).

## Estado observado no código

| Operação/protocolo          | Implementação observada                                                                                                             | Limite factual                                                                                                                                                          |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Chat genérico               | OpenAI-compatible com endpoints configuráveis; fallback ordenado; Anthropic Messages, Gemini, Cohere e Ollama nativos               | O fallback só é seguro antes de qualquer evento/saída entregue. Compatibilidade de wire format não garante paridade funcional.                                          |
| OpenAI                      | Chat Completions e Responses selecionáveis separadamente                                                                            | Responses tem conversão própria de histórico, tools, reasoning/usage e eventos semânticos; APIs de imagem, áudio, realtime e built-in tools não decorrem desse adapter. |
| OpenRouter                  | Perfil nomeado sobre transporte compatível, headers de atribuição e extensions por request                                          | Roteamento e capacidades variam com modelo/configuração do gateway.                                                                                                     |
| 9router / OmniRouter / Kimi | Transporte genérico configurável quando o endpoint implementa o protocolo suportado                                                 | Não há validação suficientemente específica para declarar cada serviço coberto. “OmniRouter” também pode apontar a serviços distintos.                                  |
| Llama-server                | Cliente dedicado com health, discovery, chat, Decision Model, embeddings, rerank, TTS e realtime de áudio                           | Discovery é declaração do servidor. Não substitui smoke test do modelo ou de cada modalidade.                                                                           |
| Laya standalone             | Cliente Jev separado, health e Decision Model de texto, bindings e CLI                                                              | Não oferece chat genérico nem discovery do llama-server; falta integração local com servidor/checkpoint real.                                                           |
| Embeddings                  | OpenAI-compatible, OpenRouter, Cohere, Voyage, Ollama e llama-server; llama-server também aceita entradas multimodais via asset map | Dimensão, modalidade e formato seguem próprios de cada protocolo/modelo.                                                                                                |
| Rerank                      | Cohere, Voyage, OpenRouter e llama-server; modalidade textual nos adapters declarados, texto/imagem efêmera no llama-server         | Scores e ordenação não devem ser comparados sem normalização contratual.                                                                                                |
| Áudio                       | TTS bufferizado/streaming e sessão realtime de transcrição/conversa no llama-server                                                 | Batch STT genérico permanece ausente; TTS não prova STT.                                                                                                                |
| Mídia/imagem/vídeo          | Tipos, asset map, envelopes persistível/efêmero, decisões por parte e conversões cobertas por adapters                              | Geração de imagem, operações gerais de vídeo, catálogo automático confiável e limites agregados permanecem pendentes.                                                   |

## Análise de paridade dos providers

1. **Perfis compartilham transporte somente quando comprovado.** OpenAI
   Chat Completions, gateways OpenAI-compatible e Ollama OpenAI-compatible não
   compartilham necessariamente auth, opções, reasoning, erro ou stream. Os
   fixtures devem manter essas diferenças visíveis.
2. **Perfis nativos preservam capacidades específicas.** Anthropic, Gemini,
   Cohere, Ollama e Responses têm formatos próprios. Converter para um
   contrato comum deve rejeitar ou preservar dados cuja semântica não possa ser
   representada; não descartar silenciosamente.
3. **Modalidade pertence à operação e ao modelo.** “Suporta visão” não implica
   PDF, áudio, vídeo, imagem em resultado de tool ou geração de imagem. Dados
   de discovery permanecem declarativos até verificação adequada.
4. **Laya e llama-server são integrações distintas.** O servidor standalone
   Jev atende decisão; o GGUF servido por llama.cpp usa endpoints do
   llama-server. Ter o mesmo modelo/weights não unifica protocolos.

## Evidência de teste

Na verificação local de 17/11/2026:

- As suites Rust, Python e Node exercitaram contratos offline e fixtures de
  providers; `scons tests` passou com 283 testes Rust (incluindo fixtures e
  teste nativo do binding Python), 102 Python e 56 Node. Nove Python e 16 Node
  foram ignorados por dependerem de serviços sem endpoint configurado.
- `scons lint` passou: formatting, Clippy, rustdoc, Ruff, `ty`, Bandit,
  Oxlint e typecheck TypeScript.
- O endpoint real Qdrant e os adapters LanceDB foram usados para a capability
  de filtro vetorial; isso não é evidência de integração com providers/modelos
  de IA reais.
- Não foram configurados credenciais remotas, llama-server com modelo, Ollama
  daemon ou checkpoint Laya para esta rodada. As integrações correspondentes
  permanecem não verificadas em serviço real.

Contagens pertencem àquela execução e não são garantia para outro SHA,
ambiente ou configuração. O processo reutilizável está no
[loop](../plans/05-loop-de-desenvolvimento.md).

## Gaps e próximos critérios

- Construir e manter uma tabela verificável provider × endpoint × operação ×
  modalidade × modelo, incluindo auth, erros, limites, stream e status real.
- Cobrir limites agregados de mídia e catálogo/metadata confiáveis; ausência de
  dado precisa continuar como `unknown` e falhar de modo conservador.
- Definir contratos antes de adicionar geração de imagem, batch STT e operações
  gerais de vídeo; especificar suporte concreto, custos, streaming e bytes.
- Executar smokes locais opt-in com versões fixadas de llama-server, Ollama e
  Laya quando os serviços/modelos estiverem disponíveis; para providers
  remotos, usar credenciais de teste sem expô-las em logs/fixtures.
- Completar conformidade compartilhada de cancelamento, redaction, respostas
  malformadas e stream truncado em Rust, Python e TypeScript.

## Referências

- [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md)
- [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](../plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md)
- [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](../checklists/05-providers-e-modalidades-llama-cpp-v1.md)
- [`docs/specs/17-midia-no-chat.md`](../specs/17-midia-no-chat.md)
- [llama.cpp v1.0.0 server API](https://github.com/ggml-org/llama.cpp/tree/v1.0.0/tools/server)
