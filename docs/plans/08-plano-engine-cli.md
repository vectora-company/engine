# Plano: CLI independente para exercitar a Engine

**Estado em 16/11/2026:** branch `feat/engine-cli`, filha do PR #120. A crate, os comandos de configuração, chat ReAct e dispatch Decision Model para os dois protocolos já foram implementados. Cinco testes de crate, smoke de configuração, `scons tests`, `scons lint`, pre-commit e `scons build-cli` passaram. Publicação do PR filho do #120 pendente. Ver contrato em [`docs/specs/20-engine-cli.md`](../specs/20-engine-cli.md), análise em [`docs/analyses/08-auditoria-engine-cli.md`](../analyses/08-auditoria-engine-cli.md) e checklist em [`docs/checklists/08-engine-cli.md`](../checklists/08-engine-cli.md).

## Etapas

1. Criar crate de workspace `engine-cli`, config local sem persistir credenciais, comandos de manutenção e teste de configuração.
2. Adicionar chat interativo sobre os clientes existentes e loop ReAct limitado com ferramenta calculator sem execução arbitrária.
3. Adicionar comando de Decision Model com seleção independente entre `laya-serve` e `llama-server`/Laya-GGUF.
4. Rodar testes do crate e gates globais; integrar no SCons install/test/lint quando necessário; publicar PR filho do #120.
5. Começar a próxima etapa da stack sem aguardar CI remoto.

## Limites e aceitação

Os clientes/contratos de produção são reutilizados; nenhum segredo é serializado; parâmetros e chamadas de tool têm validação e limites; os dois protocolos de Decision Model são exercitados por testes determinísticos; documentação geral e índice de docs apontam para os três documentos específicos. PR e branch são empilhados, sem tocar em arquivos locais fora deste escopo.
