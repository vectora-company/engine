# Plano de continuidade da Vectora Engine

**Objetivo ativo:** concluir a EPIC da primeira release completa por fases e capabilities, guiado por critérios de aceite e dependências. O estado de implementação deve ser verificado no código e nos testes; o estado de PR deve ser consultado dinamicamente no GitHub e não determina a organização deste plano. O CLI independente tem contratos e critérios em [`docs/specs/20-engine-cli.md`](../specs/20-engine-cli.md), [`docs/plans/08-plano-engine-cli.md`](08-plano-engine-cli.md), [`docs/checklists/08-engine-cli.md`](../checklists/08-engine-cli.md) e [`docs/analyses/08-auditoria-engine-cli.md`](../analyses/08-auditoria-engine-cli.md). Providers e modalidades baseados em llama.cpp v1.0.0 estão em [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](06-plano-provedores-e-modalidades-llama-cpp-v1.md).

**Objetivo de produto:** a primeira release deve ser ampla e completa, não um
MVP. O escopo e os gates são decompostos neste plano e nos artefatos de cada
capability. Isso não implica substituir o backend Python nem presumir paridade sem
testes.

## EPIC: visão, princípios e fronteiras

Uma EPIC não é um MVP: ela reúne **todo** o escopo que a Engine deve ter, sem
cortar por prazo; o ROADMAP abaixo decide a ordem. Nada de que um consumidor
(GH Bot, Vectora App, CLI, futuros) dependa do runtime fica fora: se falta,
entra aqui e é implementado na Engine, nunca num runtime substituto em
TypeScript ou Python.

**Visão:** a Engine é o runtime agêntico reutilizável, em Rust, com bindings
Python e TypeScript (e outros no futuro), que qualquer produto usa para
executar agentes com durabilidade, segurança, observabilidade e custo
controlados, sem reescrever loop, providers, storage, eventos, tools, memória
ou avaliação.

**Princípios (valem para todas as fases):**

1. Uma implementação, vários bindings: o núcleo Rust é a fonte; os bindings só
   encaminham e são verificados por fixtures comuns.
2. Storage por equivalência por par; especialização só para incompatibilidade
   verificada, registrada em `capabilities()`.
3. Default-deny, fail-closed em efeitos, redaction por construção, nenhum
   segredo em `Debug`, log ou evento.
4. Se dois métodos coexistem, implementar os dois com padrão recomendado
   (regra 9 do [`AGENTS.md`](../../AGENTS.md)); se são incompatíveis, comparar
   e perguntar no chat.
5. Contratos versionados, migrações idempotentes com rollback, adoção
   reversível e sem dual-write.
6. Nada é declarado pronto, verificado, mergeado ou lançado sem evidência atual
   (comando, SHA, ambiente, contagem); "não executado" não é evidência.
7. Decisão vital **[D]** só no chat; o trabalho independente continua.
8. O consumidor é o oráculo: cada capability é exercitada por um consumidor
   real (GH Bot primeiro, depois Vectora) antes de ser generalizada.

**Fronteira:** UI, workbenches, contas, billing, autenticação de pessoas,
webhooks e escopo GitHub, catálogo de produto e infraestrutura pertencem aos
hosts. A Engine oferece contratos, adapters e mecanismos; o host autentica,
autoriza e opera. A Engine é cliente MCP, nunca servidor MCP.

## Escopo da release 1.0 (trilha H1)

O pedido de produto é uma primeira release completa, não um MVP. A proposta
para o charter 1.0 é cobrir uma jornada de agente utilizável de ponta a ponta:

- providers de chat e multimodalidade com fallback, streaming, erros e limites
  conhecidos;
- runtime com lifecycle, coordenação da execução de tools via adapters,
  policy/aprovação, cancelamento propagado e observável, recovery e evidência
  de verificação;
- conversas/eventos duráveis, cursor e replay;
- storage com equivalência de base em cada par definido, health/readiness,
  migrations e recovery;
- memória, contexto e RAG que integrem embeddings, vetores, busca lexical ou
  híbrida e rerank;
- APIs Rust, Python e TypeScript com fixtures de conformidade, distribuição
  suportada, compatibilidade/versionamento e documentação operacional;
- tools reutilizáveis de filesystem, Git, shell, planejamento e navegação, com
  capabilities concedidas pelo host, sandbox efetivo reportado, execução
  persistida, memória com proveniência e delegação segura a subagentes;
- identidade configurável pelos consumidores: a Engine aceita contexto e
  descriptor versionados, mas não possui Souls, contas, credenciais, catálogo
  de produto ou UX;
- testes de falha, reinício, concorrência, segurança e adoção reversível em
  consumidores reais.

Autenticação, contas, billing, UI, workbenches e UX continuam no host. Isso é
uma proposta de escopo para orientar a revisão; antes de congelar o contrato da
release, o plano deve decompor cada linha em entregas, dependências e critérios
de aceite testáveis. A lista de capacidades planejadas no README ou nas specs
não é promessa de que todas serão incluídas automaticamente.

Este é o plano ativo do [goal loop](../goal-loop/README.md). Cada item do
backlog indica a origem (spec, plano ou análise), o critério de saída e as
dependências. **[D]** marca itens que exigem decisão de `brunosrz`
antes de fixar contrato; a pergunta é feita no chat depois de concluído o
trabalho independente.

## Estado verificado

