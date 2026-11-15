# Plano: storage de grafos FalkorDB

## Objetivo

Adicionar uma capability graph à Engine em uma etapa empilhada após o PR #100,
sem alterar o escopo do adapter llama-server. Implementação Rust canônica,
bindings Python e TypeScript, testes contra FalkorDB real e documentação
operacional, com o contrato público aprovado por brunosrz.

## Estado inicial verificado em 2026-11-14

- Checkout `/home/brunosrz/Área de trabalho/engine`. A documentação foi criada
  sobre `feat/llama-server-bindings`; a implementação está em
  `feat/falkordb-graph-storage`, filha da branch `docs/falkordb-graph-contract`
  (PR #101), que por sua vez é filha de #100. A stack aberta confirmada é
  #97 → #98 → #100 → #101.
- `crates/vectora-engine/src/storage.rs` seleciona SQL, vector, KV, queue e
  pubsub; não há seleção, trait ou adapter graph.
- `storage/capabilities.rs` reporta apenas esses cinco tipos.
- `docs/specs/02-storage-memoria-contexto-rag.md` não define property graph.
- Existe alteração preexistente do usuário em
  `bindings/typescript/native/index.d.ts`; deve ser preservada e excluída deste
  trabalho.
- `brunosrz` decidiu usar `falkordb` 0.11.2, a versão stable mais recente
  verificada, e elevar o MSRV global para 1.95. A etapa implementa o client
  oficial com features assíncronas e TLS Rustls, sem modo embedded.

## Etapas

1. **Contrato e pesquisa — concluídos.** Registrar
   fatos, escopo, limites e decisão de implementação em
   [`docs/specs/19-storage-de-grafos-falkordb.md`](../specs/19-storage-de-grafos-falkordb.md)
   e [`docs/analyses/07-auditoria-storage-grafos-falkordb.md`](../analyses/07-auditoria-storage-grafos-falkordb.md).
   `brunosrz` escolheu as duas camadas e aprovou client oficial 0.11.2/MSRV
   1.95.
2. **Core Rust — concluído.** Definir seleção `graph`, configuração segura, trait/adapter
   de acordo com a decisão, erros, conexão, execução parametrizada e limites;
   adicionar testes unitários e de serviço.
3. **Bindings — concluído, com correção de paridade de configuração em andamento.** Expor o mesmo contrato nas bindings Python e TypeScript, sem lógica de domínio duplicada; snapshots/stubs e fixtures compartilhadas. A auditoria posterior encontrou que wrappers encaminhavam apenas endpoint e graph name e descartavam timeout/limites do core. A branch filha do #110 corrige isso usando `GraphStoreSettings` do Rust nos dois bindings.
4. **Conformidade operacional — concluída para o escopo local.** Testar FalkorDB local via Podman/Docker:
   conexão/TLS conforme suportado, query parametrizada, isolamento, falhas,
   timeout, cancelamento, restart e persistência sob a configuração registrada.
5. **Entrega empilhada — PR #102 concluído.** O PR [#102](https://github.com/vectora-company/engine/pull/102) foi aberto como filho do #101; documentação, revisão do staging, commit e push estão concluídos.
6. **Configuração de bindings — branch `feat/falkordb-binding-timeouts`, filha do PR #110.** Expor connection/query/response deadlines e limites de linhas/bytes; usar os valores no client/config do core; testar validação e operações nos bindings contra o servidor real. Preservar a limitação de que cancelar a future do caller não prova cancelamento do comando remoto.

## Critérios de aceite

- Contrato responde às perguntas de ownership, auth/TLS, timeout/cancelamento,
  limites, erro/retry, isolamento e observabilidade.
- Seleção/capabilities Rust/Python/TypeScript serializam de forma consistente.
- Query parameters não são concatenados em texto; erro de escrita não é
  repetido automaticamente após resultado de rede incerto.
- Integração real tem evidência reproduzível com versão de servidor e cliente.
- SQLite permanece autoridade local para estado interno crítico/ledger; não
  há dual-write implícito dos dados de domínio do consumer.
- Sem alegar equivalência com os pares existentes ou suporte completo a
  GraphRAG.

## Fora de escopo

GraphRAG SDK, extração/ingestão por modelo, schemas de produto, RBAC, visualização,
orquestração de containers e features exclusivas do server sem contrato e
conformidade próprios.

## Estado e limitações

Contrato, implementação e testes locais estão concluídos para o escopo descrito. FalkorDB 6.0.3 foi exercitado via Podman. O teste Rust real cobriu auth por URL, parâmetros, Unicode, substituição de propriedades, traversal, limites, timeout e restart/AOF; os bindings Python e Node também exercitam operações contra o serviço. Verificações finais: 92 testes Rust `storage::`, 55 Python, build/typecheck Node e 41 testes Node passaram; 10 testes Node de outros serviços foram ignorados por ausência de endpoints. `scons update --simulate` não alterou manifests/lockfiles e apontou 50 candidatos Rust, Ruff 0.17.5 e ty 0.0.104, sem pacotes pnpm desatualizados. TLS e cancelamento remoto permanecem limitações registradas. A implementação foi publicada no PR #102, filho do #101. Na atualização real de 2026-11-15, `scons update` atualizou Cargo e uv; pnpm já estava atualizado. A simulação pós-update encontrou zero updates aplicáveis restantes; 15 versões mais novas seguem fora da seleção compatível com MSRV 1.95. `scons lint` e `scons tests` passaram; o teste FalkorDB service-real não executou nessa segunda rodada por ausência do endpoint e havia passado antes da atualização. A alteração do usuário em `bindings/typescript/native/index.d.ts` segue fora do PR. O PR aguarda revisão da stack.

Auditoria adicional (15/11/2026): os wrappers Python e Node só liam endpoint e
nome do grafo, descartando as configurações de timeout e limites do core. A
branch `feat/falkordb-binding-timeouts`, filha do PR #110, corrige essa
discrepância usando `GraphStoreSettings` compartilhada. Testes Rust de defaults
e validação passaram; integração Python/Node contra FalkorDB 6.0.3 passou com
timeouts/limites customizados, roundtrip e limite de linhas. O client agora
aplica timeout de conexão (10 s default) e resposta (30 s default), além do
timeout server-side de execução (10 s default). Cancelar a future encerra a
espera local, mas não prova interrupção do comando remoto. TLS e cancelamento remoto continuam sem teste. Hooks completos passaram: 217 testes Rust + 3 fixtures, pytest 95, Node 48/10 skips opcionais, Clippy, rustdoc, Ruff, Bandit, ty, oxlint e typecheck; `scons tests` passou com FalkorDB real configurado. Commit, push e PR filho do #110 são o próximo passo.
