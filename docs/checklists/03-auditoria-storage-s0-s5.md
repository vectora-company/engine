# Checklist — auditoria de storage S0–S5

Este checklist acompanha o plano [`04-plano-storage-ledger.md`](../plans/04-plano-storage-ledger.md)
e a [auditoria S0–S5](../analyses/05-auditoria-storage-s0-s5.md). “Feito” exige
evidência local registrada; não implica prontidão de produção.

## Contrato público

- [x] Seleções independentes para SQL, vetores, KV, filas e pub/sub.
- [x] Limitar configurações públicas aos adapters reais do produto.
- [x] Rejeitar seleção `memory` no core e nos bindings Python/TypeScript.
- [x] Manter doubles em memória fora da API de configuração do consumidor.
- [x] Publicar relatório estático de capabilities por backend (Rust, Python, TypeScript); probe de readiness de serviço permanece fora de escopo.

## Ledger e adapters

- [x] Inventariar ledger SQLite-first e projeção PostgreSQL.
- [x] Inventariar adapters de KV, queue e pub/sub Redis/NATS.
- [x] Inventariar adapters vetoriais LanceDB/Qdrant e diferenças conhecidas.
- [x] Cobrir concorrência entre conexões (threads), conflito de versão/idempotência, retry idêntico concorrente, restart do arquivo e limites de leitura do ledger SQLite.
- [x] Cobrir concorrência entre processos independentes no ledger SQLite (3 processos, mesma versão esperada: 1 commit e 2 `VersionConflict`; PR #49).
- [x] Implementar e cobrir backup/restore do ledger SQLite nos dois modos decididos (online e offline coordenado pelo host), com verificação, restore e `.replaced`, em Rust, Python e TypeScript.
- [ ] Documentar o procedimento operacional de recovery (ordem: parar escritores, `restore`, reabrir) no guia de operação do consumidor.
- [x] Rodar testes service-real de ACK/redelivery, TTL, pub/sub sem assinante, isolamento KV, projeção PostgreSQL e vetores contra Redis, NATS JetStream, Qdrant e PostgreSQL locais (Podman): 58 testes na execução combinada com reinícios; a execução mais recente com as imagens atualizadas passou 69/69 (sem reinícios).
- [x] Reinício de Redis e NATS JetStream: KV/jobs sobrevivem e os adapters voltam a operar. PostgreSQL parado/religado: o teste isolado passou; a execução combinada registrou uma falha ao parar um container com `AutoRemove=true`, que o Podman removeu. O teste agora confere `AutoRemove=false` antes de parar o serviço. Habilitado por `VECTORA_TEST_PODMAN_RESTART`.
- [x] Reiniciar o Qdrant local e confirmar que coleção/ponto persistem e o adapter recupera a busca no mesmo contêiner.
- [x] Caracterizar a retenção inicial sem limite das filas duráveis em Redis Streams e NATS JetStream.
- [x] Configurar limite finito comum de mensagens retidas (padrão 100000) em Redis e NATS; explicitar que mensagens mais antigas, inclusive pendentes, podem ser removidas quando o limite é atingido.
- [x] Implementar tentativas limitadas (padrão 5) e DLQ consumível nos dois backends; testes service-real passam 14/14. Redis transfere e confirma a origem atomicamente; NATS publica com deduplicação antes do ACK (at-least-once).
- [ ] Verificar em serviços reais a retomada da transferência para a DLQ no NATS e a corrida entre ACK e transferência no Redis. Correção no PR #62.
- [ ] Verificar que cada chamada de recebimento no JetStream reserva uma única mensagem e que um pedido ocioso expira no broker. O PR #63 cobre a fila principal e a DLQ.
- [ ] Verificar com JetStream real a preservação de opções do operador e a recusa de streams com subjects incompatíveis (PR #64).
- [ ] Shutdown gracioso dos consumers; o redrive é feito pelo consumidor da DLQ via `queue_enqueue` e não é transação atômica entre os streams.
- [x] Cobrir ordenação, `limit`, direção do score, upsert idempotente, mesmo id em escopos distintos, delete por escopo e dimensão inválida em LanceDB e Qdrant (contrato comum).
- [x] Cobrir os limites de entrada existentes: busca `limit` 1–1000, vetor 1–65536 dimensões (upsert), escopo/coleção (≤128, charset), chave KV ≤512, identificadores ≤128 e subjects (sem segmentos vazios nem `*`/`#`, ≤512), nos dois lados de cada fronteira.
- [x] Limite de tamanho de `metadata` vetorial, valores de KV e payloads de fila: decidido não impor limite novo (decisão do mantenedor, 2026-11-12).
- [ ] Falhas de indexação dos stores vetoriais.
- [ ] Declarar em `capabilities()` o backup do SQLite, a dependência de `notify-keyspace-events` no watch do Redis e as ausências reais das filas e do vetorial (filtros, índice, quantização).
- [ ] Testes Rust e Python de CAS, incremento, prefixo, watch e remoção de KV (hoje só Node).
- [x] Expor `flush_outbox` e `pending_count` do ledger resiliente em Python e TypeScript (PR #91, filho do #90): cobertura offline confirma flush sem PostgreSQL, preservação da fila e validação 1–1000.
- [x] Enfileirar eventos e checkpoints dos stores SQLite run/loop/policy atomicamente e projetá-los numa transação PostgreSQL (PR aberto #92); verificação de serviço 73/73 com imagens atuais.
- [x] Projeção consultável de reservations/admissions e APIs de leitura em implementação na branch filha #93; testes locais reais passaram.
- [ ] Implementar backup/verify PostgreSQL e concorrência entre projetores.
- [ ] Health/readiness dos backends com os estados da spec 02.

## Encerramento desta etapa

- [x] Executar `cargo fmt --all -- --check` e `cargo check -p vectora-engine --locked`.
- [x] Executar `ruff check`, `ruff format --check`, `ty check` e `pnpm typecheck`.
- [x] Executar testes Rust de storage e runtime Python/Node com addons nativos atualizados (resultados e SHAs na análise; reinício PostgreSQL apenas isolado).
- [x] Revisar diff, staged list e ausência de arquivos ignorados/artefatos nos PRs #46–#49.
- [x] Registrar resultados dos checks concluídos e testes pendentes na auditoria.
- [x] Commitar e publicar os PRs empilhados #46–#49 (integrados em `main` com a stack #45–#72).

Os testes/limites restantes estão descritos na análise vinculada. A antiga nota
de interrupção de testes e addons desatualizados pertencia à etapa inicial e foi
substituída pelos resultados locais registrados em 2026-11-12.

## Equivalência por par (política de 2026-11-12)

Os itens marcados concluídos foram verificados no PR indicado; os demais
são lacunas a fechar nos dois lados do par. Veja a matriz em
[`04-plano-storage-ledger.md`](../plans/04-plano-storage-ledger.md).

- [x] Vetorial: alinhar métrica em `Cosine`, normalizar ambos os scores para similaridade `[-1, 1]` (maior é melhor) e testar ranking com vetores não unitários nos dois backends. `cargo test ... storage::vector::tests` passou 9/9; dois testes integraram com Qdrant real e LanceDB local (PR #57, integrado em `main`).
- [x] Vetorial: verificar os extremos de 1 a 65536 dimensões na busca e gravação em LanceDB local e Qdrant 1.18.0, incluindo coleção ausente e dimensão incompatível. Suíte Node 46/46.
- [x] Verificar listagem KV em Redis e NATS reais: prefixo literal, ordenação, limite, exclusão de chaves expiradas e isolamento do namespace, via addon Node.
- [x] Expor remoção em Python e TypeScript. Verificar leitura e listagem após remoção explícita e repetida em Redis e NATS reais, com Node e wheel CPython 3.13.
- [x] Verificar em NATS real a validação de chaves na leitura, remoção e escrita com ou sem TTL. Suíte Node 38/38.
- [x] Verificar CAS com serviços reais nos dois backends: duas escritas concorrentes com a mesma revisão, invalidação após escrita comum, remoção/recriação, TTL e ausência da chave interna na listagem.
- [x] Verificar incremento em Redis e NATS reais: criação concorrente, delta negativo, limite numérico, valor incompatível, TTL e recriação após remoção.
- [x] Verificar watch KV em Redis e NATS reais: mudanças futuras, filtro de prefixo, isolamento, remoção, expiração e cancelamento da leitura.
- [ ] Vetorial: filtros de metadado, parâmetros de índice ANN, quantização e busca híbrida nos dois backends.
- [x] KV: implementar TTL nativo NATS 2.11+ e validar create/update/expiração contra NATS 2.15.3 e Redis 8.10.3; `storage::` passou 69/69 contra as quatro imagens novas.
- [x] Filas: teto de tentativas e DLQ consumível nos dois brokers (#61–#64).
- [ ] Filas: NACK e reentrega com atraso, deduplicação na publicação, replay e ordenação com contrato, shutdown gracioso e redrive atômico nas duas.
- [ ] Pub/sub: reconexão observável e assinatura por padrão/curinga nos dois.
- [ ] SQL: stores de lifecycle, loop e policy e backup/restore/verify no PostgreSQL como projeção, com a mesma suíte do SQLite; testes concorrentes com serviço real.
- [ ] SQL: interface transacional genérica tipada e limitada nos dois (decisão do mantenedor pendente).
- [x] Decisão do mantenedor: SQLite permanece cópia local obrigatória e autoridade de recovery para ledger e estado crítico interno; PostgreSQL recebe projeções/replicação retomável. Credenciais e autenticação/RBAC do produto ficam no host.
- [ ] Especializações: registrar cada uma em `capabilities()` com o motivo de incompatibilidade verificado na documentação.

## Recovery da projeção PostgreSQL — PR #94, filha do #93

- [x] Repor a outbox inteira sob transação SQLite sem janela de writes sem outbox.
- [x] Reexecutar projeção PostgreSQL idempotente e comparar contagens de agregados/versões por scope.
- [x] Testar serviço real: apagar scope temporária e reconstruir eventos, checkpoint, admission, reservations e reserva standalone.
- [x] Expor operação Rust, Python e TypeScript; bindings rejeitam operação sem URL PostgreSQL.
- [ ] Testar linhas extras, conflito de conteúdo e indisponibilidade remota.
- [ ] Executar suíte NATS com versão 2.11+ (servidor local é 2.10).
- [x] Commit `984e2d066dcc05b0a98e076361ada632751fc61a`, push e abrir PR [#94](https://github.com/vectora-company/engine/pull/94), filho do #93.