| Capability                                                                                        | Estado                                                                                                                                                                                                                                                                                                                                                               | Evidência                                                                                                             |
| ------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Chat OpenAI-compatible, OpenAI Responses, Anthropic, Gemini, Cohere, Ollama, OpenRouter, fallback | Implementada (experimental), bindings Python e TypeScript                                                                                                                                                                                                                                                                                                            | `chat/openai_compatible.rs`, `chat/native_providers.rs`, `chat/fallback.rs`; fixtures e bindings                      |
| Embeddings e rerank com fallback por quota                                                        | Implementada (experimental)                                                                                                                                                                                                                                                                                                                                          | `llm/embeddings.rs`, `llm/rerank.rs`; testes Python                                                                   |
| Run lifecycle, dispatch, policy/aprovação, loop modelo/tool, gates de verificação                 | Implementada no core e nos bindings                                                                                                                                                                                                                                                                                                                                  | `harness/*.rs`, `harness/model_tool_loop.rs`, `harness/policy.rs`, `harness/verification.rs`                          |
| Stores SQLite de lifecycle, loop e policy                                                         | Implementada                                                                                                                                                                                                                                                                                                                                                         | `storage/sqlite_run.rs`, `sqlite_loop.rs`, `sqlite_policy.rs`                                                         |
| Ledger SQLite: idempotência, conflito de versão, multiprocesso, restart                           | Implementada                                                                                                                                                                                                                                                                                                                                                         | `storage/sqlite.rs` (`multiprocess_child_writer`, `concurrent_appends_with_same_expected_version_commit_exactly_one`) |
| Backup/restore SQLite online e offline coordenado                                                 | Implementada                                                                                                                                                                                                                                                                                                                                                         | `storage/sqlite/backup.rs` (`backup_online`, `backup_offline`, `verify_backup`, `restore`)                            |
| Projeção PostgreSQL com SQLite como autoridade                                                    | Parcial: projeção, rebuild/verificação, concorrência entre processos, bootstrap versionado e retenção durante outage estão verificadas contra PostgreSQL real. Ensaio do restore operacional ainda pendente; backup PostgreSQL é gerenciado pelo operador (`pg_dump` ou equivalente).                                                                                | `storage/resilient_ledger.rs`, `storage/sqlite*.rs`, `storage/postgres.rs`; testes multiprocesso e migration real     |
| Vetorial: cosseno, dimensões 1–65536, filtro de metadados, coleção ausente e dimensão divergente  | Implementada                                                                                                                                                                                                                                                                                                                                                         | `storage/vector.rs`; filtros equivalentes antes do top-k em LanceDB/Qdrant, contrato 3                                |
| KV: TTL nativo no NATS (2.11+), CAS por revisão, incremento, prefixo, watch                       | Implementada                                                                                                                                                                                                                                                                                                                                                         | `storage/messaging.rs` (`allow_message_ttl`, `compare_and_set`, `increment`, `list_keys`, `watch`)                    |
| Filas: retenção finita (100000), 5 tentativas, DLQ consumível                                     | Implementada                                                                                                                                                                                                                                                                                                                                                         | `storage/messaging.rs` (`max_messages`, `max_attempts`, `receive_dead_letter`)                                        |
| Graph FalkorDB: API portátil e extensão Cypher                                                    | Implementada em Rust/Python/TypeScript; testes reais 6.0.3/TLS/AOF; cancelamento remoto não garantido                                                                                                                                                                                                                                                                | `storage/graph.rs`, spec 19 e análise 07                                                                              |
| Relatório estático de capabilities por backend                                                    | Implementada                                                                                                                                                                                                                                                                                                                                                         | `storage/capabilities.rs`                                                                                             |
| Snapshot de contratos para hosts TypeScript                                                       | Implementada                                                                                                                                                                                                                                                                                                                                                         | `bindings/typescript/src/index.ts` (`engineContractSnapshot`)                                                         |
| Providers/modalidades: chat, Decision Model, embeddings, rerank, áudio, imagem e bindings         | Parcial: geração OpenAI Images/OpenAI-compatible/OpenRouter implementada no core e bindings; retorno efêmero de bytes padrão e gravação opcional no asset map. Fixtures offline passaram; faltam smokes reais, edição e APIs dedicadas de vídeo.                                                                                                                     | spec 18, plano 06, checklist 05                                                                                       |
| Mídia no chat: tipos, preparação, contexto e serialização por protocolo                           | Implementada parcialmente nos três níveis; requests persistíveis/efêmeros, asset map, limites por asset/agregados, retry multimodal e input audio WAV/MP3 no OpenAI Responses; catálogo e limites de duração/resolução pendentes                                                                                                                                     | `chat/media.rs`, adapters OpenAI/native/fallback; checklist `docs/checklists/04-midia-no-chat.md`                     |
| Harness de tools/identidade/isolamento                                                            | Parcial: contexto/grants versionados e validados em Rust/Python/TypeScript; novo dispatch exige contexto vinculado ao actor/scope/policy digest e nega capabilities ausentes. Autenticação do issuer e revalidação no adapter imediatamente antes do efeito seguem no host; filesystem/Git/shell broker, sandbox, planos, memória e deep search permanecem pendentes | spec 21, plano 09, checklist 09, análise 09                                                                           |

Os testes service-real (Redis, NATS, Qdrant, PostgreSQL) são condicionais às
variáveis `VECTORA_TEST_*`; sem elas contam como não executados. Evidências e
contagens por execução estão na
[auditoria de storage](../analyses/05-auditoria-storage-s0-s5.md).

