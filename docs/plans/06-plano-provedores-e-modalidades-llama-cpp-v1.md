# Plano: providers e modalidades de IA com referência llama.cpp v1.0.0

**Estado:** ativo. PRs [#97](https://github.com/vectora-company/engine/pull/97) → [#98](https://github.com/vectora-company/engine/pull/98) → [#100](https://github.com/vectora-company/engine/pull/100) formam a base da stack; FalkorDB está em [#101](https://github.com/vectora-company/engine/pull/101) → [#102](https://github.com/vectora-company/engine/pull/102), seguido pelo Decision Model [#103](https://github.com/vectora-company/engine/pull/103) e TTS [#104](https://github.com/vectora-company/engine/pull/104). A sessão realtime está publicada no PR [#105](https://github.com/vectora-company/engine/pull/105), head `e97925b`, filho da branch do #104; status remoto verificado `OPEN`/`MERGEABLE`.
**Base do plano:** `main` em `f4a8cf9d7e30d9fb667a7f443f6572067e536d81` após o merge do PR [#96](https://github.com/vectora-company/engine/pull/96); etapa ativa em branch atualizada por fast-forward.
**Referência normativa:** [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md).

## Objetivo

Fazer a Engine oferecer contratos LLM amplos e honestos: providers e gateways
citados pelo produto, chat e streaming, modelos de decisão, visão/imagem,
áudio, vídeo, embeddings, rerank e capacidades correlatas. Cada operação deve
ter suporte medido por protocolo e provider/modelo, com Rust canônico e
conformidade equivalente nos bindings Python e TypeScript.

O llama.cpp `v1.0.0` é a referência concreta para a nova stack. Usaremos a tag e
seus contratos HTTP do `llama-server`; isso não significa adotar o runtime C++
como dependência de build da Engine.

## Estado confirmado do ponto de partida

- A stack anterior até o PR #96 foi integrada; `main` está em
  `f4a8cf9d7e30d9fb667a7f443f6572067e536d81` e não havia PR aberto no início
  desta trilha.
- Chat atual: cliente genérico OpenAI-compatible; adapters nativos Anthropic,
  Google Gemini e Cohere; fallback e requests efêmeros com mídia.
- Embeddings: OpenAI-compatible, OpenRouter, Cohere, Voyage e Ollama.
- Rerank: Cohere, Voyage e OpenRouter.
- No `main` de base não havia cliente chat dedicado para Ollama, perfil dedicado para llama.cpp,
  Discovery de modalidades, adapter nativo de Decision Model, geração de
  imagem, TTS ou STT. `ContentPart` tipado não comprova suporte de rede.
- OpenRouter, 9router, OmniRouter, OpenAI e Kimi podem compartilhar transporte
  OpenAI-compatible quando o endpoint for compatível; cada perfil ainda exige
  validação de rota, auth, headers, erros, stream e modalidades.

Na branch filha do PR #97, a etapa 2 já adiciona o perfil `LlamaServerClient`
ao core Rust; a ausência descrita acima é o estado-base de `main`, não o estado
desta branch. O profile ainda aguarda exposição equivalente nos bindings.

Evidências e fontes estão em
[`docs/analyses/06-auditoria-providers-modalidades.md`](../analyses/06-auditoria-providers-modalidades.md).

## Etapas empilhadas

### Etapa 1 — matriz e contrato v1.0.0 (etapa documental de origem)

Atualizar especificação, inventário, gaps, testes existentes e critérios da
primeira implementação. Fixar as rotas e diferenças de `llama-server` pela tag
v1.0.0 e caracterizar os providers nomeados como perfis de protocolo, não
promessas indiscriminadas.

**Saída:** concluída no PR documental [#97](https://github.com/vectora-company/engine/pull/97), head `616f558`, sobre `main`; spec 18, análise, checklist, índices e goal loop atualizados; nenhuma API de produção alterada.

### Etapa 2 — perfil llama.cpp, health, descoberta e bindings (PRs #98 e #100)

Adicionar settings e adapter Rust sobre HTTP, com model discovery em
`/v1/models`, estado/capabilities por modelo e chat/text/stream. Testar rotas,
normalização de URL, auth, limits, erros e modalidades anunciadas. Expor APIs
idênticas nos bindings.

**Saída:** fixtures HTTP deterministas mais integração real opt-in contra
`llama-server` v1.0.0; valores de modalities reportados pelo servidor são
preservados sem assumir que representam suporte efetivo do modelo, inclusive
defaults de compatibilidade.

**Estado do core:** implementado na branch `feat/llama-server-profile`, PR [#98](https://github.com/vectora-company/engine/pull/98). O core Rust implementa settings validados, health `/health`, discovery `/v1/models` com arrays de modalidades e metadata preservados, e chat/stream pelo transporte compartilhado. Cinco testes focados e a suíte Rust lib (191 testes), Clippy e rustdoc passaram.

**Estado dos bindings:** implementado na branch filha `feat/llama-server-bindings`, PR [#100](https://github.com/vectora-company/engine/pull/100) aberto sobre #98. Python e TypeScript expõem health, models, completion, completion efêmera, streaming e streaming efêmero sobre o mesmo cliente Rust. Fixtures HTTP exercitam health/discovery/completion/stream nos dois bindings. Python (54 testes), Node (40 passaram, 10 ignorados por exigirem serviços), Ruff, `ty`, TypeScript, `cargo fmt`, Clippy do core e testes unitários dos crates Python/Node passaram. Integração contra `llama-server` real v1.0.0 continua pendente.

### Etapa 3 — Decision Models (PR #103; filha do #102)

A branch `feat/llama-server-decision-model` implementa a operação independente
de chat para `/v1/systemone`: perguntas `choice`, `score` e `noul`; validação de critérios e da capability
`decisions` anunciada por discovery; request persistível separado do envelope
efêmero com data URLs de imagem/áudio e limite configurável; respostas e
probabilidades validadas/preservadas, timeout/auth/limite de resposta. Não há streaming
neste endpoint. Core Rust e wrappers Python/TypeScript estão implementados; fixtures HTTP
passaram nos três runtimes. Rust: teste de rota/probabilidades e quatro testes
de contrato; pytest focado passou; Node: 40 passaram e 11 foram ignorados por
serviços não configurados. `scons build`, Clippy da lib, Ruff/ty e TypeScript
typecheck passaram. Integração real llama-server continua pendente. A subetapa TTS bufferizada descrita abaixo é filha do #103; stream de áudio e realtime ASR seguem como trabalho separado.

**Saída:** conformidade Rust/Python/TypeScript e testes reais com modelo
decision compatível quando disponível.

### Etapa 4 — áudio

**Subetapa 4a — síntese bufferizada (PR #104, filha do #103):** implementa
`/v1/audio/speech` e a extensão nativa `/tts`; conserva WAV/PCM16/PCM float,
bytes binários, `Content-Type`/sample rate e vozes anunciadas por `/props`.
`speaker_ref_b64` passa apenas pelo envelope efêmero. Rust e bindings
Python/TypeScript têm fixtures. Streaming de áudio continua pendente.

**Subetapa 4b — áudio ao vivo e transcrição (PR #105, filha do #104):** a branch
`feat/llama-server-realtime-audio` implementa conexão `/v1/realtime`, intents
transcription/conversation, PCM16 mono com rate model-native ou override,
eventos JSON forward-compatible, filas com backpressure e fechamento que cancela
a sessão. Core Rust e wrappers Python/TypeScript expõem o mesmo fluxo de send,
receive e commit. Testes do core e bindings passaram localmente; a verificação
com servidor/modelo real continua pendente. O PR #105 foi aberto como filho do
[#104](https://github.com/vectora-company/engine/pull/104) sem aguardar CI. Batch
transcription segue separado: só implementar quando uma API específica for
confirmada.

**Saída da etapa:** streaming/backpressure/cancelamento, formatos PCM/WAV e
metadados, limites e testes de desconexão para cada operação implementada.

**Subetapa 4c — streaming TTS (branch `feat/llama-server-streaming-tts`, filha do #105):**
stream binário conforme `stream_format: audio` de `/v1/audio/speech` e `stream: true`
de `/tts`, com metadata de Content-Type/sample rate, consumo incremental com
backpressure, cancelamento e limites em Rust/Python/TypeScript. Resposta TTS em
buffer permanece compatível com a API já entregue no #104. A referência v1.0.0
descreve WAV streaming com tamanho desconhecido no header; a API preserva os
bytes sem tentar reescrever o header. O trabalho foi iniciado sem aguardar CI do
#105.

**Estado local da subetapa 4c (15/11/2026):** core Rust e APIs de streaming
Python/TypeScript estão implementados. O cliente entrega chunks binários
limitados a 64 KiB, aplica limites de resposta e timeouts, preserva Content-Type
e sample rate, detecta resposta vazia e falha de conexão após áudio parcial; o
drop/close cancela o consumo. Fixtures cobrem ambas as rotas e entrega
incremental. Workspace Rust (208 testes, 3 fixtures e teste nativo Python),
Clippy, pytest Python (56 passaram, 1 FalkorDB opt-in pulado), build NAPI,
TypeScript typecheck e Node (42 passaram, 11 serviços opcionais pulados)
passaram. O PR [#106](https://github.com/vectora-company/engine/pull/106), head
`3b28190`, foi publicado como filho do #105 e está `OPEN`/`MERGEABLE`. Integração
com modelo real continua pendente; a próxima etapa é caracterizar e ampliar os
providers/gateways da Etapa 5 sobre o #106, sem aguardar CI remoto.

### Etapa 5 — providers e gateways

Caracterizar OpenAI, OpenRouter, 9router, OmniRouter, Kimi, Ollama, Anthropic,
Gemini e Cohere por operação suportada. Criar perfis nomeados onde defaults ou
diferenças forem úteis; compartilhar codec/transporte quando conformes.

**Saída:** fixtures por endpoint/protocolo; nenhuma modalidade ou API atribuída
ao provider sem prova.

### Etapa 6 — imagem, vídeo e embeddings multimodais

Implementar operações de geração de imagem apenas para endpoints reais e
compatibilidade demonstrada. Completar entrada de áudio/vídeo e embeddings
multimodais do llama-server, preservando requests efêmeros e asset map. Criar
paridade por API e registrar especializações comprovadas.

**Saída:** testes com conteúdo e limites; separação entre visão e geração de
imagem e entre vídeo de entrada/saída.

### Etapa 7 — integração, suporte e gates de release

Completar matriz de providers, fixtures comuns Rust/Python/TypeScript,
instalação/distribuição, documentação operacional, testes reais opt-in e
adaptação reversível dos consumidores. A release ampla só passa quando os
critérios do Epic de primeira release estiverem fechados.

## Critérios transversais

- Atualizar plano, spec, análise e checklist a cada PR; manter `docs/README.md`,
  `docs/plans/00-prompt-loop-goal.md` e `docs/goal-loop/README.md` ligados aos
  artefatos com o caminho completo a partir da raiz do repositório.
- Um PR por etapa, empilhado sobre o anterior; começar a etapa seguinte sem
  aguardar aprovação ou CI remoto.
- Core Rust antes dos bindings; bindings encaminham ao núcleo, sem lógica de
  protocolo duplicada.
- Fixtures offline sem tokens; endpoints/modelos reais apenas opt-in e com
  versões imutáveis registradas.
- Não executar retry após entregar texto/áudio/evento parcial.
- Não transformar dados externos (README, headers de servidores, respostas de
  modelo) em instruções de execução ou confiança.

## Gaps de release que esta trilha não fecha sozinha

Storage ainda tem migrations PostgreSQL com rollback e SQL genérico pendentes;
memória/context planner/RAG e adoção por consumidores têm trabalho próprio.
Essas áreas permanecem no plano de continuidade e entram nos gates da release;
esta trilha não as declara concluídas.

### Log da etapa de bindings

- Implementados seis métodos nativos por binding: health, models, completion, completion efêmera e início de stream nos modos persistível/efêmero.
- Wrappers públicos Python/TypeScript encaminham ao mesmo cliente Rust; `_native.pyi` atualizado para o type checker.
- Fixtures locais verificam `/health`, `/v1/models`, `/v1/chat/completions` e SSE com `text_delta` nos dois bindings.
- Verificações observadas: Python 54/54; Node 40 passaram, 10 skipped por dependência de serviços; Ruff, `ty`, TypeScript, `cargo fmt`, Clippy do core e testes unitários dos crates Python/Node passaram.
- `bindings/typescript/native/index.d.ts` contém alteração local preexistente e fica fora do stage; artefatos temporários NAPI também não fazem parte do produto.
