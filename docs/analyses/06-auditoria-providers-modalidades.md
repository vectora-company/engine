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

| Operação/protocolo          | Implementação observada                                                                                                                                                       | Limite factual                                                                                                                                                                                                                                               |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Chat genérico               | OpenAI-compatible com endpoints configuráveis; fallback ordenado; Anthropic Messages, Gemini, Cohere e Ollama nativos                                                         | O fallback só é seguro antes de qualquer evento/saída entregue. Compatibilidade de wire format não garante paridade funcional.                                                                                                                               |
| OpenAI                      | Chat Completions e Responses selecionáveis separadamente; Responses serializa áudio WAV/MP3 como `input_audio`                                                                | Responses tem conversão própria de histórico, mídia, tools, reasoning/usage e eventos semânticos. Capability de áudio permanece desconhecida sem metadata/override por modelo.                                                                               |
| OpenRouter                  | Perfil nomeado sobre transporte compatível, headers de atribuição e extensions por request                                                                                    | Roteamento e capacidades variam com modelo/configuração do gateway.                                                                                                                                                                                          |
| 9router / OmniRouter / Kimi | Transporte genérico configurável quando o endpoint implementa o protocolo suportado                                                                                           | Não há validação suficientemente específica para declarar cada serviço coberto. “OmniRouter” também pode apontar a serviços distintos.                                                                                                                       |
| Llama-server                | Cliente dedicado com health, discovery, chat, Decision Model, embeddings, rerank, TTS e realtime de áudio                                                                     | Discovery é declaração do servidor. Não substitui smoke test do modelo ou de cada modalidade.                                                                                                                                                                |
| Laya standalone             | Adapter Laya para SystemOne, health e Decision Model de texto, bindings e CLI; checkpoint opcional                                                                            | Jev é um modelo específico. Não oferece chat genérico nem discovery OpenAI de llama-server; falta smoke com servidor/checkpoint real.                                                                                                                        |
| Embeddings                  | OpenAI-compatible, OpenRouter, Cohere, Voyage, Ollama e llama-server; llama-server também aceita entradas multimodais via asset map                                           | Dimensão, modalidade e formato seguem próprios de cada protocolo/modelo.                                                                                                                                                                                     |
| Rerank                      | Cohere, Voyage, OpenRouter e llama-server; modalidade textual nos adapters declarados, texto/imagem efêmera no llama-server                                                   | Scores e ordenação não devem ser comparados sem normalização contratual.                                                                                                                                                                                     |
| Áudio                       | TTS bufferizado/streaming e sessão realtime de transcrição/conversa no llama-server                                                                                           | Batch STT OpenAI-compatible tem client multipart Rust e wrappers Python/TypeScript; fixtures Python/Node e gates locais passaram nesta revisão; smoke de serviço/modelo real pendente. Não implica STT de outros providers.                                  |
| Mídia/imagem/vídeo          | Tipos, asset map, envelopes persistível/efêmero, decisões por parte e conversões cobertas por adapters; geração OpenAI Images/OpenAI-compatible/OpenRouter no core e bindings | Geração usa retorno efêmero de bytes por padrão ou gravação opt-in no mapa mutável do host; assinatura, MIME, SHA-256 e limite são verificados. Falta smoke real por provider/modelo, edição, APIs gerais de vídeo, catálogo e limites de duração/resolução. |

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
   A precedência executável é override do consumidor > metadata do modelo/gateway
   > perfil genérico > desconhecido. O teste usa um modelo text-only para impedir
   > que o perfil genérico OpenAI habilite visão; conversão `/v1/models` é opt-in,
   > sem fetch, cache ou catálogo automático. O Decision Model do llama-server
   > aplica modalities descobertas às imagens/áudio efêmeros e mantém override
   > explícito do consumidor como camada superior. OpenAI Responses e Chat
   > Completions mantêm perfis separados: Responses mapeia WAV/MP3, mas sem
   > metadata/override por modelo a capability fica desconhecida.
4. **Laya e llama-server são runtimes/adapters distintos para Decision Model.**
   SystemOne é a API/protocolo; Jev é um modelo específico e Laya não é seu
   alias. Laya oferece checkpoints próprios e roteamento sem seletor; o GGUF servido por llama.cpp usa
   descoberta e endpoints do llama-server. Ter os mesmos pesos não unifica
   capacidades de runtime.

