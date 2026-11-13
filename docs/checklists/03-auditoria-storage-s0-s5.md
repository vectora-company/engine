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
- [x] Rodar testes service-real de ACK/redelivery, TTL, pub/sub sem assinante, isolamento KV, projeção PostgreSQL e vetores contra Redis, NATS JetStream, Qdrant e PostgreSQL locais (Podman): 58 testes passam na execução combinada mais recente.
- [x] Reinício de Redis e NATS JetStream: KV/jobs sobrevivem e os adapters voltam a operar. PostgreSQL parado/religado: o teste isolado passou; a execução combinada registrou uma falha ao parar um container com `AutoRemove=true`, que o Podman removeu. O teste agora confere `AutoRemove=false` antes de parar o serviço. Habilitado por `VECTORA_TEST_PODMAN_RESTART`.
- [x] Reiniciar o Qdrant local e confirmar que coleção/ponto persistem e o adapter recupera a busca no mesmo contêiner.
- [x] Caracterizar a retenção inicial sem limite das filas duráveis em Redis Streams e NATS JetStream.
- [x] Configurar limite finito comum de mensagens retidas (padrão 100000) em Redis e NATS; explicitar que mensagens mais antigas, inclusive pendentes, podem ser removidas quando o limite é atingido.
- [x] Implementar tentativas limitadas (padrão 5) e DLQ consumível nos dois backends; testes service-real passam 14/14. Redis transfere e confirma a origem atomicamente; NATS publica com deduplicação antes do ACK (at-least-once).
- [ ] Verificar em serviços reais a retomada da transferência para a DLQ no NATS e a corrida entre ACK e transferência no Redis. Correção empilhada sobre o PR #61.
- [ ] Verificar que cada chamada de recebimento no JetStream reserva uma única mensagem e que um pedido ocioso expira no broker. A correção cobre a fila principal e a DLQ.
- [ ] Verificar com JetStream real a preservação de opções do operador e a recusa de streams com subjects incompatíveis.
- [ ] Shutdown gracioso dos consumers; o redrive é feito pelo consumidor da DLQ via `queue_enqueue` e não é transação atômica entre os streams.
- [x] Cobrir ordenação, `limit`, direção do score, upsert idempotente, mesmo id em escopos distintos, delete por escopo e dimensão inválida em LanceDB e Qdrant (contrato comum).
- [x] Cobrir os limites de entrada existentes: busca `limit` 1–1000, vetor 1–65536 dimensões (upsert), escopo/coleção (≤128, charset), chave KV ≤512, identificadores ≤128 e subjects (sem segmentos vazios nem `*`/`#`, ≤512), nos dois lados de cada fronteira.
- [x] Limite de tamanho de `metadata` vetorial, valores de KV e payloads de fila: decidido não impor limite novo (decisão do mantenedor, 2026-11-12).
- [ ] Falhas de indexação dos stores vetoriais.
- [ ] Definir limite/semântica comum sem esconder diferenças específicas.

## Encerramento desta etapa

- [x] Executar `cargo fmt --all -- --check` e `cargo check -p vectora-engine --locked`.
- [x] Executar `ruff check`, `ruff format --check`, `ty check` e `pnpm typecheck`.
- [x] Executar testes Rust de storage e runtime Python/Node com addons nativos atualizados (resultados e SHAs na análise; reinício PostgreSQL apenas isolado).
- [x] Revisar diff, staged list e ausência de arquivos ignorados/artefatos nos PRs #46–#49.
- [x] Registrar resultados dos checks concluídos e testes pendentes na auditoria.
- [x] Commitar e publicar os PRs empilhados #46–#49; #49 está no topo da stack.

Os testes/limites restantes estão descritos na análise vinculada. A antiga nota
de interrupção de testes e addons desatualizados pertencia à etapa inicial e foi
substituída pelos resultados locais registrados em 2026-11-12.

## Equivalência por par (política de 2026-11-12)

Os itens marcados concluídos foram verificados na branch indicada; os demais
são lacunas a fechar nos dois lados do par. Veja a matriz em
[`04-plano-storage-ledger.md`](../plans/04-plano-storage-ledger.md).

- [x] Vetorial: alinhar métrica em `Cosine`, normalizar ambos os scores para similaridade `[-1, 1]` (maior é melhor) e testar ranking com vetores não unitários nos dois backends. `cargo test ... storage::vector::tests` passou 9/9; dois testes integraram com Qdrant real e LanceDB local (branch do PR #57; não integrado em `main`).
- [ ] Vetorial: verificar os extremos do limite compartilhado de 1 a 65536 dimensões na busca e gravação em LanceDB e Qdrant. A validação já está no core.
- [ ] Vetorial: filtros de metadado, parâmetros de índice ANN, quantização e busca híbrida nos dois backends.
- [x] KV: implementar TTL nativo NATS 2.11+ e validar create/update/expiração contra NATS 2.15.3 e Redis 8.10.3; `storage::` passou 69/69 contra as quatro imagens novas. CAS por revisão, incremento, prefixo e watch ainda faltam nos dois.
- [ ] Filas: `max_deliver`/contagem de entregas, DLQ, NACK/atraso e deduplicação equivalentes nas duas.
- [ ] SQL: stores de lifecycle, loop e policy e backup/restore/verify no PostgreSQL, com a mesma suíte do SQLite.
- [x] Decisão do mantenedor: SQLite permanece cópia local obrigatória e autoridade de recovery para ledger e estado crítico interno; PostgreSQL recebe projeções/replicação retomável. Credenciais e autenticação/RBAC do produto ficam no host.
- [ ] Especializações: registrar cada uma em `capabilities()` com o motivo de incompatibilidade verificado na documentação.
