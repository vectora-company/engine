# Plano de continuidade da Vectora Engine

**Objetivo ativo:** concluir a EPIC da primeira release completa por fases e capabilities, guiado por critérios de aceite e dependências. O estado de implementação deve ser verificado no código e nos testes; o estado de PR deve ser consultado dinamicamente no GitHub e não determina a organização deste plano. O CLI independente tem contratos e critérios em [`docs/specs/20-engine-cli.md`](../specs/20-engine-cli.md), [`docs/plans/08-plano-engine-cli.md`](08-plano-engine-cli.md), [`docs/checklists/08-engine-cli.md`](../checklists/08-engine-cli.md) e [`docs/analyses/08-auditoria-engine-cli.md`](../analyses/08-auditoria-engine-cli.md). Providers e modalidades baseados em llama.cpp v1.0.0 estão em [`docs/plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md`](06-plano-provedores-e-modalidades-llama-cpp-v1.md).

**Objetivo de produto:** a primeira release deve ser ampla e completa, não um
MVP. O escopo e os gates são decompostos neste plano e nos artefatos de cada
capability. Isso não implica substituir o backend Python nem presumir paridade sem
testes.

## Proposta de escopo da primeira release

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

| Capability                                                                                        | Estado                                                                                                                                                                                                                                                                                                                              | Evidência                                                                                                             |
| ------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Chat OpenAI-compatible, OpenAI Responses, Anthropic, Gemini, Cohere, Ollama, OpenRouter, fallback | Implementada (experimental), bindings Python e TypeScript                                                                                                                                                                                                                                                                           | `chat/openai_compatible.rs`, `chat/native_providers.rs`, `chat/fallback.rs`; fixtures e bindings                      |
| Embeddings e rerank com fallback por quota                                                        | Implementada (experimental)                                                                                                                                                                                                                                                                                                         | `llm/embeddings.rs`, `llm/rerank.rs`; testes Python                                                                   |
| Run lifecycle, dispatch, policy/aprovação, loop modelo/tool, gates de verificação                 | Implementada no core e nos bindings                                                                                                                                                                                                                                                                                                 | `harness/*.rs`, `harness/model_tool_loop.rs`, `harness/policy.rs`, `harness/verification.rs`                          |
| Stores SQLite de lifecycle, loop e policy                                                         | Implementada                                                                                                                                                                                                                                                                                                                        | `storage/sqlite_run.rs`, `sqlite_loop.rs`, `sqlite_policy.rs`                                                         |
| Ledger SQLite: idempotência, conflito de versão, multiprocesso, restart                           | Implementada                                                                                                                                                                                                                                                                                                                        | `storage/sqlite.rs` (`multiprocess_child_writer`, `concurrent_appends_with_same_expected_version_commit_exactly_one`) |
| Backup/restore SQLite online e offline coordenado                                                 | Implementada                                                                                                                                                                                                                                                                                                                        | `storage/sqlite/backup.rs` (`backup_online`, `backup_offline`, `verify_backup`, `restore`)                            |
| Projeção PostgreSQL com SQLite como autoridade                                                    | Parcial: projeção de eventos/checkpoints/reservas/admissions, rebuild idempotente e rejeição de divergências estão integrados; concorrência entre dois projetores no mesmo processo também foi verificada. Migrations versionadas com rollback e concorrência multiprocesso seguem pendentes                                        | `storage/resilient_ledger.rs`, `storage/sqlite*.rs`, `storage/postgres.rs`; testes de projeção e restart no ledger    |
| Vetorial: cosseno, dimensões 1–65536, filtro de metadados, coleção ausente e dimensão divergente  | Implementada                                                                                                                                                                                                                                                                                                                        | `storage/vector.rs`; filtros equivalentes antes do top-k em LanceDB/Qdrant, contrato 3                                |
| KV: TTL nativo no NATS (2.11+), CAS por revisão, incremento, prefixo, watch                       | Implementada                                                                                                                                                                                                                                                                                                                        | `storage/messaging.rs` (`allow_message_ttl`, `compare_and_set`, `increment`, `list_keys`, `watch`)                    |
| Filas: retenção finita (100000), 5 tentativas, DLQ consumível                                     | Implementada                                                                                                                                                                                                                                                                                                                        | `storage/messaging.rs` (`max_messages`, `max_attempts`, `receive_dead_letter`)                                        |
| Graph FalkorDB: API portátil e extensão Cypher                                                    | Implementada em Rust/Python/TypeScript; testes reais 6.0.3/TLS/AOF; cancelamento remoto não garantido                                                                                                                                                                                                                               | `storage/graph.rs`, spec 19 e análise 07                                                                              |
| Relatório estático de capabilities por backend                                                    | Implementada                                                                                                                                                                                                                                                                                                                        | `storage/capabilities.rs`                                                                                             |
| Snapshot de contratos para hosts TypeScript                                                       | Implementada                                                                                                                                                                                                                                                                                                                        | `bindings/typescript/src/index.ts` (`engineContractSnapshot`)                                                         |
| Providers/modalidades: chat, Decision Model, embeddings, rerank, áudio, bindings                  | Implementado parcialmente; Decision Model é categoria, Jev é modelo, SystemOne é API e Laya é runtime independente; Responses aceita WAV/MP3 e faz fallback textual seguro; batch STT OpenAI-compatible está no core e bindings com fixtures locais, sem smoke de provider real; geração de imagem e APIs dedicadas de vídeo faltam | spec 18, plano 06, checklist 05                                                                                       |
| Mídia no chat: tipos, preparação, contexto e serialização por protocolo                           | Implementada parcialmente nos três níveis; requests persistíveis/efêmeros, asset map, limites por asset/agregados, retry multimodal e input audio WAV/MP3 no OpenAI Responses; catálogo e limites de duração/resolução pendentes                                                                                                    | `chat/media.rs`, adapters OpenAI/native/fallback; checklist `docs/checklists/04-midia-no-chat.md`                     |
| Harness de tools/identidade/isolamento                                                            | Parcial em contratos: tool loop, schemas, policy snapshots, approvals e stores SQLite existem; filesystem/Git/shell broker, identidade host versionada, sandbox executável, planos de alto nível, memória portável, subagentes e deep search não existem como capabilities comuns                                                   | specs 01/10/12/13/16; auditoria Vectora `docs/analyses/09-runtime-tools-vectora.md`                                   |

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