## Evidência de teste

Na verificação local de 17/11/2026, após a implementação do input audio:

- `scons tests` passou: 281 testes da lib Rust, 3 fixtures Rust, 8 testes CLI,
  1 teste nativo Python, 102 Python (10 skips) e 57 Node (18 skips). Fixtures
  Rust validam WAV/MP3 e fallback seguro de MIME não suportado; os testes Python e
  Node reidratam WAV pelo asset map com capability específica do modelo e
  confirmam o payload enviado a Responses. A fixture Laya confirma SystemOne
  sem seletor.
- `scons lint` passou: formatting, Clippy, rustdoc, Ruff, `ty`, Bandit,
  Oxlint e typecheck TypeScript.
- O endpoint real Qdrant e os adapters LanceDB foram usados para a capability
  de filtro vetorial; isso não é evidência de integração com providers/modelos
  de IA reais.
- Não foram configurados credenciais remotas, llama-server com modelo, Ollama
  daemon ou checkpoint Laya para esta rodada. As integrações correspondentes
  permanecem não verificadas em serviço real.
- A serialização WAV/MP3 confirmada por fixture não comprova que cada modelo
  Responses habilitado na conta aceite áudio; falta smoke por modelo.
- Fixtures Rust verificam o payload Responses WAV/MP3 e rejeitam formatos fora
  do contrato; a capability do perfil é verificada separadamente de Chat
  Completions. Isso comprova serialização, não suporte de cada modelo.

Em uma verificação posterior do checkout (17/11/2026), `scons lint` passou;
`scons install`, `scons lint` e `scons tests` passaram. A suíte teve 294 testes
da lib Rust, 3 fixtures Rust, 8 testes CLI, 1 teste nativo Python, 105 testes
Python (10 skips) e 62 testes Node (18 skips). Fixtures HTTP exercitam geração
de imagem nos modos de bytes e asset map nos bindings Python/Node; os testes
também rejeitam mapa somente de leitura antes da chamada ao provider. Três
testes Rust cobrem OpenRouter, store e validação. Os skips são condicionais à
configuração de serviços e não contam como integração real de provider.

Na validação local seguinte, após adicionar testes HTTP para `429`/`Retry-After`
e limite de resposta do batch STT, `CARGO_BUILD_JOBS=2 scons lint` e
`CARGO_BUILD_JOBS=2 scons tests` passaram: 287 testes Rust, 3 fixtures Rust,
8 testes CLI, 1 teste nativo Python, 103 testes Python (10 skips) e 59 testes
Node (18 skips). Os 18 skips Node e 10 skips Python dependem de serviços e
variáveis opcionais; não foram contados como integração real dos providers.
`llm::transcription::tests` passou 6/6. Não havia endpoint ou credencial remota
de STT configurada; smoke real permanece pendente.

Contagens pertencem àquela execução e não são garantia para outro SHA,
ambiente ou configuração. O processo reutilizável está no
[loop](../plans/05-loop-de-desenvolvimento.md).

## Gaps e próximos critérios

- Construir e manter uma tabela verificável provider × endpoint × operação ×
  modalidade × modelo, incluindo auth, erros, limites, stream e status real.
- Orçamento agregado configurável de bytes/assets cobre chat, Decision Model
  efêmero, embeddings e rerank multimodais. Conversão explícita de metadata de
  modelo está nos bindings, mas discovery/cache é do consumidor. Completar catálogo/metadata
  confiáveis e definir limites de duração/resolução; ausência de dado precisa
  continuar como `unknown` e falhar de modo conservador.
- Geração de imagem possui contrato versionado e fixtures offline em Rust,
  Python e Node. Falta smoke real por provider/modelo, edição, streaming e
  comparação de custos/limites. Operações gerais de vídeo ainda exigem contrato
  e adapters dedicados.
- Batch STT usa multipart no perfil OpenAI-compatible, sem retry automático.
  Fixtures Rust/Python/Node exercitam envio e parsing; testes HTTP Rust também
  confirmam `429` com `Retry-After` sem expor o corpo do provider e rejeição de
  resposta acima do limite configurado. Fixtures locais não comprovam integração
  com serviços reais nem suporte de modelos específicos.
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
