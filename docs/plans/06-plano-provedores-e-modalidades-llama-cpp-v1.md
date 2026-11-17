# Plano: providers e modalidades de IA com referência llama.cpp v1.0.0

**Estado verificado em 16/11/2026:** a stack até o PR [#112](https://github.com/vectora-company/engine/pull/112) está integrada em `main` (`4140ddeffa646c4dfd2f0e6a7402a1b00e337099`). O cliente standalone `laya-serve` está no PR [#113](https://github.com/vectora-company/engine/pull/113). Embeddings textuais llama-server estão no PR [#114](https://github.com/vectora-company/engine/pull/114), empilhado sobre #113. Embeddings multimodais estão no PR [#115](https://github.com/vectora-company/engine/pull/115), filho do #114. O rerank textual está no PR [#116](https://github.com/vectora-company/engine/pull/116), filho do #115. Rerank multimodal está no PR [#117](https://github.com/vectora-company/engine/pull/117), filho do #116. Etapa ativa: ampliar testes negativos do rerank multimodal na branch `test/llama-rerank-negative-cases`, filha do #117.
**Base ativa:** PR #113, head `b6da4ed6a420f0093098e05534fba30adf0df50d`; checkout permitido `/home/brunosrz/Área de trabalho/engine`.
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

### Etapa 3b — servidor oficial Laya `laya-serve`

`laya-serve` implementa Jev diretamente: health `/health` e inferência
`/v1/systemone`; não fornece discovery `/v1/models` e a rota HTTP documentada
aceita texto, não mídia. `LayaClient` é um perfil separado de
`LlamaServerClient`: Bearer opcional, envio direto de `model/state/questions`,
timeouts e limites de corpo, sem preflight OpenAI. Ele aceita os campos extras
Laya (`routing`, metadata das respostas e usage estendido), valida `choice` e
`score` com distribuição arredondada a quatro casas e suporta score no formato
de mapas `probabilities`/`legend`. Python e TypeScript expõem health e decisão
de texto. O smoke test real é opt-in por `VECTORA_TEST_LAYA_URL`,
`VECTORA_TEST_LAYA_API_KEY` e `VECTORA_TEST_LAYA_MODEL` (padrão `jev-1`); não
baixa checkpoints no CI padrão. Verificação local: Clippy da lib e testes com
`-D warnings`; `scons tests` passou com Rust 227 + 3 fixtures de conformidade +
1 teste Rust do binding, pytest 95/1 skip e Node 48/11 skips opcionais. O build
release NAPI levou 20m19s no cache frio inicial e passou; a recompilação dentro
do gate SCons levou 0,85s. O servidor/checkpoint real Laya não estava
configurado, então o smoke opt-in continua pendente.

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

**Subetapa 5a — Ollama chat nativo (branch `feat/ollama-chat-profile`, filha
do PR #106):** integrar `/api/chat` ao contrato comum além do transporte
OpenAI-compatible já disponível. Preservar `think`, `format`, `keep_alive`,
`options`, imagens inline base64, ferramentas, reasoning e usage; aceitar
endpoint local sem API key e autenticação opcional remota. Cobrir resposta JSON
e NDJSON com fixtures em Rust/Python/TypeScript, e documentar limites por
operação. A API Responses não é declarada como suportada por esta subetapa.

Estado da implementação (15/11/2026): core Rust e bindings usam a mesma
`NativeChatClient`; a API key opcional só gera Bearer quando configurada. NDJSON
emite deltas, usage e conclusão apenas após `done: true`, e falha EOF prematura.
Fixtures passaram em Rust/Python/Node; workspace passou (210 + 3 fixtures +
teste nativo Python), pytest 57/1 skip, Node 43/11 skips, NAPI/typecheck,
Clippy, Ruff/ty/format. O servidor local não está ativo, sem verificação real.
Publicado como PR [#107](https://github.com/vectora-company/engine/pull/107), filho do #106; GitHub confirma `OPEN`/`MERGEABLE`. O daemon Ollama não está ativo neste host; integração real fica como verificação complementar, sem bloquear a próxima especialização.

**Subetapa 5b — perfil de chat OpenRouter (branch `feat/openrouter-chat-profile`, filha do PR #107):** adicionar configuração nomeada sobre o transporte OpenAI-compatible existente. O perfil deve usar endpoint padrão ou `base_url` customizada, exigir chave, suportar headers de atribuição opcionais e encaminhar campos nativos de roteamento por request. O client genérico continua disponível para 9router, OmniRouter e endpoints compatíveis. Fixtures provam endpoint, auth, attribution e body sem inferir suporte a modalidades por nome. Publicado no PR [#108](https://github.com/vectora-company/engine/pull/108), `OPEN`/`MERGEABLE`; suites completas e hooks passaram.

**Subetapa 5c — OpenAI Responses API (branch `feat/openai-responses-profile`, filha do PR #108):** adicionar protocolo Responses como alternativa ao Chat Completions. Mapear histórico completo, tools e resultados, input de imagem, formatos estruturados por extensions, reasoning/usage e eventos SSE semânticos. Usar `store: false` por padrão e permitir override explícito. Rejeitar estados/outputs que o contrato comum não consiga preservar; não declarar built-in tools, áudio/vídeo ou geração de imagem nesta etapa. Fixtures cobrem request, tools, output, falha/incomplete e término semântico. Fontes oficiais: [Responses overview](https://developers.openai.com/api/reference/responses/overview), [create](https://developers.openai.com/api/reference/resources/responses/methods/create), [stream events](https://developers.openai.com/api/reference/resources/responses/streaming-events), [function calling](https://developers.openai.com/api/docs/guides/function-calling), [image input](https://developers.openai.com/api/docs/guides/images-vision).

**Estado em 15/11/2026:** core Rust implementa requests Responses com input de texto/imagem, histórico completo, function calls/results, opções, extensões, `store: false` e override explícito. Parser interpreta texto/refusal, reasoning summary, tools e usage; SSE mapeia texto/reasoning, argumentos, usage e terminal lifecycle e falha sem evento terminal. Rust workspace: 215 testes passaram; Clippy workspace com `-D warnings` passou. Fixtures HTTP nos bindings Python e TypeScript passaram; build NAPI release, TypeScript, Node (45 pass, 11 skips), pytest completo, Ruff/format/ty, Prettier e hooks passaram. O PR [#109](https://github.com/vectora-company/engine/pull/109), filho do #108, foi publicado. Integração real OpenAI não foi executada por ausência de credencial.

**Subetapa 5d — perfil OpenAI Chat Completions (branch `feat/openai-chat-profile`, filha do PR #109):** adicionar perfil nomeado ao transporte OpenAI-compatible existente, com base URL padrão `https://api.openai.com/v1`, chave obrigatória, override de endpoint e headers extras não reservados. Manter Chat Completions e Responses como protocolos selecionáveis separados; encaminhar extensões por request e validar endpoint/auth em fixtures Rust/Python/TypeScript. Atribuição `app_url`/`app_title` é exclusiva do perfil OpenRouter e não deve aparecer nos requests OpenAI. Não inferir que o perfil cobre outras APIs OpenAI, tools hospedadas ou modalidades que o modelo não anuncia.

Estado local (15/11/2026): perfil implementado em Rust/Python/TypeScript; Rust workspace 216 + 3 fixtures + 1 teste nativo Python, Clippy, Python pytest completo, NAPI/TypeScript e Node 46/11 skips passaram. Ruff/format/ty passaram. Revisão de diff restringiu headers de atribuição OpenRouter ao próprio perfil; fixtures Python/Node agora verificam que não vazam no OpenAI. Rodar hooks completos e atualizar este registro com o PR após publicar. Endpoint OpenAI real não foi chamado; integração requer credencial.

**Saída:** fixtures por endpoint/protocolo; nenhuma modalidade ou API atribuída
ao provider sem prova.

### Etapa 6 — imagem, vídeo e embeddings multimodais

Implementar operações de geração de imagem apenas para endpoints reais e
compatibilidade demonstrada. Completar entrada de áudio/vídeo e embeddings
multimodais do llama-server, preservando requests efêmeros e asset map. Criar
paridade por API e registrar especializações comprovadas.

**Saída:** testes com conteúdo e limites; separação entre visão e geração de
imagem e entre vídeo de entrada/saída.

**Subetapa 6a — embeddings texto llama-server (PR #114, filho do #113):** adicionar
`/v1/embeddings` como operação nomeada nos três runtimes. Implementado e publicado;
preserva a ordem por
`index`, validar cardinalidade, dimensão e finitude, mapear usage e autenticar;
entrada vazia não abre conexão. Fixtures Rust/Node verificam request e resposta.

**Subetapa 6b — embeddings multimodais (branch `feat/llama-server-multimodal-embeddings-assets`, filha do PR #114):** inputs tipados por texto, imagem, áudio e vídeo conforme `llama.cpp` v1.0.0. O core busca assets somente no mapa efêmero do consumidor, confere digest/tamanho/MIME e exige que o modelo anuncie a modalidade; rejeita URLs, paths e bytes inline. Python e TypeScript expõem a chamada efêmera. Quatro testes Rust focados passaram, incluindo serialização das três modalidades, recusa de URL remota e fail-closed sem POST quando o modelo não anuncia imagem. Fixtures Python e Node passaram. O PR [#115](https://github.com/vectora-company/engine/pull/115), filho do #114, foi publicado; a suíte passou com Rust 234 + 3 fixtures + 1 fronteira Python, pytest 97/1 skip e Node 50/11 skips; lint e hooks passaram.

**Subetapa 6c — rerank textual llama-server (branch `feat/llama-server-rerank`, filha do #115):** implementar `/rerank` com formato Jina textual, usando o contrato `RerankRequest`/`RerankResponse`; mapear `top_n`, `model`, índices, score e usage, preservar ranking e rejeitar respostas inválidas. Base URL pode ser raiz ou `/v1`; autenticação é opcional para servidores locais e Bearer quando configurada. Não aceitar mídia nesta subetapa; imagens multimodais ficam para extensão efêmera posterior. Fixtures Rust/Python/TypeScript cobrem rota, auth, request, resposta e validação. Gates integrados passaram: Rust 236; três fixtures de conformidade e uma fronteira Python; pytest 98 passed/1 skipped; Node 51 passed/11 skips opcionais; `scons lint` passou. O pre-commit completo será registrado após a execução final. Publicar como PR filho do #115 e iniciar o follow-up sem esperar CI remoto.

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

Estado atual da subetapa 5b (15/11/2026): adicionado o perfil OpenRouter ao `OpenAiCompatibleConfig` e settings dos bindings. A rota padrão é `/api/v1/chat/completions`; `app_url`/`app_title` produzem `HTTP-Referer`/`X-OpenRouter-Title`; headers explícitos prevalecem. `provider`, `models` e `route` permanecem extensions do request, evitando política de preço/privacidade global. Fixtures Rust, Python e Node passaram: workspace 212 + 3 fixtures + 1 binding Python nativo; pytest 58/1 skip; Node 44/11 skips; typecheck, Clippy, Ruff/format/ty e Prettier passaram. Suites completas e hooks pre-commit passaram. O PR [#108](https://github.com/vectora-company/engine/pull/108), filho do #107, foi publicado e o GitHub confirma `OPEN`/`MERGEABLE`. O endpoint OpenRouter real exige credencial e não foi chamado. Docs oficiais: [Chat Completions/attribution](https://openrouter.ai/docs/api/api-reference/chat/create-a-chat-completion) e [provider routing](https://openrouter.ai/docs/guides/routing/provider-selection).

**Subetapa 6d — rerank multimodal efêmero (branch `feat/llama-server-multimodal-rerank`, filha do PR #116):** suporta texto e imagens em `query` e `documents` pelo formato Jina da API oficial; reidrata somente referências ao asset map do consumidor, exige que `/v1/models` declare imagem e recusa URL, path, bytes inline, documentos e modalidades não suportadas. Core Rust e bindings Python/TypeScript implementados; testes focados Rust (2), Python (1) e Node (1) passaram. `scons tests` passou (Rust 238; 3 fixtures + 1 fronteira Python; pytest 99/1 skip; Node 52/11 skips); `scons lint` passou. Pre-commit e PR #117 pendentes.

**Subetapa 6e — testes negativos do rerank multimodal (branch `test/llama-rerank-negative-cases`, filha do PR #117):** acrescenta casos de capability de imagem ausente, score fora do intervalo e índice duplicado na resposta. Os quatro testes focados de rerank multimodal passaram após a inclusão desses casos. `scons tests` passou (Rust 240; 3 fixtures + 1 fronteira Python; pytest 99/1 skip; Node 52/11 skips); `scons lint` passou. Pre-commit e PR #118 pendentes.
