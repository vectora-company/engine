# Plano: CLI independente para exercitar a Engine

**Estado em 16/11/2026:** PR #121 aberto sobre #120; branch `feat/engine-cli`. Wizard agora configura llama.cpp, Ollama, OpenAI Chat/Responses, OpenRouter, Anthropic, Gemini, Cohere e routers customizados; Decision Model seleciona Laya server ou llama.cpp/Laya-GGUF. As credenciais são capturadas sem eco e armazenadas separadamente com permissões 0600 no Unix. `scons build` concluiu e registrou o comando; `command -v engine-cli`, `--help`, wizard smoke e 8 testes passaram. Falta validar pre-commit e subir a atualização no PR #121; `scons install` chama o mesmo build/registro, mas ainda não foi executado isoladamente. Ver contrato em [`docs/specs/20-engine-cli.md`](../specs/20-engine-cli.md), análise em [`docs/analyses/08-auditoria-engine-cli.md`](../analyses/08-auditoria-engine-cli.md) e checklist em [`docs/checklists/08-engine-cli.md`](../checklists/08-engine-cli.md).

## Etapas

1. Criar crate, comandos de manutenção e configuração sem segredos no JSON. **Concluído.**
2. Adicionar chat e loop ReAct limitado com calculator. **Concluído.**
3. Selecionar Decision Model entre `laya-serve` e `llama-server`/Laya-GGUF. **Concluído.**
4. Wizard para providers nativos e compatíveis, URL, modelo e chave mascarada. **Concluído; 8 testes e smoke manual passaram.**
5. Registrar automaticamente o executável no PATH por `scons build` e `scons install`. **Build testado; install usa o mesmo helper e aguarda verificação isolada.**
6. Rodar gates/pre-commit, atualizar PR #121 e iniciar a próxima etapa sem aguardar CI remoto.

## Limites e aceitação

Os clientes/contratos de produção são reutilizados; nenhum segredo é serializado; parâmetros e chamadas de tool têm validação e limites; os dois protocolos de Decision Model são exercitados por testes determinísticos; documentação geral e índice de docs apontam para os três documentos específicos. PR e branch são empilhados, sem tocar em arquivos locais fora deste escopo.
