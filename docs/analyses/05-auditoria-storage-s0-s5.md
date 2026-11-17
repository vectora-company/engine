# Auditoria de storage: ledger, adapters e conformidade

**Verificação:** 17 de novembro de 2026. A implementação deve ser conferida no
checkout e SHA em uso; testes service-real descritos aqui são evidência dos
cenários indicados, não um selo de equivalência global. O estado e as lacunas
por par estão resumidos em [`docs/plans/04-plano-storage-ledger.md`](../plans/04-plano-storage-ledger.md),
e os critérios verificáveis estão em
[`docs/checklists/03-auditoria-storage-s0-s5.md`](../checklists/03-auditoria-storage-s0-s5.md).

## Estado por capability

| Capability                            | Estado observado                                              | Evidência e cobertura                                                                                                                                                                                                                                                                                   | Lacunas principais                                                                                                                                                                                                   |
| ------------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ledger e stores SQLite                | Implementados experimentalmente                               | Append idempotente, conflito de versão, concorrência, restart, stores de lifecycle/loop/policy e backup online/offline com verify/restore. Há teste multiprocesso: três writers disputam a mesma versão; um confirma e dois recebem conflito.                                                           | Procedimento operacional de recovery para os consumidores.                                                                                                                                                           |
| Projeção PostgreSQL                   | Parcial; SQLite continua cópia obrigatória e autoridade local | Outbox transacional, flush, projeção de eventos/checkpoints/admissions/reservations, rebuild idempotente e verificação de divergência; testes reais cobrem perda e reconstrução de scope e projetores independentes no mesmo processo.                                                                  | Stores SQL equivalentes, migrations com rollback, concorrência multiprocesso/admissions. Backup é gerenciado pelo operador (`pg_dump` ou equivalente); a Engine verifica e reconstrói a projeção a partir de SQLite. |
| LanceDB e Qdrant                      | Base vetorial parcial e alinhada                              | Upsert/delete por escopo, cosseno normalizado para similaridade `[-1, 1]`, score maior é melhor, dimensões 1–65536 e validação de coleção/dimensão; cenários reais cobrem ordenação, isolamento e restart Qdrant.                                                                                       | Filtro por metadado, ANN configurável, quantização, lexical/hybrid search, falha de indexação observável, backup/snapshot e health.                                                                                  |
| Redis e NATS JetStream KV             | Operações base cobertas nos dois                              | Testes reais Rust e Python cobrem CAS concorrente, incremento atômico, TTL/expiração, prefixo/ordenação/limite, watch de put/remove/expiração, cancelamento e isolamento.                                                                                                                               | Automação da matriz no runner geral. Redis watch requer keyspace notifications `K$gxe` ou `KA`; NATS TTL por chave requer Server 2.11+.                                                                              |
| Redis Streams e NATS JetStream queues | Retry limitado e DLQ implementados; demais contratos parciais | Testes reais cobrem ACK/redelivery, retenção, cinco tentativas, DLQ, retomada após reparar destino NATS, corrida ACK×DLQ Redis, pull único, timeout e preservação de configuração JetStream. Rust `storage::messaging::tests`: 19/19.                                                                   | NACK, retry agendado, deduplicação de publicação, replay por cursor, shutdown gracioso e redrive atômico comum. Transferência NATS é at-least-once e pode duplicar se ACK se perder.                                 |
| Redis Pub/Sub e NATS Core             | Publicação e assinatura efêmeras implementadas                | Mensagem sem assinante é descartada.                                                                                                                                                                                                                                                                    | Reconexão observável e assinatura por padrão/curinga.                                                                                                                                                                |
| Relatório `capabilities()`            | Relatório declarativo estático implementado nos três níveis   | Informa operações, ausências, TTL, direção de score, durabilidade e notas. A revisão atual declara backup/restore SQLite, backup PostgreSQL gerenciado e rebuild/verify, pré-requisitos Redis/NATS e lacunas vetoriais/de filas. Testes direcionados passaram em Rust (6), Python (1) e TypeScript (1). | Sem classificação estruturada de lacuna, emulação e especialização; sem health/readiness ou probe do serviço.                                                                                                        |
| Property graph FalkorDB               | Capability independente implementada                          | Contrato portátil de graph e extensão Cypher, com especificação própria.                                                                                                                                                                                                                                | Não compõe os cinco pares acima; ver análise específica de graph storage.                                                                                                                                            |

## Evidência de conformidade KV

A execução real de KV usou Redis 8.10.3 e NATS JetStream 2.15.3 em containers
Podman temporários. Redis foi iniciado com notificações `K$gxe`. A suíte Rust
passou 15/15; em Python, os dois casos do teste de conformidade passaram, um por
backend. Os casos verificam CAS com writers concorrentes, oito incrementos
concorrentes, TTL e expiração, listagem limitada por prefixo, eventos de watch,
remoção e cancelamento de leitura pendente. Os serviços foram acessados pelo
loopback e os containers temporários foram removidos após a execução.

Essa cobertura demonstra apenas os cenários listados. O teste de binding
TypeScript verifica que o relatório Rust é serializado e exposto corretamente;
não substitui uma execução service-real independente pelo binding.

## Evidência de filas e DLQ

Contra Redis 8.10.3 e NATS JetStream 2.15.3 reais, a suíte Rust cobriu quatro
casos de falha e operação: retry da transferência DLQ após corrigir o stream
NATS incompatível; corrida entre ACK e transferência DLQ no Redis com um único
resultado terminal; uma mensagem por pull e expiração de pull ocioso; preservação
de parâmetros do operador e recusa de subjects incompatíveis sem mutação.

A execução `cargo test -p vectora-engine --lib storage::messaging::tests
--locked` passou 19/19. Clippy com `-D warnings` e `cargo fmt --check` também
passaram. A execução não cobre crash entre publicação da DLQ NATS e ACK, nem
shutdown gracioso do consumer.

## Ledger e recuperação PostgreSQL

SQLite mantém o append e a outbox na mesma transação. Quando a projeção está
habilitada, eventos, checkpoints, admissions e reservations são projetados de
forma retomável; uma indisponibilidade PostgreSQL mantém os dados locais e as
pendências. O rebuild exige pausar writers, repõe a outbox local, reaplica a
scope idempotentemente e compara contagens/versões e os registros projetados.
Não há snapshot atômico entre os dois bancos.

Testes contra PostgreSQL real verificaram rebuild completo, linha extra
(`ProjectionMismatch`), payload divergente (`ProjectionConflict`) e concorrência
de dois projetores independentes no mesmo processo. Concorrência entre
processos e disputa de admissions continuam sem evidência.

## Limites da auditoria

- Testes condicionais sem endpoint não executam integração e devem ser marcados
  como não executados.
- Serviços reais e mocks validam dimensões diferentes; um não substitui o outro.
- A paridade de KV verificada não implica paridade de filas, SQL, vetores ou
  pub/sub.
- A ordem das entregas é definida por dependências e critérios no plano; esta
  análise não registra cronologia de branches, commits ou PRs.
