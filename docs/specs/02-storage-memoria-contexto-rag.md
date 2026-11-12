# Storage, memória, contexto e recuperação na engine

## Objetivo

Definir contratos de persistência e contexto reutilizáveis por produtos diferentes, com adapters para SQLite, PostgreSQL, LanceDB, Qdrant, Redis e NATS JetStream quando cada consumer selecionar essas capacidades. A engine fornece interfaces e semântica explícita; não transforma dados de produto em schema universal.

Uma aplicação pode começar local e embarcada (por exemplo, SQLite + LanceDB), rodar como serviço (por exemplo, PostgreSQL + Qdrant + Redis) ou usar outras combinações. NATS JetStream é uma capability de messaging/stream/job quando contratada, não um requisito implícito de toda instalação.

## Fronteira da engine e do consumer

### Engine

- Contratos e adapters genéricos SQL, vector store, KV e filas/eventos.
- Ledger/checkpoints/eventos genéricos de execução, quando usados pelo runtime.
- Interfaces de memória e recuperação com provenance, escopo e confiança.
- Migrations versionadas dos schemas próprios da engine.
- Persistir primeiro em SQLite todo estado interno necessário à continuidade
  ou recuperação. Projeções PostgreSQL são retomáveis e nunca substituem essa
  cópia local obrigatória.
- Matriz de capabilities, readiness/health, telemetria e falhas/degradação.

### Produto consumidor

- Tabelas e políticas de contas, autenticação, RBAC, billing, workspaces, settings e entidades de negócio.
- Definição do tenant/principal/namespace confiável e das regras de retenção de produto.
- Autenticação de usuários, credenciais e RBAC do produto. A Engine não guarda
  credenciais em claro; persiste apenas estado técnico interno de policy quando
  necessário à recuperação do runtime.
- Origem e permissão de documentos, conversas e dados a indexar.
- Operação/configuração dos serviços remotos, credenciais, backups de deployment e seleção de planos/licenças.

O fato de o Vectora usar hoje determinadas tabelas ou diretórios não os torna parte da API da engine. A migração identifica separadamente dados do runtime e dados do produto.

## Capabilities e adapters

| Interface       | Backends candidatos           | Contrato comum                                                                                                                             | Onde a semântica observável deve ser igual nos dois lados do par                                            |
| --------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| SQL durável     | SQLite, PostgreSQL            | Transações, consultas tipadas/limitadas, migrações, paginação, health e fechamento de pool.                                                | Locks, concorrência, JSON, precisão/datas, isolamento, conflitos, constraints e comportamento de migration. |
| Vector store    | LanceDB, Qdrant               | Coleções, upsert/delete idempotente, busca cosseno, score de similaridade em `[-1, 1]` (maior é melhor), filtros/metadata, top-k e health. | Operadores/filtros suportados, lexical/hybrid search, consistência, limites de payload e criação de índice. |
| KV/cache        | NATS JetStream KV, Redis      | get/set/delete, TTL, namespace e operação atômica quando suportada. TTL por mensagem no NATS requer Server 2.11+ e `AllowMsgTTL`.          | Durabilidade, atomicidade, eviction, TTL e escopo multi-processo.                                           |
| Event bus/queue | NATS JetStream, Redis Streams | Envelope versionado, idempotency/task ID, publish/consume, timeout, cancelamento, estado de entrega e health.                              | ACK, consumer/group, redelivery, retenção, replay, ordenação, DLQ, deduplicação e semântica de shutdown.    |

Um adapter não simula em silêncio uma capacidade ausente e não substitui automaticamente uma store durável por memória.

## Equivalência por par e especialização

Os backends formam cinco **pares**: SQL (SQLite, PostgreSQL), vetorial
(LanceDB, Qdrant), KV/cache (Redis, NATS JetStream KV), fila durável (Redis
Streams, NATS JetStream) e pub/sub (Redis Pub/Sub, NATS Core). O storage é
entregue em duas camadas, nesta ordem.

### Camada 1: base funcional equivalente (obrigatória)

