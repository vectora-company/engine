# Checklist histórico: cliente OpenAI-compatible

Este documento preserva a auditoria de segurança/robustez do cliente HTTP.
Não é checklist de uma etapa atual nem fonte para estado de release.

## Findings revisados

- [x] Desativar redirect automático para evitar envio de credenciais a outro host.
- [x] Validar `Bearer {api_key}` como `HeaderValue` antes de enviar requests.
- [x] Omitir `description` e `detail` opcionais quando ausentes.
- [x] Tratar `usage: null` e `tool_calls: null` como dados ausentes.
- [x] Permitir typecheck TypeScript sem build N-API, mantendo declaration
      versionada e preservada pelo clean.
- [x] Registrar evidências e limites em
      [`docs/analyses/01-auditoria-cliente-openai-compatible.md`](../analyses/01-auditoria-cliente-openai-compatible.md).

## Evidência histórica

Na execução registrada, passaram fmt, testes focados do cliente, Clippy,
`pnpm typecheck` inclusive sem `native/index.js`, e `git diff --check`. Essa
execução é evidência histórica; repita os gates adequados ao modificar o
cliente ou seus bindings.
