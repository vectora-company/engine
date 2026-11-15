# Checklist: storage de grafos FalkorDB

Estados: `feito`, `pendente`, `bloqueado`, `não aplicável`.

## Contrato e pesquisa

- [x] Confirmar branch, working tree, remote e PRs abertos; #100 é o topo.
- [x] Ler `AGENTS.md`, instruções locais, spec 02, planos gerais e estado atual
      dos adapters Rust/bindings.
- [x] Confirmar documentação do protocolo/query, parâmetros, timeout, client
      Rust, MSRV/features e persistência upstream.
- [x] Criar spec, plano e análise com caminho completo nos índices/prompt/goal.
- [ ] **Bloqueado por decisão vital:** selecionar API Cypher FalkorDB, contrato
      property graph portável ou as duas camadas; decisão pedida no chat.
- [ ] Definir versão mínima do servidor e política operacional depois de fixar
      contrato e conferir disponibilidade local do serviço.

## Implementação

- [ ] Core Rust e dependência oficial com features mínimas, após decisão.
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
