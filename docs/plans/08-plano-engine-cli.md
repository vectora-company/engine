# Plano: CLI independente para exercitar a Engine

**Estado verificado em 17/11/2026:** o wizard configura llama.cpp, Ollama, OpenAI Chat/Responses, OpenRouter, Anthropic, Gemini, Cohere e gateways compatíveis; o Decision Model seleciona Laya standalone ou llama.cpp/Laya-GGUF. Credenciais são capturadas sem eco e ficam separadas do JSON, com modo 0600 no Unix. `scons build`, registro em PATH, `--help`, smoke do wizard, 8 testes, lint e pre-commit passaram. `scons install` chama o helper comum, mas o fluxo integral de instalação não foi executado. Branch, PR e base da stack são estado operacional: consulte GitHub antes de atualizar ou empilhar. Ver [`docs/specs/20-engine-cli.md`](../specs/20-engine-cli.md), [`docs/analyses/08-auditoria-engine-cli.md`](../analyses/08-auditoria-engine-cli.md) e [`docs/checklists/08-engine-cli.md`](../checklists/08-engine-cli.md).

## Etapas

1. Criar crate, comandos de manutenção e configuração sem segredos no JSON. **Concluído.**
2. Adicionar chat e loop ReAct limitado com calculator. **Concluído.**
3. Selecionar Decision Model entre `laya-serve` e `llama-server`/Laya-GGUF. **Concluído.**
4. Wizard para providers nativos e compatíveis, URL, modelo e chave mascarada. **Concluído; 8 testes e smoke manual passaram.**
5. Registrar automaticamente o executável no PATH por `scons build` e `scons install`. **Build testado; install usa o mesmo helper e aguarda verificação isolada.**
6. Rodar gates/pre-commit, publicar a atualização no PR correspondente e iniciar a próxima etapa sem aguardar CI remoto.

## Limites e aceitação

Os clientes/contratos de produção são reutilizados; nenhum segredo é serializado; parâmetros e chamadas de tool têm validação e limites; os dois protocolos de Decision Model são exercitados por testes determinísticos; documentação geral e índice de docs apontam para os três documentos específicos. PR e branch são empilhados, sem tocar em arquivos locais fora deste escopo.