Tudo o que **existe nos dois lados** de um par deve ser implementado nos dois,
com a mesma API, o mesmo comportamento observável (erros, ordenação, escopo,
limites, TTL, ACK, score) e a mesma suíte de conformidade executada contra cada
backend. A base é o **maior denominador comum viável**, não o menor: se os dois
backends conseguem fazer algo, os dois adapters fazem.

- Uma capacidade presente em um lado e viável no outro é uma **lacuna**, não uma
  "diferença a declarar". Lacunas ficam na matriz de equivalência do
  [plano de storage](../plans/04-plano-storage-ledger.md) até serem fechadas.
- A semântica visível ao consumidor é igual. Quando o serviço expõe valores
  diferentes (por exemplo, a métrica de distância ou a direção do score), o
  adapter alinha a configuração ou normaliza o resultado para o contrato comum.
- Implementar um lado primeiro é aceitável como etapa, desde que o outro lado
  entre na mesma stack de PRs e a lacuna esteja registrada. Prioridade, esforço
  ou preferência por um backend não justificam deixar um lado sem a capacidade.
- Diferença que vem só da **configuração do serviço** (por exemplo, durabilidade
  conforme o volume do contêiner) é declarada, não corrigida pela Engine.

### Camada 2: especialização (depois da base)

Só depois de a base ser equivalente cada backend se especializa no que o outro
**não consegue fazer por incompatibilidade técnica**. A especialização:

- exige um motivo de incompatibilidade verificado na documentação e na versão
  do projeto, registrado em `capabilities()`; não existe por decisão de
  produto;
- é uma capability opcional que consumidores descobrem antes de usar; chamá-la
  no backend incompatível retorna erro tipado, nunca um resultado aproximado;
- é emulada no outro lado quando houver emulação com a mesma semântica
  observável; a emulação é declarada como tal;
- deriva só do que a incompatibilidade impede: recursos derivados que ambos os
  lados puderem sustentar continuam na base.

Exemplos de **base a implementar e verificar** (não afirmação de que já estejam
disponíveis nos adapters): busca vetorial com filtros por metadado, ANN
configurável, quantização e busca híbrida; o consumidor pode compor qualquer
adapter conforme para RAG e contexto do agente. LanceDB 0.22.3, versão da
dependência Rust desta branch, contém builders IVF-HNSW-PQ e IVF-HNSW-SQ; isso
não significa que o adapter já configure índices. O contêiner Qdrant em execução
é 1.12.4; `SConstruct` provisiona 1.15.5, e Qdrant usa HNSW como índice denso
padrão. Os tipos de índice e seus
parâmetros precisam de um mapeamento comum testado, não só de nomes parecidos.
No SQL, a base inclui transações, migrações, backup/restore e JSON. No KV,
TTL, escrita condicional por revisão, incremento atômico, listagem por prefixo
e observação de mudanças. Nas filas, ACK/NACK, redelivery com teto, DLQ,
deduplicação e replay. A disponibilidade nativa ou emulada de cada item ainda
deve ser confirmada por versão e registrada na matriz do plano.

TurboQuant (arXiv:2504.19874) é quantização de vetores em geral. Qdrant só
oferece a implementação nativa documentada a partir da versão 1.18.0, enquanto
o contêiner em execução é 1.12.4 (`SConstruct` provisiona 1.15.5). O suporte equivalente no
LanceDB 0.22.3 não está confirmado. Se a Engine implementar o método em camada
comum, a capability pertence à base; se depender de implementação exclusiva
de um backend, deve permanecer fora da base até haver equivalência ou uma
incompatibilidade documentada. PolarQuant (arXiv:2502.02617) trata da
quantização de KV cache de LLM, não do KV store Redis/NATS.

Exemplos de **especialização** (só se a incompatibilidade for confirmada):
compartilhar buckets vetoriais como arquivos em armazenamento de objetos
(LanceDB, por ser embutido e baseado em arquivos), versionamento e _time
travel_ nativo de tabelas (LanceDB), multi-tenancy por payload e sharding
distribuído (Qdrant), Row-Level Security, `LISTEN/NOTIFY` e replicação lógica
(PostgreSQL), request/reply nativo (NATS), estruturas como hashes e sorted sets
(Redis). Cada item acima precisa de confirmação na documentação antes de virar
especialização; se o outro lado puder oferecê-lo, volta para a base.