No domínio multimodal, transcrição batch via OpenAI-compatible
`/v1/audio/transcriptions` está implementada no core e bindings: asset map
efêmero, validação de tamanho/digest/MIME, multipart, formatos JSON/texto/
subtítulo, classificação de rate limit sem expor corpo do provider e limite
configurável da resposta. As fixtures Rust/Python/Node e os gates locais
registrados na análise passaram. Falta smoke real com endpoint/modelo
configurado; a operação não faz retry automático. Sua spec está em
[`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md)
e o plano em
[`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](06-plano-provedores-e-modalidades-llama-cpp-v1.md).

## Decisões vigentes

As decisões abaixo foram tomadas por `brunosrz` e estão registradas na fonte
indicada. Não as reabra sem pedido dele.

- Equivalência por par de storage e especialização só por incompatibilidade:
  `AGENTS.md` (regra 5) e [spec 02](../specs/02-storage-memoria-contexto-rag.md).
- SQLite é a cópia local obrigatória e a autoridade de recuperação do ledger e
  dos estados críticos; PostgreSQL é projeção ou replicação retomável:
  [plano de storage](04-plano-storage-ledger.md).
- Backup do ledger SQLite em dois modos (online e offline coordenado pelo host)
  e sem limite novo de tamanho para metadata vetorial, valores de KV e payloads
  de fila (o ledger mantém 1 MiB por evento):
  [auditoria de storage](../analyses/05-auditoria-storage-s0-s5.md).
- NATS Server 2.11 ou superior para TTL por chave; filas com retenção finita
  configurável (100000), cinco tentativas e DLQ; vetores com cosseno em
  `[-1, 1]` e dimensões de 1 a 65536.
- Processo: PRs sempre empilhados; commit, push e PR sem pedido; aprovar PR e
  criar issue só a pedido; decisão vital perguntada no chat; CI remoto não é
  evidência: [loop de desenvolvimento](05-loop-de-desenvolvimento.md).

## Decisões pendentes

Todas se resolvem somente no chat: padrão e limites da admission; artifact fora
do ledger; envelope de job; `capabilities()` estruturado; health, readiness e
biblioteca de tracing; ANN, quantização, híbrida e mapeamento de índices; SQL
genérico; migração de dados; Decision Models no harness; política de versão
(crate `0.0.1` e bindings `0.1.0`); binding Go; edição de imagem e vídeo;
política de licença; i18n e acessibilidade; integração com motor durável de
workflow.

## Revisão documental transversal

Em 17/11/2026, os planos de inferência e loop agêntico, a capability de
providers/modalidades, mídia, FalkorDB, índices e checklist transversal foram
reconciliados por domínio/capability no checkout. Foram removidos estados
voláteis de PR como estrutura e corrigidas declarações contraditórias sobre
OpenAI Responses e filtros vetoriais. O inventário e os itens restantes estão
em [`docs/checklists/07-checklist-completo-da-engine.md`](../checklists/07-checklist-completo-da-engine.md).
Ainda falta revisar auditorias históricas, matriz de conformidade e documentos
transversais restantes; números/títulos de PR não são índice do plano.

## Pilares do escopo total

`GB` = exigido pelo GH Bot, `VA` = Vectora App, `CLI` = engine-cli. O estado de
cada item está no backlog por fases e nos artefatos de cada capability.

