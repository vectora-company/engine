# Checklist: storage de grafos FalkorDB

Estados: `feito`, `pendente`, `bloqueado`, `não aplicável`.

## Contrato e pesquisa

- [x] Confirmar branch, working tree, remote e PRs abertos; #100 é o topo.
- [x] Ler `AGENTS.md`, instruções locais, spec 02, planos gerais e estado atual
      dos adapters Rust/bindings.
- [x] Confirmar documentação do protocolo/query, parâmetros, timeout, client
      Rust, MSRV/features e persistência upstream.
- [x] Criar spec, plano e análise com caminho completo nos índices/prompt/goal.
- [x] `brunosrz` escolheu as duas camadas: contrato portátil mais extensão Cypher FalkorDB.
- [ ] **Bloqueado por decisão vital:** escolher dependência/MSRV: client 0.11.2 com Rust 1.95,
      validar 0.10.4 com Rust 1.89, ou protocolo direto usando redis existente.
- [ ] Definir versão mínima do servidor e política operacional depois de fixar
      dependência e conferir disponibilidade local do serviço.

## Implementação

- [ ] Core Rust e dependência com features mínimas, após decisão de MSRV.
- [ ] Seleção versionada, erros, capabilities e segurança do nome do grafo.
- [ ] Queries tipadas/parametrizadas, timeout, cancelamento e limites.
- [ ] Bindings Python/TypeScript, stubs, fixture e snapshot.
- [ ] Testes unitários, erros, limites e conformidade dos bindings.
- [ ] Integração service-real FalkorDB com versão/configuração registradas.
- [ ] Restart e verificação de persistência apenas sob configuração explícita;
      não declarar garantia universal.

## Entrega

- [ ] Atualizar spec, plano, análise, checklists, índices e Q&As afetadas.
- [ ] Format, lint, testes adequados e `git diff --check`.
- [ ] Revisar staging para excluir mudanças preexistentes do usuário.
- [ ] Commit, push e PR empilhado sobre a branch anterior da stack.
