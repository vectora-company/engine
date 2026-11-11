# Checklist: matriz de conformidade das capabilities

**Estado:** em andamento
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
- [x] Comparar classificação de fallback, troca antes de saída e comportamento após conteúdo parcial com o código e testes locais.
- [x] Não foi reproduzida divergência entre o contrato e a implementação; registrar gaps de cobertura em vez de alterar semântica.
- [ ] Executar os checks locais relevantes no snapshot final; registrar SHA, comandos e limites.
- [x] Atualizar o plano de continuidade e specs 01/02/04 para refletir a matriz e remover status contraditórios.
- [ ] Estender fixtures comuns a fallback e policy/loop, e cobrir cancelamento de stream explicitamente.
- [ ] Após fixtures e fallback, revisar cobertura service-real antes de avançar à auditoria detalhada S0-S5 de storage.

## Evidências desta rodada

- `git show main:...`: arquivos-fonte e testes auditados no SHA `22c861f`.
- Baseline `main`: 101 testes Rust, 38 funções Python e 28 casos Node antes da fixture compartilhada.
- Branch atual: adiciona um caso compartilhado Python e um caso Node, mais um teste de integração Rust.
- Os totais são contagens estáticas; não significam que os testes foram rodados nesta etapa.
- Fixture comum passou em Rust (`cargo test -p vectora-engine --test conformance_fixtures --locked`), Python (`PYTHONPATH=bindings/python/python uv run pytest bindings/python/tests/test_binding.py -k shared_conformance_fixture -q`) e Node (`pnpm exec tsc -p tsconfig.json`; `pnpm exec node --test --test-name-pattern='shared JSON fixture' test/package.test.mjs`).
- Rust formatting, Ruff, ty (com `PYTHONPATH` do pacote), Oxlint, TypeScript typecheck, `git diff --check` e links relativos passaram. Testes executados contra o worktree antes do commit; teste Node usou a saída compilada localmente.

## Critério de conclusão

A etapa de conformidade só fecha quando fixtures para os contratos definidos
no plano executam nos três consumidores aplicáveis, a semântica de fallback
está conferida e os gaps de cobertura aprovados, e o estado/limites da matriz
estão atualizados no plano e nas specs. Enquanto esses itens estiverem
pendentes, não iniciar adoção de consumer nem declarar paridade.
