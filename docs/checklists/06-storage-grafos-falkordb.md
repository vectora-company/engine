# Checklist: storage graph FalkorDB

Contrato em [`docs/specs/19-storage-de-grafos-falkordb.md`](../specs/19-storage-de-grafos-falkordb.md),
plano em [`docs/plans/07-plano-storage-grafos-falkordb.md`](../plans/07-plano-storage-grafos-falkordb.md)
e evidências na
[`docs/analyses/07-auditoria-storage-grafos-falkordb.md`](../analyses/07-auditoria-storage-grafos-falkordb.md).

## Contrato e implementação

- [x] API portátil de nodes, relations e traversal mais extensão Cypher específica.
- [x] Seleção independente de SQL/vector/KV/queue/pubsub e capability report.
- [x] Client oficial `falkordb` 0.11.2; MSRV Rust 1.95; embedded desativado.
- [x] Queries parametrizadas e configuração compartilhada entre Rust e bindings.
- [x] Timeouts de conexão/query/resposta, limites de linhas/bytes e validação.
- [x] TLS Rustls sem sobrescrever provider selecionado pelo host.
- [x] Redaction, testes de parâmetros, Unicode, traversal, properties e erros.

## Integração real

- [x] FalkorDB 6.0.3: auth, operações portáveis, Cypher parametrizada e limites.
- [x] Restart/recovery com configuração de teste AOF síncrono.
- [x] TLS com CA de teste confiável nos bindings exercitados.
- [x] Cancelamento remoto investigado: indisponível no client; future cancelada
      não é prova de que query em execução parou.
- [ ] Repetir restart/recovery com a configuração operacional alvo do consumidor.
- [ ] Adicionar conformidade para cada nova operação/diferença do dialeto.

## Gate por mudança

- [ ] Rust: fmt, Clippy, docs e testes do core.
- [ ] Python: build binding, pytest, Ruff e `ty`.
- [ ] TypeScript: build, typecheck, Oxlint e Node tests.
- [ ] Serviço real de versão fixada quando a alteração tocar protocolo,
      persistência, TLS ou recuperação; skips por falta de URL não contam.
- [ ] Atualizar spec, plano e análise se contrato, limites ou evidências mudarem.
