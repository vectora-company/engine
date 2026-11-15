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

## Entrega

- [x] Atualizar spec, plano, análise, checklists, índices e Q&As afetadas.
- [x] Formatação Rust, Clippy (`-D warnings`), testes Rust de storage (92), testes Python (55), build/typecheck/testes Node (41 aprovados; 10 ignorados), simulação SCons e `git diff --check`.
- [x] Revisar staging; a mudança preexistente em `bindings/typescript/native/index.d.ts` ficou fora do PR.
- [x] Commit `b14dee7`, push e PR [#102](https://github.com/vectora-company/engine/pull/102) aberto sobre a branch anterior #101.
