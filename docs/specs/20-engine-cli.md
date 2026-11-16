# Spec 20 — CLI independente da Engine

## Objetivo

Oferecer um executável pequeno para configurar endpoints e exercitar os contratos Rust da Engine sem depender do Vectora ou do GH Bot. A CLI é um consumidor de referência e um instrumento de desenvolvimento; não cria uma segunda implementação do runtime.

## Contrato

- Binário/package `engine-cli`, em `crates/engine-cli`, sobre `vectora-engine`.
- Configuração JSON local, selecionável por `ENGINE_CLI_CONFIG` ou por `--` caminho posicional dos comandos; padrão `$XDG_CONFIG_HOME/vectora/engine-cli/config.json` ou `$HOME/.config/vectora/engine-cli/config.json`.
- `init`, `show`, `validate` e `set <campo.dotted> <valor>` permitem criar e manter configuração. A chave de API é referenciada por nome de variável de ambiente (`api_key_env`), nunca gravada no arquivo.
- Chat interativo via rustyline. O loop ReAct chama o cliente de chat da Engine e executa somente a ferramenta local `calculator`, com expressão aritmética limitada; máximo de chamadas configurável e limitado a 32.
- `chat.backend=llama_server` atende llama.cpp, inclusive modelos Laya-GGUF carregados pelo operador. `openai_compatible` exercita endpoints que seguem esse contrato.
- `decision.backend=laya_server` usa o protocolo Jev de `laya-serve`; `llama_server` usa `/v1/systemone` de llama.cpp v1.0.0, incluindo Laya-GGUF quando servido por esse runtime. São protocolos distintos e permanecem selecionáveis.
- `decide <prompt>` envia uma pergunta tipada pelo cliente existente; não transforma Decision Model em geração de texto ou em chat.
- Interface de terminal usa rustyline, termimad para Markdown e indicatif para progresso. Interromper/EOF termina o chat. Erros não devem expor corpo HTTP, segredo ou configuração sensível.

## Fora do escopo

Execução arbitrária de shell, MCP, filesystem tools, UI TUI complexa, histórico persistido de conversas, gerenciamento de servidor/checkpoint, onboarding de providers e um runtime ReAct paralelo. O operador inicia os servidores Laya/llama.cpp externamente.

## Verificação

Unit tests cobrem serialização sem segredo, backend Laya em ambos os protocolos, edição de configuração e cálculo seguro. O pacote deve compilar e seus testes executar com `cargo test -p engine-cli --locked`; gates globais seguem `scons tests` e `scons lint`.
