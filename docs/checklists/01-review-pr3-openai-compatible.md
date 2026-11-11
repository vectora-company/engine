# Checklist — review do PR #3 e contrato OpenAI-compatible

Estados desta etapa: `feito`, `pendente`, `bloqueado` ou `não aplicável`.

## Escopo e fontes

- [x] Confirmar base `main` (`22c861f`) e branch desta etapa.
- [x] Ler spec [03 — Chat OpenAI-compatible](../specs/03-chat-openai-compatible.md).
- [x] Comparar os cinco comentários do PR #3 com código e testes atuais.
- [x] Registrar classificação e evidência em
      [auditoria PR #3](../analyses/01-auditoria-review-pr3.md).

## Correções

- [x] Desativar redirect automático e manter resposta 3xx classificada como
      erro HTTP do provider.
- [x] Validar `Bearer {api_key}` como `HeaderValue` antes de enviar requests.
- [x] Omitir `description` e `detail` opcionais quando ausentes.
- [x] Preservar tratamento existente para `usage: null` e `tool_calls: null`.
- [x] Permitir typecheck TypeScript sem build N-API, versionando o `.d.ts` e
      preservando-o em `scons clean`.

## Verificação

- [x] `cargo fmt --all -- --check`.
- [x] Testes focados do módulo `chat::openai_compatible`.
- [x] `pnpm typecheck` em `bindings/typescript` com a declaração versionada.
- [x] Typecheck com `native/index.js` temporariamente ausente; o arquivo foi
      restaurado após a verificação.
- [ ] Revisar `git diff`, lista staged e arquivos ignorados antes do commit.

## Evidência desta etapa

Base: `main` em `22c861f`; implementação ainda não commitada nesta branch.

- `cargo fmt --all -- --check` — passou.
- `cargo clippy -p vectora-engine --all-targets --locked -- -D warnings` — passou.
- `cargo test -p vectora-engine chat::openai_compatible::tests --locked` — 8
  passaram, 0 falharam.
- `pnpm --dir bindings/typescript typecheck` — passou, inclusive sem o arquivo
  gerado `bindings/typescript/native/index.js`.
- `git diff --check` — passou.

Não inferir sucesso de CI remoto; esta etapa não acompanha workflows GitHub.
