# Checklist: matriz de conformidade das capabilities

**Estado:** concluído para a etapa de matriz; auditoria S0–S5 permanece fora desta etapa.
**Plano:** [`03-plano-continuidade.md`](../plans/03-plano-continuidade.md)
**Análise/matriz:** [`02-matriz-conformidade-capabilities.md`](../analyses/02-matriz-conformidade-capabilities.md)
**Base de código:** `main` em `22c861f`

## Inventário e limites

- [x] Enumerar módulos e contratos públicos no core Rust.
- [x] Localizar wrappers e exports Python e TypeScript/Node.
- [x] Relacionar testes Rust, Python e TypeScript por área.
- [x] Separar APIs implementadas, parciais e planejadas.
- [x] Registrar limites de ownership do host, cancelamento e persistência.
- [x] Identificar seleções de storage cujo adapter não é suportado.
- [x] Identificar ausência de memory/context/RAG, sessão/replay e extensão de tools.

## Trabalho que permanece nesta etapa do plano de continuidade

- [x] Criar fixture JSON compartilhada para run lifecycle, verification gate e storage selection nos bindings Rust/Python/TypeScript.
- [x] Estender a fixture comum para fallback de completion e streaming nos três bindings.
- [x] Comparar classificação de fallback, troca antes de saída e comportamento após conteúdo parcial com o código e testes locais.
- [x] Não foi reproduzida divergência entre o contrato e a implementação; registrar gaps de cobertura em vez de alterar semântica.
- [x] Executar os checks locais relevantes no snapshot desta etapa; registrar comandos e limites.
- [x] Atualizar o plano de continuidade e specs 01/02/04 para refletir a matriz e remover status contraditórios.
- [x] Fechar a validação da fixture compartilhada de policy/loop/ledger nos três bindings.
- [x] Cobrir cancelamento explícito da iteração de stream nos três bindings, sem observar eventos após o ponto de cancelamento.
- [x] Inventariar testes que dependem de serviços reais e registrar quais são opcionais e quais cenários de falha/restart ainda não são cobertos.

## Evidências desta rodada

- `git show main:...`: arquivos-fonte e testes auditados no SHA `22c861f`.
- Baseline `main`: 101 testes Rust, 38 funções Python e 28 casos Node antes da fixture compartilhada.
- Branch atual: adiciona um caso compartilhado Python e um caso Node, mais um teste de integração Rust.
- A fixture de fallback completion/stream passou nos três bindings: Rust (`cargo test -p vectora-engine --test conformance_fixtures --locked`), Python (`PYTHONPATH=bindings/python/python uv run pytest bindings/python/tests/test_binding.py -k shared_fallback_fixture -q`) e Node (`pnpm --dir bindings/typescript exec node --test test/conformance-fallback.test.mjs`).
- A nova fixture de policy, loop e ledger passou em Rust e Python: `cargo test -p vectora-engine --test conformance_fixtures --locked` (3 testes) e `PYTHONPATH=bindings/python/python uv run pytest bindings/python/tests/test_binding.py -k 'shared_conformance_fixture_covers_policy_loop_and_ledger or shared_fallback_fixture_conforms_to_completion_and_streaming' -q` (2 testes).
- Cancelamento da iteração de stream passou em Rust no teste de conformidade, Python no teste de fallback compartilhado e Node em `pnpm --dir bindings/typescript exec node --test test/conformance-fallback.test.mjs` (2 testes).
- A suíte Node completa passou: `pnpm --dir bindings/typescript exec node --test test/package.test.mjs test/conformance-fallback.test.mjs` (31 testes). O addon, loader e stub foram gerados pelo comando padrão `pnpm --dir bindings/typescript exec napi build --platform --esm --manifest-path native/Cargo.toml`. A execução anterior com `--profile dev` falhava porque o empacotador buscava o artefato no diretório errado. `native/index.js` é gerado/ignorado; `native/index.d.ts` versionado contém os exports gerados.
- Os totais são contagens estáticas; não significam que os testes foram rodados nesta etapa.
- Fixture comum passou em Rust (`cargo test -p vectora-engine --test conformance_fixtures --locked`), Python (`PYTHONPATH=bindings/python/python uv run pytest bindings/python/tests/test_binding.py -k 'shared_conformance_fixture_covers_policy_loop_and_ledger or shared_fallback_fixture_conforms_to_completion_and_streaming' -q`) e Node (`pnpm --dir bindings/typescript exec node --test --test-name-pattern='shared JSON fixture|shared fallback fixture' test/package.test.mjs test/conformance-fallback.test.mjs`).
- Rust formatting (`cargo fmt --all -- --check`), Ruff, ty (`PYTHONPATH=bindings/python/python uv run ty check utils bindings/python`), Oxlint, Prettier, `git diff --check` e links relativos passaram. O teste Node usou a saída compilada localmente; nenhum estado de CI remoto foi consultado.
- Os testes com serviços reais são condicionais: PostgreSQL e ledger resiliente consultam `VECTORA_TEST_POSTGRES_URL`; Qdrant usa `VECTORA_TEST_QDRANT_URL`; Redis e NATS usam `VECTORA_TEST_REDIS_URL` e `VECTORA_TEST_NATS_URL`. Sem essas variáveis, os testes retornam sem validar o serviço. O código cobre operações selecionadas de append/idempotência, projeção SQLite-first, escopo vetorial, TTL/pub-sub e redelivery/ACK; não há um runner que suba os quatro serviços e execute uma matriz combinada de falha, reconexão, restart e retenção. Esses casos permanecem gaps da etapa seguinte.

## Critério de conclusão

A etapa de conformidade fecha com as fixtures executadas nos três consumidores
aplicáveis, a semântica de fallback conferida e os limites de cobertura
documentados no plano e na análise. O próximo estágio deve tratar os gaps de
integração com serviços reais no plano S0–S5; esta conclusão não declara paridade
nem autoriza adoção de consumer.