## Revisão documental transversal

Em 17/11/2026, os planos de inferência e loop agêntico, a capability de
providers/modalidades, mídia, FalkorDB, índices e checklist transversal foram
reconciliados por domínio/capability no checkout. Foram removidos estados
voláteis de PR como estrutura e corrigidas declarações contraditórias sobre
OpenAI Responses e filtros vetoriais. O inventário e os itens restantes estão
em [`docs/checklists/07-checklist-completo-da-engine.md`](../checklists/07-checklist-completo-da-engine.md).
Ainda falta revisar auditorias históricas, matriz de conformidade e documentos
transversais restantes; números/títulos de PR não são índice do plano.

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
   admissions. Verificação contra divergência e concorrência de projetores têm
   cobertura; migrations versionadas com rollback ainda faltam. O contrato de
   desastre usa backup gerenciado (`pg_dump`) e verificação/rebuild da projeção
   pelo SQLite. Evidências e limites service-real ficam em
   [`docs/analyses/05-auditoria-storage-s0-s5.md`](../analyses/05-auditoria-storage-s0-s5.md).

8. Interface SQL genérica tipada e limitada em SQLite e PostgreSQL. **[D]**
   segurança e escopo.
9. Migração de dados (fase 6 da spec 02): contagens, hashes e rollback.

### Fase 3 — eventos, conversas e replay

Origem: [spec 04](../specs/04-conversas-streaming-e-threads.md) e spec 02.

1. Envelope público de evento (event_id, sequência/cursor, versão de schema,
   IDs correlacionados, timestamp) e classes mínimas. **[D]** dados.
2. Gaps representáveis e replay por cursor com reconexão sem duplicação.
3. API de conversa/sessão, histórico paginado e projeções; admissão com
   `queued`, `blocked` e `rejected` e concorrência por conversa. **[D]**.
4. Buffer, coalescência e cancelamento do assinante.
5. Referência de artifact fora do ledger (hash, tamanho, expiração, acesso).
   **[D]** dados e segurança.

### Fase 4 — runtime agêntico

Origem: [spec 01](../specs/01-runtime-agentico-e-harness.md) e
[plano do loop agêntico](02-plano-loop-agentico.md).

1. Tipos `Turn`, `Step`, `Artifact` e evento terminal versionado.
2. Orçamento de tempo, tokens, custo e contexto no loop e no `PolicySnapshot`.
3. Output schema de tool, validação de saída estruturada e dry-run declarado.
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
10. Aceitar descriptor de identidade/Soul configurado pelo host, versionado e
    escopado, sem manter conteúdo, contas ou credenciais de produto na Engine.

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
   Geração de imagem e vídeo dedicado
   permanecem pendentes até haver contrato e evidência de endpoint.

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
