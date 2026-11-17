# Análise — CLI independente da Engine

## Contexto confirmado

A Engine contém adapters Rust distintos para Decision Model: `LayaClient` acessa o servidor standalone Laya e `LlamaServerClient` acessa llama.cpp. Ambos falam SystemOne, mas descoberta de modelo e modalidades são próprias de cada runtime. Jev é um modelo específico, não o nome da API; Laya também fornece checkpoints próprios e permite omitir o seletor para roteamento. Laya-GGUF servido por llama.cpp usa o segundo adapter. Não se deve inferir suporte a chat OpenAI-compatible em `laya-serve`. O launcher `laya-install.sh` documenta ambos os runtimes.

O workspace não tinha binário consumidor para exercitar chat e configuração sem bindings/aplicações. A Engine já modela tool calls e mensagens de ferramenta em `ChatRequest`/`ChatCompletion`, então o CLI pode atuar como cliente fino e não precisa duplicar dispatch persistente do harness.

## Decisões de implementação

A CLI mantém configuração separada do runtime, permite secrets do ambiente e onboarding mascarado para guardar credenciais em arquivo separado com permissões restritas. OpenAI/OpenRouter usam perfis nomeados; Anthropic, Google/Gemini, Cohere, Ollama e OpenAI Responses reutilizam `NativeChatClient`; gateways como 9router/OmniRouter usam OpenAI-compatible e URL configurável. ReAct permanece limitado à ferramenta local allowlisted. SCons instala no PATH pelos alvos de build e `install`.

## Estado / evidência

Estado verificado em 17/11/2026: wizard configura clientes nativos Anthropic, Google/Gemini, Cohere, Ollama, OpenAI Responses e endpoints compatíveis OpenAI Chat/OpenRouter/custom. A credencial é capturada sem eco e gravada em arquivo separado não criptografado, modo 0600 no Unix; env do processo prevalece. `scons build` levou 14m33s no core release e 8,36s no CLI, e registrou o binário em `~/.cargo/bin`; `build-core`, resolução PATH, `--help`, chat sem argumentos e wizard smoke passaram. `cargo test -p engine-cli --locked` (8), `scons lint` e pre-commit passaram. `scons install` chama o mesmo helper antes dos downloads externos, mas o fluxo completo não foi executado. Seleção/configuração não provam inferência real com modelo; isso requer servidor e pesos configurados. A posição da branch na stack deve ser consultada no GitHub e não é registrada como organização desta análise.
