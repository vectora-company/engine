# Binding TypeScript/Node

O pacote `@vectora/engine` no npm usa napi-rs sobre o núcleo Rust. A API pública
em `src/` fornece tipos TypeScript e converte as mensagens JSON da fronteira
nativa; transporte, SSE, validação e erros continuam no Rust.

## Desenvolvimento

Na raiz do repositório, `scons install` instala Rust, tooling Python e
dependências pnpm. Dentro deste diretório, `pnpm build` compila o addon local e
gera a fachada JavaScript e as declarações. `pnpm test` valida o carregamento
local do addon. O build de artefatos para outras plataformas e a publicação no
npm serão habilitados em um workflow de release separado.

Consuma `chatComplete(settings, request)` como `Promise` ou `chatStream` como
`AsyncIterable`. Encerrar a iteração cancela a tarefa Rust e a requisição HTTP.
Os objetos `settings`, `request` e eventos seguem
`docs/specs/03-chat-openai-compatible.md`.
