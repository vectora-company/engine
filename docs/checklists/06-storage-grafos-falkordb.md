# Checklist: storage de grafos FalkorDB

Estados: `feito`, `pendente`, `bloqueado`, `não aplicável`.

## Contrato e pesquisa

- [x] Confirmar branch, working tree, remote e PRs abertos; #101 (`docs/falkordb-graph-contract`) é o topo anterior desta etapa.
- [x] Ler `AGENTS.md`, instruções locais, spec 02, planos gerais e estado atual
      dos adapters Rust/bindings.
- [x] Confirmar documentação do protocolo/query, parâmetros, timeout, client
      Rust, MSRV/features e persistência upstream.
- [x] Criar spec, plano e análise com caminho completo nos índices/prompt/goal.
- [x] `brunosrz` escolheu as duas camadas: contrato portátil mais extensão Cypher FalkorDB.
- [x] `brunosrz` aprovou client stable oficial `falkordb` 0.11.2 e elevação do MSRV para Rust 1.95.
- [x] Fixar imagem de teste FalkorDB 6.0.3 e documentar que persistência varia
      conforme configuração; recovery foi testado com AOF síncrono.

## Implementação

- [x] Core Rust e dependência `falkordb` 0.11.2 com features mínimas (Tokio, Serde, Rustls; embedded desativado).
- [x] Seleção `graph: falkordb`, capability report e wrappers de operação JSON Python/TypeScript.
- [x] Seleção, erros, capabilities e validação de nome do grafo.
- [x] Queries tipadas/parametrizadas, timeout e limites; cancelamento remoto
      não é garantido pelo client e permanece limitação documentada.
- [x] Bindings Python/TypeScript e testes de contrato/integração.
- [x] Testes unitários Rust, reporte da capability nos dois bindings; `cargo check --workspace --all-features --locked`, Clippy e 92 testes de storage aprovados.
- [x] Testes funcionais das bindings Python (55) e Node (41); roundtrip, atualização de propriedades e Cypher parametrizado. Dez testes Node de outros serviços foram ignorados por ausência de endpoints.
- [x] Integração Rust/Python/Node contra FalkorDB 6.0.3 via Podman; auth por URL e propriedades via Cypher exercitadas.
- [x] Restart/recovery e persistência testados com AOF síncrono e Podman (`VECTORA_TEST_PODMAN_RESTART`); isso valida somente essa configuração.
- [ ] Exercitar TLS e cancelamento de query no servidor; não alegar cobertura até existir suporte verificado.
- [x] Corrigir paridade de configuração nos bindings: `GraphStoreSettings` compartilhada, connect/query/response timeout e limites de rows/bytes encaminhados ao core; defaults e rejeição de opções inválidas cobertos.
- [x] Verificar os bindings Python e TypeScript com opções customizadas contra FalkorDB 6.0.3; roundtrip e limite de linhas passaram.

## Entrega

- [x] Atualizar spec, plano, análise, checklists, índices e Q&As afetadas.
- [x] Antes da atualização: FalkorDB real via Podman, auth, parâmetros, timeout/limites e restart/AOF; 92 testes Rust de storage passaram.
- [x] Após atualização real: `scons lint` passou; `scons tests` passou com Rust 196 + 3 fixtures, pytest 87/1 skipped e Node 40/11 skipped (sem endpoints de serviços). O teste FalkorDB foi ignorado nesta rodada; integração real aprovada antes da atualização.
- [x] Revisar staging; a mudança preexistente em `bindings/typescript/native/index.d.ts` ficou fora do PR.
- [x] PR [#102](https://github.com/vectora-company/engine/pull/102) aberto como filho do #101; lockfiles atualizados e SCons ajustado para reutilizar a store do `node_modules`.
- [x] Hooks completos passaram: rustfmt, Clippy, rustdoc, 217 testes Rust + 3 fixtures, Ruff, Bandit, ty, oxlint e typecheck. `scons tests` passou: Rust 217 + 3 fixtures, pytest 95 e Node 48/10 skips opcionais; FalkorDB real configurado nos três bindings. Staging revisado sem `bindings/typescript/native/index.d.ts` ou `bindings/python/uv.lock`.
- [x] Commit `039798c`, push e PR [#111](https://github.com/vectora-company/engine/pull/111) aberto como filho do #110 (base `feat/openai-chat-profile`); GitHub reportou `MERGEABLE` na criação.