| Pilar                              | Escopo total (implementado, parcial e não iniciado)                                                                                                                                                                                                                                                                                                                                                    | Consumidores |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------ |
| P1 Providers e modalidades         | Chat, embeddings, rerank, Decision Models, fala, imagem; smokes reais, paridade de erros e streaming, catálogo e limites por modelo, edição de imagem, vídeo, outros providers de fala e visão, routing e seleção de modelo por tarefa, custo por provider.                                                                                                                                            | GB VA CLI    |
| P2 Runtime agêntico e harness      | Lifecycle, turno, loop, policy, verificação; `Turn`/`Step`/`Artifact`, orçamentos, gate de saída de tool, dry-run, revalidação antes do efeito, coordenador de execução, hooks, Plan Mode e HITL, plano persistido, subagentes e orquestração multiagente medida, recovery com reconciliação, stores PostgreSQL equivalentes, saída estruturada validada, cancelamento, timeout e retry ponta a ponta. | GB VA CLI    |
| P3 Identidade, tools e isolamento  | Identidade e grants; persistência e revogação, registro dinâmico e catálogo por principal, versão de tool, filesystem confinado, Git, shell por argv, `SandboxBroker`, navegação profunda com SSRF por hop, cliente MCP, adapters externos.                                                                                                                                                            | GB VA        |
| P4 Eventos, conversas e replay     | Envelope, log, histórico, espelho do run, leitura tolerante e retomada por `event_id`; retenção e cursor expirado, subscriber, admission, artifact fora do ledger **[D]**, projeção de timeline, channel adapter, redaction de eventos, métricas, `Usage.cost` para `cost_micros`.                                                                                                                     | GB VA        |
| P5 Storage, ledger e mensageria    | Ledger, backup, PostgreSQL, vetores, KV, filas, pub/sub, grafos; restore ensaiado, migrations com down, ANN/quantização, busca lexical e híbrida, shutdown gracioso, DLQ atômica, reconexão pub/sub, fixtures comuns, locking e dedupe distribuídos, envelope de job **[D]**, `capabilities()` estruturado **[D]**, health e readiness **[D]**, SQL genérico **[D]**.                                  | GB VA        |
| P6 Memória, contexto e RAG         | Contratos de memória, documento, chunk e proveniência; `MemoryRecord`, Context Planner determinístico, `ContextProjection` e compactação por unidades, indexação de documentos e código com falha visível, RAG (BM25, híbrido, rerank), memória episódica e semântica com retenção, jobs longos de indexação, oráculo de recuperação.                                                                  | GB VA        |
| P7 Mídia no chat                   | Partes tipadas e assets (implementado); capability por modelo confiável, matriz de serialização por provider, limites de duração e resolução, mídia de saída no Responses.                                                                                                                                                                                                                             | VA CLI       |
| P8 Extensibilidade                 | Hooks, Skills, plugins com manifesto assinado (código de terceiros tratado como hostil), prompt management, registro dinâmico, cliente MCP.                                                                                                                                                                                                                                                            | VA GB        |
| P9 Orquestração e trabalho durável | Background tasks, DAG, fila e scheduler com leases, jobs com envelope, agendamento temporal, integração opcional com motor durável de workflow (sem "Temporal" próprio por padrão), retry e DLQ com reconciliação **[D]**.                                                                                                                                                                             | GB VA        |
| P10 Avaliação, qualidade e replay  | Fixtures de conformidade (conversa e contratos hoje; faltam KV, filas, vetores, mídia), suíte de replay e de dois perfis, cassette de provider, avaliação de agente e de retrieval, benchmarks por SHA com oráculo auditado, casos de revisão de PR (utilidade e falsos positivos).                                                                                                                    | GB VA        |
| P11 Observabilidade e custo        | Correlação conversa/turno/run/step/tool, telemetria sem segredo, métricas (fila e idade, latência, falhas, retries, cancelamento, health), custo e usage por run, biblioteca de tracing **[D]**.                                                                                                                                                                                                       | GB VA        |
| P12 Segurança e privacidade        | Redaction (`Debug` de KV, filas e pub/sub pendente), SSRF por hop e em endpoints configuráveis, aprovação ligada a digest, sandbox efetivo, segredos por referência, export de configuração sem segredos, threat model, SBOM, assinatura e proveniência, política de licença **[D]**.                                                                                                                  | todos        |
| P13 Bindings e distribuição        | Python e TypeScript (Node 22+ e Bun, ESM) em Linux, macOS e Windows; `engineContractSnapshot()` ampliado; npm, PyPI, crates.io; instalação limpa; workflow de release; política de versão **[D]**; binding Go **[D]**; períodos de compatibilidade com os consumidores.                                                                                                                                | GB VA        |
| P14 CLI e experiência de dev       | `engine-cli`, `scons install` isolado, `doctor`, flag para execução sem serviços externos quando aplicável, documentação pública e templates.                                                                                                                                                                                                                                                          | CLI          |
| P15 Operação dos hosts             | Shutdown propagado, stores embutidos que fecham e reabrem, snapshot e restauração consistentes, migração de schema, múltiplas réplicas com storage e locking compartilhados, health e readiness, ensaio de falhas parciais.                                                                                                                                                                            | GB           |
| P16 Governança e longo prazo       | i18n e acessibilidade da CLI **[D]**, semver e depreciação, auditoria documental contínua, matriz de conformidade viva, baseline de performance por SHA, segurança coordenada.                                                                                                                                                                                                                         | todos        |

## Dependências dos consumidores