## Ledger e estado de execução

Estado experimental revalidado em `main` (`22c861f`): há ledger/checkpoints em
SQLite, stores SQLite especializados de lifecycle, loop e policy, um
`PostgresLedger` usado como projeção, e `ResilientLedger` que grava localmente
com outbox antes de projetar para PostgreSQL. Também existem adapters vetoriais
LanceDB/Qdrant e messaging Redis/NATS para KV, fila e pub/sub. Consulte a
[`matriz de conformidade`](../analyses/02-matriz-conformidade-capabilities.md)
para APIs, bindings, testes e limites por adapter.

Isso ainda não é equivalência entre SQLite e PostgreSQL: PostgreSQL não tem
os stores de lifecycle/loop/policy nem backup/restore, e o append resiliente
mantém a autoridade local em SQLite (lacunas na matriz do plano de storage).
As seleções públicas aceitam apenas os seis backends definidos nesta spec;
implementações em memória são doubles internos de teste e não fazem parte da
configuração de produto. Não há ainda APIs de Memory, Context Planner,
compactação/context engineering, indexação documental ou RAG. Os bindings de
ledger exigem arquivo SQLite persistente. A presença de adapters não prova
equivalência operacional: testes de integração service-real, falhas/restart e
retention continuam gaps.

- Separar registro append-only de eventos, projeções consultáveis, checkpoints, cursor e referências a artifacts.
- Envelope deve carregar `event_id`, sequência por agregado/stream, versão do schema, idempotency key, run/turno e escopo confiável do consumidor.
- Retry, redelivery, reconexão e replay não duplicam eventos ou efeitos; lacunas de sequência são representáveis e reconciliáveis.
- Garantir atomicidade entre eventos relacionados somente onde o adapter/contrato realmente a oferecer; declarar fronteiras transacionais entre SQL, vector stores e message bus.
- Payload grande fica fora do ledger com referência, hash, tamanho, classificação, expiração e política de acesso.
- Checkpoints são versionados e restauráveis; checkpoint incompatível falha com erro explícito, não inicia estado novo silenciosamente.
- SQLite permite append de evento e checkpoint na mesma transação por `sqlite_ledger_append_checkpointed`; o snapshot e o evento devem representar o mesmo sequence.
- Antes de entregar uma tool com efeito externo ao adapter, persista um evento/checkpoint `effect_started`; após o resultado, persista o resultado e novo checkpoint. Uma execução interrompida nesse intervalo é incerta e requer reconciliação explícita.

## Memória, contexto e RAG

- Contratos genéricos para itens de memória, documento, chunk, embedding, provenance, origem, versão, escopo, data e confiança.
- Context Planner seleciona deterministicamente por escopo autorizado, relevância, recência e orçamento; retorna razão da seleção, não apenas texto concatenado.
- Preservar unidades relacionadas do histórico (chamada de tool e resultado, aprovação e decisão, falha pendente) ao compactar ou truncar contexto.
- Conteúdo recuperado e documentos externos são dados não confiáveis; não alteram política, capabilities nem identidade.
- Falhas de embedding ou indexação ficam visíveis e não convertem corpus parcialmente indexado em status completo.
- Estratégias BM25/lexical, vetor, híbrida, reranker e query expansion são capabilities opt-in. Avaliar qualidade, custo e latência separadamente, com corpus e configuração identificados.
- Estruturas de knowledge/context graph podem integrar-se por contrato, mas visualização, painéis, navegação e UX pertencem ao produto.

## Jobs e mensageria

