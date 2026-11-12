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
- [x] Cobrir concorrência entre processos independentes no ledger SQLite (3 processos, mesma versão esperada: 1 commit e 2 `VersionConflict`).
- [ ] Cobrir backup/restore e recovery operacional do ledger (requer decisão de contrato sobre o mecanismo de backup).
- [x] Rodar os testes service-real existentes e novos de ACK/redelivery, TTL, pub/sub sem assinante e isolamento de namespace contra Redis, NATS JetStream, Qdrant e PostgreSQL locais (Podman): 52 testes de `storage::` passam.
- [x] Cobrir reinício dos serviços: Redis e NATS JetStream (KV e filas retêm dados e o adapter volta a operar) e PostgreSQL parado/religado (append local continua `Pending`; a outbox drena em ordem). Habilitado por `VECTORA_TEST_PODMAN_RESTART`.
- [ ] Cobrir shutdown gracioso do adapter, retenção/limites JetStream e reinício do Qdrant.
- [x] Cobrir ordenação, `limit`, direção do score, upsert idempotente, mesmo id em escopos distintos, delete por escopo e dimensão inválida em LanceDB e Qdrant (contrato comum).
- [ ] Cobrir limites de payload e falhas de indexação dos stores vetoriais.
- [ ] Definir limite/semântica comum sem esconder diferenças específicas.

## Encerramento desta etapa

- [x] Executar `cargo fmt --all -- --check` e `cargo check -p vectora-engine --locked`.
- [x] Executar `ruff check`, `ruff format --check`, `ty check` e `pnpm typecheck`.
- [ ] Executar testes Rust e os testes runtime dos bindings com addons nativos atualizados.
- [ ] Revisar diff, staged list e ausência de arquivos ignorados/artefatos.
- [x] Registrar resultados dos checks concluídos e testes pendentes na auditoria.
- [ ] Commitar e publicar o PR empilhado sobre a branch base autorizada.

O teste Rust de storage foi interrompido após mais de dez minutos devido à
pressão alta de swap/I/O. A suíte Rust iniciada pelo `pre-commit` também foi
interrompida e não foi aprovada. Os testes runtime Python/Node não foram executados
porque os addons nativos locais estão desatualizados. Esses resultados não
devem ser tratados como aprovados; o detalhe está na auditoria vinculada acima.

Os itens pendentes acima pertencem às próximas entregas da auditoria; eles não
devem ser marcados como concluídos apenas porque a seleção inválida foi
removida.