**GH Bot** consome somente o binding TypeScript local; capability ausente se
implementa na Engine e se liga ao bot na mesma stack, sem runtime substituto.
Detalhes e fontes: [`docs/engine-integration.md`](https://github.com/vectora-company/gh-bot/blob/main/docs/engine-integration.md)
e demais documentos do repositório `gh-bot`.

| Necessidade do GH Bot                                                                | Pilar      | Estado                                                |
| ------------------------------------------------------------------------------------ | ---------- | ----------------------------------------------------- |
| Streaming, cancelamento, erros, timeout, retry e saída estruturada pelo binding real | P1, P2     | parcial                                               |
| Provider e modelo por tarefa, Decision Models, limites                               | P1, P2     | parcial                                               |
| Contexto tipado e limitado de PR (diff, arquivos), chunking, índice do repositório   | P6         | não iniciado                                          |
| Identidade, escopo, policy e capabilities por contexto confiável                     | P3         | implementado (v1); persistência e revogação pendentes |
| Policy aplicada antes de cada efeito; tools separadas das permissões GitHub          | P2, P3     | parcial                                               |
| Ledger, checkpoint, idempotência, replay e storage por capability                    | P4, P5     | parcial (subscriber e retenção pendentes)             |
| Estado terminal recuperável, retry e DLQ                                             | P2, P5, P9 | parcial                                               |
| Visibilidade de job: provider, modelo, versão de policy, custo, usage, etapa         | P4, P11    | não iniciado                                          |
| Orçamento de tempo, tokens e custo por execução                                      | P2         | não iniciado                                          |
| Redaction em logs, traces e erros; export de configuração sem segredos               | P12        | parcial                                               |
| SSRF e validação de endpoints configuráveis                                          | P3, P12    | não iniciado                                          |
| Shutdown gracioso propagado; stores que fecham e reabrem                             | P5, P15    | não iniciado                                          |
| Múltiplas réplicas: storage e locking compartilhados, dedupe distribuída             | P5, P15    | não iniciado                                          |
| Health e readiness com estados de capacidade                                         | P5, P11    | não iniciado **[D]**                                  |
| Métricas de fila, idade, latência, falhas, retries e cancelamento                    | P11        | não iniciado                                          |
| Backup, snapshot consistente, restauração e migração de schema                       | P5         | parcial                                               |
| Snapshot de versões de contrato (o bot lê nove contratos)                            | P13        | parcial; ampliar                                      |
| Binding em Node 22+ e Bun, ESM, três sistemas; npm                                   | P13        | parcial                                               |
| Avaliação de revisão (utilidade e falsos positivos)                                  | P10        | não iniciado                                          |
| Tools somente leitura de Git e arquivos; escrita só com policy                       | P3         | não iniciado                                          |
| Jobs, agendamento e fila de revisão                                                  | P9         | não iniciado                                          |

**Vectora App** absorve por capability, com rollback e sem dual-write, na ordem
da [análise cruzada](../analyses/03-analise-cruzada-akita-engine-vectora-gh-bot.md):
loop de conversa, registro de tools, `ContextProjection` e compactação, memória
e RAG, Context Graph (por contrato), subagentes, scheduler, sandbox, hooks e
plugins, prompt management, routing, evals e replay, MCP. UI, workbenches, tools
de produto, contas, billing e sidecars permanecem no app.

## ROADMAP

As fases 0 a 6 são a trilha da release 1.0; as fases 7 a 12 completam a visão.
O GH Bot exercita cada capability assim que ela existe, em stack própria no
repositório dele; consumo real valida o contrato e não espera release npm.

| Fase                                          | Pilares       | Horizonte      | Depende de | Estado       |
| --------------------------------------------- | ------------- | -------------- | ---------- | ------------ |
| 0 Correções imediatas e dívida de verificação | todos         | H0 agora       | n/a        | parcial      |
| 1 Mídia e providers                           | P1, P7        | H0             | 0          | parcial      |
| 2 Storage: base equivalente                   | P5            | H0             | 0          | parcial      |
| 3 Eventos, conversas e replay                 | P4            | H0 (ativa)     | 0, 2       | parcial      |
| 4 Runtime agêntico, tools e isolamento        | P2, P3        | H1 release 1.0 | 3          | não iniciado |
| 5 Memória, contexto e RAG                     | P6            | H1             | 2, 3       | não iniciado |
| 6 Observabilidade, avaliação e release        | P10, P11, P13 | H1             | 1 a 5      | não iniciado |
| 7 Orquestração e trabalho durável             | P9            | H2             | 3, 4       | não iniciado |
| 8 Extensibilidade                             | P8            | H2             | 4          | não iniciado |
| 9 Escala e operação multi-réplica             | P5, P15       | H2             | 2, 7       | não iniciado |
| 10 Absorção do Vectora App                    | P2 a P9       | H2             | 4 a 8      | não iniciado |
| 11 Ecossistema                                | P1, P13       | H3 longo prazo | 6          | não iniciado |
| 12 Governança e longo prazo                   | P12, P16      | contínuo       | todas      | parcial      |

## Backlog por fases

### Fase 0 — correções imediatas

1. **Mídia nativa e integração do preparo — implementadas no checkout.**
   Payloads nativos, preparo manual, contexto efêmero por chamada e contexto
   padrão estão disponíveis. Retry textual único para rejeição
   multimodal classificada e `ToolResult` tipado também estão disponíveis; OpenAI Responses possui adapter próprio. Orçamento agregado cobre Decision Model, embeddings e rerank multimodais do llama-server; conversão explícita de metadata de `/v1/models` está nos bindings e vence perfil genérico. Discovery/cache automático, catálogo completo e limites de duração/resolução continuam pendentes.
   Evidências e critérios: [`docs/checklists/04-midia-no-chat.md`](../checklists/04-midia-no-chat.md).
2. **Mídia: imagens de resultado de tool — implementado nesta etapa.** `ToolResult` usa partes tipadas; Anthropic, Gemini e OpenAI Chat Completions mapeiam imagens conforme a spec 17. Cohere mantém representação textual porque o protocolo não declara suporte a imagem no resultado. Evidência e testes estão registrados na spec 17 e checklist de mídia.
3. **Segredos em `Debug` — corrigido nesta etapa.** `VectorStoreConfig` agora
   redige chave e endpoint (que pode conter credenciais em userinfo/query) e
   tem teste contra vazamento. Ver checklist de storage e análise de storage.
4. **Cobertura de persistência, policy e KV — parcial.** Os testes do core para
   `SqliteModelToolLoopStore::submit_tool_result`, `recover` e
   `reconcile_tool_result`, além de `PolicyDigestReuse` e `StateTooLarge` no
   store de policy (spec 16), passaram. A suíte KV agora cobre em Rust e Python
   CAS concorrente, incremento atômico, prefixo/ordenação, TTL/expiração, watch
   (put/remove/expiração/cancelamento), remoção e isolamento contra Redis e NATS
   reais. Falta integrar a execução dessa matriz
   ao runner geral e fechar os cenários de fila/DLQ abaixo. Evidências estão em
   [`docs/analyses/05-auditoria-storage-s0-s5.md`](../analyses/05-auditoria-storage-s0-s5.md).
5. **Validação real de fila/DLQ — implementada nesta etapa.** Testes service-real
   cobrem retentativa após DLQ NATS incompatível ser reparada, corrida ACK×DLQ
   Redis com um único resultado terminal, uma entrega por pull, expiração de pull
   ocioso, preservação das opções configuráveis e recusa sem mutação de subjects
   incompatíveis. Evidências em
   [`docs/analyses/05-auditoria-storage-s0-s5.md`](../analyses/05-auditoria-storage-s0-s5.md).
6. **`capabilities()` — requisitos e lacunas declarados nesta etapa.** O
   relatório agora informa backup/restore SQLite, backup PostgreSQL gerenciado
   pelo operador e rebuild/verify via SQLite, pré-requisito de notificações no
   watch Redis e lacunas reais de vetores e filas. Testes de serialização passam
   em Rust, Python e TypeScript. A API ainda não diferencia por tipo estruturado
   lacuna, emulação e especialização; esse refinamento permanece na Fase 2.

### Fase 1 — mídia e conteúdo multimodal

Origem: [`docs/specs/17-midia-no-chat.md`](../specs/17-midia-no-chat.md),
comportamento observado nos consumidores e checklist
[`docs/checklists/04-midia-no-chat.md`](../checklists/04-midia-no-chat.md).
A Engine oferece tipos de conteúdo multimodal, preparo explícito por asset map,
contexto efêmero por chamada e por cliente, limites, retry seguro e resultados
multimodais de tools. O host mantém bytes fora de requests persistíveis; MIME e
modalidade são verificados na reidratação. A cobertura varia por protocolo e
precisa ser marcada por feature no checklist específico.

Limites agregados opcionais de bytes e contagem de assets estão implementados
no preparo de chat, requests efêmeros inline, Decision Model, embeddings e
rerank multimodais de llama-server, com testes Rust/Python/TypeScript. Critérios
ainda pendentes: catálogo atualizado de suporte e limites por
provider/modelo/protocolo; cobertura equivalente de todos os protocolos que
aceitam a mesma forma de mídia; limites de duração e resolução; mídia de saída
no adapter Responses; testes de consumo e replay de assets em fluxos reais.
OpenAI Responses serializa áudio WAV/MP3, mas a capability permanece
desconhecida sem metadata/override por modelo; testes Rust, Python e Node
confirmam materialização do asset autorizado e wire payload. A matriz de implementação
e evidências está no checklist da
capability.

### Fase 2 — storage: base equivalente

Origem: [plano de storage](04-plano-storage-ledger.md) e
[spec 02](../specs/02-storage-memoria-contexto-rag.md).

1. Vetorial nos dois backends: **filtros escalares equivalentes implementados**
   antes do top-k, com paths aninhados e composição lógica; contrato vetorial 3
   usa metadata JSON consultável no LanceDB e exige recriação das coleções com
   schema legado. Seguem pendentes índice ANN parametrizado, quantização,
   busca lexical/híbrida, falhas de indexação visíveis e backup/snapshot.
   **[D]** mapeamento comum de índices.
2. Filas nos dois backends: **NACK/reentrega atrasada, enqueue idempotente por
   janela e replay read-only paginado por cursor implementados e testados nos
   dois serviços e bindings**. O replay mantém posição vinculada a versão,
   backend e namespace e não toca offsets de consumers. Testes reais cobrem
   cursor expirado após trim e 32 produtores concorrentes com replay ordenado,
   sem perdas/duplicatas. Pendem shutdown gracioso e redrive atômico da DLQ;
   envelope de job versionado. **[D]** envelope de job.
3. Pub/sub: **assinatura por padrão implementada nos dois brokers**, com
   `*` para um segmento e `>` final para múltiplos; Redis filtra localmente
   para corresponder ao contrato por segmento do NATS. Pende reconexão
   observável. Publicação NATS aguarda confirmação do envio, inclusive nos
   bindings de chamada curta; Rust/Python/Node foram exercitados contra Redis
   8.10.3 e NATS 2.15.3.
4. `capabilities()` estruturado (lacuna, emulação, especialização com motivo).
   **[D]** formato.
5. Health/readiness com os seis estados da spec 02 e observabilidade sem
   segredos (adapter, duração, retry, correlação). **[D]** biblioteca de
   tracing/métricas.
6. **`flush_outbox` e `pending_count` nos bindings — implementados.** Python expõe inteiro; TypeScript expõe `bigint`; limite 1–1000 validado mesmo sem PostgreSQL. Sem URL, flush válido retorna 0 e preserva a outbox SQLite. Fixtures comuns de KV, filas e vetores nos três níveis continuam pendentes.
7. **Projeção PostgreSQL — parcial.** SQLite permanece autoridade local; a
   outbox e o rebuild idempotente projetam eventos, checkpoints, reservations e
   admissions. Verificação contra divergência, outage isolado, concorrência de
   projetores em processos independentes e bootstrap da migration versionada têm
   cobertura real. O rollback restaura backup gerenciado (`pg_dump`) e faz
   verify/rebuild da projeção
   pelo SQLite. Evidências e limites service-real ficam em
   [`docs/analyses/05-auditoria-storage-s0-s5.md`](../analyses/05-auditoria-storage-s0-s5.md).

8. Interface SQL genérica tipada e limitada em SQLite e PostgreSQL. **[D]**
   segurança e escopo.
9. Migração de dados (fase 6 da spec 02): contagens, hashes e rollback.

### Fase 3 — eventos, conversas e replay

Origem: [spec 04](../specs/04-conversas-streaming-e-threads.md) e spec 02.
O plano, checklist e análise específicos estão em
[`docs/plans/conversation-events.md`](conversation-events.md),
[`docs/checklists/conversation-events.md`](../checklists/conversation-events.md)
e [`docs/analyses/conversation-events.md`](../analyses/conversation-events.md).

1. **Implementado e verificado localmente:** envelope Rust v1
   (`ConversationEvent`) com event/conversation/turn/run/step IDs, sequência,
   timestamp, origem e payloads tipados; validação estrutural; bindings Python
   e TypeScript encaminham ao mesmo validador Rust; fixture comum nos três
   níveis. Evidência: `crates/vectora-engine/src/harness/events.rs`,
   `tests/fixtures/conversation-events-v1.json` e a análise da capability.
2. **Parcial:** log durável no ledger SQLite com sequência atribuída pela store,
   idempotência por `event_id`, histórico paginado por cursor e espelho
   idempotente do lifecycle do run, com um único evento terminal por run (Rust,
   Python e TypeScript, verificados localmente). Leitura tolerante com `gaps` e
   retomada por `event_id` implementadas e verificadas. Faltam política de
   retenção com cursor expirado e fixture comum do store nos três níveis.
   Evidência: `crates/vectora-engine/src/storage/sqlite/conversation.rs`.
3. **Pendente:** admissão com `queued`, `blocked` e `rejected` e concorrência por
   conversa. Os dois modos (fila FIFO limitada e admissão imediata) coexistem;
   o padrão recomendado e os limites aguardam confirmação no chat. Comparativo
   em [`docs/analyses/conversation-events.md`](../analyses/conversation-events.md).
4. **Não iniciado:** buffer, coalescência e cancelamento do assinante.
5. Referência de artifact fora do ledger (hash, tamanho, expiração, acesso).
   **[D]** dados e segurança.

### Fase 4 — runtime agêntico

Origem: [spec 01](../specs/01-runtime-agentico-e-harness.md) e
[plano do loop agêntico](02-plano-loop-agentico.md).

1. Tipos `Turn`, `Step`, `Artifact` e evento terminal versionado.
2. Orçamento de tempo, tokens, custo e contexto no loop e no `PolicySnapshot`.
3. [parcial] `ToolContract.output_schema` opcional valida `ToolResult.parts`
   via `validate_tool_result_output`; o host ainda precisa invocar o gate antes
   de persistir/expor a saída. Dry-run declarado segue pendente.
4. Ligação automática lifecycle, loop e policy (revalidação antes de cada
   efeito).
5. Hooks pre-tool, post-tool e pre-final; Plan Mode; subagentes com escopo e
   orçamento derivados; contrato de isolamento/sandbox.
6. Decision Models. **[D]**.
7. Redaction e fixtures de dois perfis de consumidor.
8. Implementar tools portáveis filesystem/Git/shell, plano persistente,
   “remember” com proveniência e execução de subagentes com parent ID; usar
   grants derivados da policy e APIs do host para efeitos.
9. Implementar pesquisa/navegação profunda bounded, com adapter de busca/fetch,
   proveniência e validação SSRF em cada URL/redirect/link.
10. [parcial] Aceitar descriptor de identidade/Soul configurado pelo host,
    versionado e escopado, sem manter conteúdo, contas ou credenciais de produto
    na Engine. `HostIdentityContext` e grants têm validação temporal e estrutural
    no core e bindings; autenticação do issuer/revogação e aplicação antes de cada
    efeito ainda pertencem às etapas seguintes. O dispatch com identidade agora
    exige grant correspondente e nega por padrão capabilities ausentes.

O contrato, o plano ordenado, checklist e análise da expansão estão em
[`docs/specs/21-harness-tools-identidade-e-isolamento.md`](../specs/21-harness-tools-identidade-e-isolamento.md),
[`docs/plans/09-plano-runtime-tools-identidade-e-isolamento.md`](09-plano-runtime-tools-identidade-e-isolamento.md),
[`docs/checklists/09-runtime-tools-identidade-e-isolamento.md`](../checklists/09-runtime-tools-identidade-e-isolamento.md)
e [`docs/analyses/09-runtime-tools-vectora.md`](../analyses/09-runtime-tools-vectora.md).

### Fase 5 — memória, contexto e RAG

Origem: spec 02 (fase S5 do plano de storage). Contratos de memória,
documento, chunk e proveniência; Context Planner determinístico; compactação
que preserva unidades; indexação com falha visível e RAG opt-in (lexical,
híbrido, rerank). Memória episódica/semântica precisa de proveniência, escopo,
sensibilidade, validade, revisão e deleção, conforme spec 21. Depende das fases
2 e 3.

### Storage de grafos FalkorDB

Adicionar graph storage à Engine, mantendo fora do escopo a ingestão/extração de
GraphRAG e sem alterar os pares SQL, vector, KV, queue ou pubsub. O estado
verificado e as lacunas constam em [`docs/specs/19-storage-de-grafos-falkordb.md`](../specs/19-storage-de-grafos-falkordb.md), [`docs/plans/07-plano-storage-grafos-falkordb.md`](07-plano-storage-grafos-falkordb.md), [`docs/analyses/07-auditoria-storage-grafos-falkordb.md`](../analyses/07-auditoria-storage-grafos-falkordb.md) e [`docs/checklists/06-storage-grafos-falkordb.md`](../checklists/06-storage-grafos-falkordb.md).

`brunosrz` escolheu duas camadas: contrato property graph portátil e extensão
Cypher específica de FalkorDB; também aprovou o client `falkordb` 0.11.2 e Rust
1.95. A implementação expõe configuração de timeouts e limites no core e nos
bindings. Testes com FalkorDB 6.0.3 cobrem roundtrip, limites e TLS real em
Rust/Python/Node. Defaults: conexão 10 s, query server-side 10 s, resposta
client-side 30 s, 1.000 linhas e 4 MiB. O client oficial não comprova
cancelamento remoto nem rollback após timeout. Estado, critérios e evidências
por capacidade estão em [`docs/specs/19-storage-de-grafos-falkordb.md`](../specs/19-storage-de-grafos-falkordb.md), [`docs/plans/07-plano-storage-grafos-falkordb.md`](07-plano-storage-grafos-falkordb.md), [`docs/analyses/07-auditoria-storage-grafos-falkordb.md`](../analyses/07-auditoria-storage-grafos-falkordb.md) e [`docs/checklists/06-storage-grafos-falkordb.md`](../checklists/06-storage-grafos-falkordb.md).

### Fase 6 — adoção, distribuição e medição

1. Integração do host de policy (recompilar, autenticar recibo, revalidar antes
   do efeito) e de cancelamento (propagar, impor timeout no ponto de execução,
   testes de falha parcial e resultado `unknown`).
2. GH Bot consumindo `engineContractSnapshot()`; ampliar o snapshot para
   storage e mídia e expô-lo também em Python. O Bot usa hoje só `chatStream`
   e monta as versões em `app/src/server/engine.ts`
   (`/media/brunosrz/externo/vectora-repos/gh-bot`); a troca é um PR no
   repositório do Bot. Integração com `ModelToolLoop`, `ToolPolicy` e
   `VerificationGate` vem depois.
3. Piloto reversível no Vectora App, uma capability por vez, sem chamadas
   paralelas a providers reais.
4. Publicação em crates.io, PyPI e npm com matriz de plataformas e workflow de
   release separado do CI; alinhar versões (crate `0.0.1`, bindings `0.1.0`).
   **[D]** política de versão.
5. Medições reproduzíveis por SHA (latência, memória, throughput, custo,
   recuperação).
6. Batch STT OpenAI-compatible `/v1/audio/transcriptions`: core e bindings
   implementados; fixtures Rust/Python/Node, limites de resposta e classificação
   de erro cobertos. Resta smoke real e sincronização contínua dos registros.
   Geração básica de imagem OpenAI/OpenRouter está implementada em core e
   bindings, com bytes efêmeros por padrão e opção de gravar no asset map; as
   fixtures offline passaram. Falta smoke remoto por provider/modelo; edição e
   vídeo dedicado permanecem pendentes.

### Fase 7 — orquestração e trabalho durável

Pilar P9. Background tasks, DAG, fila e scheduler com leases e workers; jobs
com envelope versionado; agendamento temporal; avaliar integração com motor
durável de workflow (a Engine não implementa um "Temporal" próprio por
padrão); retry e DLQ com reconciliação. Origem: `analyses/03` e `analyses/04`.
**[D]** envelope de job e política de integração.

### Fase 8 — extensibilidade

Pilar P8. Hooks e plugins com manifesto assinado e verificado, Skills, prompt
management (identidade, versão, composição, trust boundary, rollout e
avaliação), cliente MCP, adapters externos uniformes e trust boundary para
código de terceiros.

### Fase 9 — escala e operação multi-réplica

Pilares P5 e P15. Storage e locking compartilhados, dedupe distribuída,
shutdown e upgrade sem perda, ensaio de falhas parciais, estratégia de entrega
e contratos para Helm e HA, sem prometer alta disponibilidade antes de haver
métricas e evidência.

### Fase 10 — absorção do Vectora App

Migração por capability, com rollback e sem dual-write, na ordem da análise
cruzada. Cada capability só migra depois de contrato, fixtures e oráculo
independentes.

### Fase 11 — ecossistema

Binding Go (issue #20, decisão ABI C/cgo ou IPC) **[D]**; outros providers,
modalidades e backends de grafo só depois da base comum; novos bindings por
demanda.

### Fase 12 — governança e longo prazo

Segurança contínua, política de licença e SBOM, i18n e acessibilidade **[D]**,
semver e depreciação, auditoria documental viva, baselines de performance e
matriz de conformidade viva.

### Trilha transversal

- Grafo FalkorDB: get/delete de relação, readiness, versão do contrato nos
  bindings.
- Inconsistências já identificadas: `ConversationEvent` ainda exige `sequence`
  do produtor; conversão `Usage.cost` para `cost_micros` indefinida.
- A flag para iniciar o Vectora sem o gateway de Workers da Cloudflare pertence
  ao repositório do app `vectora` (`backend/api/server.py`,
  `settings.gateway_enabled`); a Engine não tem ligação com o Services.

## Gates de adoção e release

Uma capability só avança de experimental quando:

- contratos e erros são versionados, documentados e testados nas três entradas;
- timeout, cancelamento, redaction e falhas/degradação têm resultado observável;
- concorrência, restart e idempotência são testados quando houver efeitos;
- fallback e rollback do consumidor são testados sem duplicar efeitos;
- métricas são reproduzíveis e limitadas ao workload medido;
- consumidores e limites de compatibilidade estão identificados.

Nenhuma capability está declarada estável. Código em revisão ou testes
isolados não satisfazem, por si só, o gate de release; consulte a matriz de
conformidade para o estado e a cobertura de cada capability.

## Seleção da próxima capability

Escolha no backlog por fases a capability incompleta com dependências satisfeitas.
Registre seu estado e evidências nos artefatos específicos; consulte o GitHub
separadamente para identificar a base atual da stack. PRs não definem a ordem
conceitual das fases.

## Regras de priorização

Escolha a capability incompleta cujas dependências estejam satisfeitas e que
reduza um gap de release. O avanço de uma capability não espera aprovação nem
CI de outra etapa; a base GitHub é descoberta no momento de entregar cada
branch.
