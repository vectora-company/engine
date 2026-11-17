# Spec 20 — CLI independente da Engine

## Objetivo

Oferecer um executável pequeno para configurar endpoints e exercitar os contratos Rust da Engine sem depender do Vectora ou do GH Bot. A CLI é um consumidor de referência e um instrumento de desenvolvimento; não cria uma segunda implementação do runtime.

## Contrato

- Binário/package `engine-cli`, em `crates/engine-cli`, sobre `vectora-engine`.
- Configuração JSON local, selecionável por `ENGINE_CLI_CONFIG` ou caminho posicional; padrão `$XDG_CONFIG_HOME/vectora/engine-cli/config.json` ou `$HOME/.config/vectora/engine-cli/config.json`. Sem argumentos, a primeira execução abre o wizard e as seguintes entram no chat.
- `init`, `show`, `validate` e `set <campo.dotted> <valor>` permitem manter configuração. Chaves são referenciadas por nome de variável de ambiente (`api_key_env`), nunca gravadas no JSON.
- `setup` configura llama.cpp/Laya-GGUF, Ollama, OpenAI Chat Completions/Responses, OpenRouter, Anthropic, Google Gemini, Cohere e endpoints OpenAI-compatible como 9router/OmniRouter. O segredo é digitado sem eco e guardado em `secrets.json` separado, com permissões owner-only no Unix; variáveis já exportadas no ambiente têm precedência.
- Providers nativos usam os clientes/protocolos implementados no core; OpenAI/OpenRouter têm perfis OpenAI-compatible; routers e gateways adicionais usam URL customizada quando compatíveis.
- Chat interativo via rustyline. O loop ReAct chama o cliente de chat da Engine e executa somente a ferramenta local `calculator`, com expressão aritmética limitada; máximo de chamadas configurável e limitado a 32.
- `chat.backend=llama_server` atende llama.cpp, inclusive modelos Laya-GGUF carregados pelo operador. `openai_compatible` exercita OpenAI, OpenRouter, Ollama e gateways que seguem esse contrato.
- Decision Model é a categoria de uso; Jev é um modelo específico. `decision.backend=laya_server` usa o servidor standalone Laya e o protocolo SystemOne; `llama_server` usa `/v1/systemone` do llama.cpp v1.0.0, inclusive para Laya-GGUF servido nesse runtime. São adapters para runtimes distintos sobre SystemOne. No CLI, Laya aceita seletor opcional de checkpoint (vazio deixa o servidor rotear); llama-server exige model id para descoberta de capabilities.
- `decide <prompt>` envia uma pergunta tipada pelo cliente existente; não transforma Decision Model em geração de texto ou em chat.
- Interface de terminal usa rustyline, termimad para Markdown e indicatif para progresso. Interromper/EOF termina o chat. Erros não devem expor corpo HTTP, segredo ou configuração sensível.

## Fora do escopo

Execução arbitrária de shell, MCP, filesystem tools, UI TUI complexa, histórico persistido de conversas, gerenciamento de servidor/checkpoint e um runtime ReAct paralelo. O operador inicia os servidores Laya/llama.cpp externamente.

`scons build`, seus alvos focados de build e `scons install` instalam o executável em `~/.cargo/bin/engine-cli`, disponível no PATH usual do rustup. A chave local é texto legível no arquivo separado e não é criptografada; o modo Unix é 0600.

Fluxo: executar `scons install` para instalar dependências e o comando, depois `scons build` para construir core e bindings. Rodar `engine-cli`; na primeira execução abre o wizard e salva config, depois inicia o chat ReAct.

## Verificação

Unit tests cobrem serialização sem segredo, perfis/provider, permissões do arquivo de segredo, ambos os protocolos Laya, edição de configuração e cálculo seguro. O pacote deve compilar e seus testes executar com `cargo test -p engine-cli --locked`; gates globais seguem `scons tests` e `scons lint`.
