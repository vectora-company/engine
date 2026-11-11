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
- Matriz de capabilities, readiness/health, telemetria e falhas/degradação.

### Produto consumidor

- Tabelas e políticas de contas, autenticação, RBAC, billing, workspaces, settings e entidades de negócio.
- Definição do tenant/principal/namespace confiável e das regras de retenção de produto.
- Origem e permissão de documentos, conversas e dados a indexar.
- Operação/configuração dos serviços remotos, credenciais, backups de deployment e seleção de planos/licenças.

O fato de o Vectora usar hoje determinadas tabelas ou diretórios não os torna parte da API da engine. A migração identifica separadamente dados do runtime e dados do produto.

## Capabilities e adapters

| Interface       | Backends candidatos                                        | Contrato comum                                                                                                | Diferenças que precisam continuar explícitas                                                                                            |
| --------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| SQL durável     | SQLite, PostgreSQL                                         | Transações, consultas tipadas/limitadas, migrações, paginação, health e fechamento de pool.                   | Locks, concorrência, JSON, precisão/datas, isolamento, conflitos, constraints e comportamento de migration.                             |
| Vector store    | LanceDB, Qdrant                                            | Coleções, upsert/delete idempotente, busca, filtros/metadata, top-k e health.                                 | Operadores/filtros suportados, métrica e direção do score, lexical/hybrid search, consistência, limites de payload e criação de índice. |
| KV/cache        | Redis; SQLite ou memória só se explicitamente selecionados | get/set/delete, TTL, namespace e operação atômica quando suportada.                                           | Durabilidade, atomicidade, eviction, TTL e escopo multi-processo. Memória nunca é apresentada como durável.                             |
| Event bus/queue | NATS JetStream, Redis Streams                              | Envelope versionado, idempotency/task ID, publish/consume, timeout, cancelamento, estado de entrega e health. | ACK, consumer/group, redelivery, retenção, replay, ordenação, DLQ, deduplicação e semântica de shutdown.                                |

A interface comum expõe somente o menor denominador seguro. Recursos próprios são capabilities opcionais verificáveis. Um adapter não simula uma capacidade ausente e não substitui automaticamente uma store durável por memória.

## Ledger e estado de execução

Estado experimental revalidado em `main` (`22c861f`): há ledger/checkpoints em
SQLite, stores SQLite especializados de lifecycle, loop e policy, um
`PostgresLedger` usado como projeção, e `ResilientLedger` que grava localmente
com outbox antes de projetar para PostgreSQL. Também existem adapters vetoriais
LanceDB/Qdrant e messaging Redis/NATS para KV, fila e pub/sub. Consulte a
[`matriz de conformidade`](../analyses/02-matriz-conformidade-capabilities.md)
para APIs, bindings, testes e limites por adapter.

Isso não equivale a uma interface SQL genérica: PostgreSQL ainda não é adapter
de lifecycle/loop/policy nem substitui a autoridade SQLite do append resiliente.
A seleção `memory` de KV/queue/pub-sub é serializável, mas os adapters atuais
retornam `Unsupported`. Não há ainda APIs de Memory, Context Planner,
compactação/context engineering, indexação documental ou RAG. Os bindings de
ledger exigem arquivo SQLite persistente. A presença de adapters não prova
paridade operacional: testes de integração service-real, falhas/restart e
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

1. **Contrato Rust:** criar tipos, capabilities, matriz semântica e suite comum; adaptar esta especificação a partir da issue do Vectora.
2. **SQL:** implementar SQLite e PostgreSQL com casos comuns e específicos.
3. **Vetorial:** implementar LanceDB e Qdrant sem forçar equivalência de busca/filtro.
4. **KV/mensageria:** implementar Redis e NATS JetStream para usos descobertos, documentando semânticas distintas.
5. **Distribuição e adoção:** cada capability pública inclui bindings Python e TypeScript no mesmo PR. Preparar publicação e adotar de forma reversível, começando pelo GH Bot TypeScript e seguindo posteriormente com o backend Python do Vectora.
6. **Migração de dados:** por entidade/capability, com métricas de integridade e rollback aprovados antes da troca de padrão.

A ordem pode mudar conforme dependências reais, mas não remove os gates de contrato, conformidade e recuperação.

## Critérios de aceite

- Todo adapter declara capabilities e passa pela suíte comum e por casos próprios.
- Testes reais de serviços cobrem restart/reconnect, timeout, cancelamento, concorrência, idempotência e ciclo de vida; mocks não substituem esses testes.
- SQL cobre transactions, migrations, lock/contention, conflicts, rollback, paginação e diferenças de tipos.
- Vector stores cobrem IDs, upsert/delete, filtros, dimensões, ranking, score, ordenação, coleção ausente e erro de indexação.
- Redis/NATS cobrem TTL/retention, ack/redelivery, duplicate event, replay, disconnect, shutdown e degradação.
- Contratos de memória/Context Planner não ultrapassam tenant/escopo autorizado e preservam origem, confiança e motivo.
- Migração demonstra contagens, hashes, divergências, estado final e rollback sem dual-write.
- Nenhum fallback disfarça menor durabilidade, garantias ausentes ou conteúdo RAG incompleto.
- Benchmarks registram SHA, adapter/versão, hardware, dados e configuração; não generalizam para backends/cargas não medidos.