- Envelope versionado para job: task ID idempotente, tipo/versão, owner/escopo, prazo, política de retry, cancelamento, estado e vínculo opcional a run.
- Distinguir execução interativa do loop de agente de jobs longos de indexação, crawl, avaliação ou manutenção.
- O ledger ou store durável definido pelo contrato é a autoridade para estado; eventos de fila são transporte/projeção quando assim configurado.
- Não introduzir Celery ou outro broker sem comparação documentada contra adapters e consumidores necessários.
- NATS JetStream e Redis Streams mantêm contratos distintos para ack, redelivery, retention, replay e dead-letter; reconexão/shutdown são parte dos testes.

## Migrations, compatibilidade e recuperação

- Todo schema da engine tem versão, migration idempotente, compatibilidade definida e caminho de downgrade ou rollback documentado.
- Migrações de backend identificam fonte de verdade, congelamento/continuidade de escrita, mapeamento de IDs, contagens, hashes, divergências, repetição segura e rollback.
- Sem dual-write entre implementações e sem shadow mode que cause efeitos duplicados. Comparação de leitura usa snapshots/fixtures isolados.
- Backup, restore e reconciliação são capabilities com resultado observável, não consequência presumida do adapter.
- Isolamento entre tenants/consumidores deve vir do contexto confiável e ser aplicado em cada query/key/collection; não aceitar escopo vindo apenas dos argumentos do modelo.

## Observabilidade e degradação

Cada operação deve permitir registrar adapter/capability escolhida, versão/config sem segredo, duração, resultado, timeout/retry e correlação com run/turno. Health distingue configuração inválida, indisponível, degradado, read-only, não requerido e funcional. Fallbacks indicam exatamente o backend efetivo e as garantias reduzidas.

## Fases de implementação

1. **Contrato Rust:** criar tipos, capabilities, matriz semântica e suite comum; seleções públicas devem corresponder apenas aos seis backends concretos desta spec. Doubles em memória ficam restritos aos testes.
2. **SQL:** implementar SQLite e PostgreSQL com a mesma base funcional; especializações só depois e com motivo de incompatibilidade.
3. **Vetorial:** implementar LanceDB e Qdrant com a mesma base funcional (métrica e score alinhados, filtros, índices, quantização, busca híbrida); especializações só depois e com motivo de incompatibilidade.
4. **KV/mensageria:** implementar Redis e NATS JetStream com a mesma base funcional (TTL, revisão, ACK, redelivery, retenção, DLQ); semânticas distintas só quando a incompatibilidade for confirmada e registrada.
5. **Distribuição e adoção:** cada capability pública inclui bindings Python e TypeScript no mesmo PR. Preparar publicação e adotar de forma reversível, começando pelo GH Bot TypeScript e seguindo posteriormente com o backend Python do Vectora.
6. **Migração de dados:** por entidade/capability, com métricas de integridade e rollback aprovados antes da troca de padrão.

A ordem pode mudar conforme dependências reais, mas não remove os gates de contrato, conformidade e recuperação.

## Critérios de aceite

- Todo adapter declara capabilities e passa pela suíte comum e por casos próprios.
- Toda capability presente em um lado de um par e viável no outro está implementada nos dois e passa pela mesma suíte de conformidade; cada diferença restante tem motivo de incompatibilidade registrado.
- Testes reais de serviços cobrem restart/reconnect, timeout, cancelamento, concorrência, idempotência e ciclo de vida; mocks não substituem esses testes.
- SQL cobre transactions, migrations, lock/contention, conflicts, rollback, paginação e diferenças de tipos.
- Vector stores cobrem IDs, upsert/delete, filtros, dimensões, ranking, score, ordenação, coleção ausente e erro de indexação.
- Redis/NATS cobrem TTL/retention, ack/redelivery, duplicate event, replay, disconnect, shutdown e degradação.
- Contratos de memória/Context Planner não ultrapassam tenant/escopo autorizado e preservam origem, confiança e motivo.
- Migração demonstra contagens, hashes, divergências, estado final e rollback sem dual-write.
- Nenhum fallback disfarça menor durabilidade, garantias ausentes ou conteúdo RAG incompleto.
- Benchmarks registram SHA, adapter/versão, hardware, dados e configuração; não generalizam para backends/cargas não medidos.
