# Contribuindo com Vectora Engine

Vectora Engine está em bootstrap. Mudanças de contrato precisam ser revisadas
com a documentação de arquitetura e com os consumidores do contrato. Preserve
a implementação atual do Vectora até a
capability correspondente demonstrar conformidade e rollback.

## Requisitos

- Rust stable via `rustup`, com `rustfmt` e `clippy` (selecionados por
  `rust-toolchain.toml`).
- `scons` para os alvos de desenvolvimento.
- `uv` e Python 3.13 para checks do tooling compartilhado em `utils/`.

## Preparar o ambiente

```bash
scons install
```

O comando instala os componentes Rust selecionados, baixa dependências Cargo
conforme `Cargo.lock` e sincroniza o ambiente Python de tooling. Não instala
dependências do app Vectora nem sidecars de produto.

## Verificações

```bash
scons lint
scons tests
```

Os comandos diretos são `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
`uv run ruff check utils` e `uv run pytest`.

## Pull requests

- Use Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `chore:` etc.).
- Escreva título e descrição em português e descreva o comportamento entregue.
- Vincule uma issue pública correspondente em uma linha `Refs: #N` ou
  `Closes: #N`; não invente vínculo quando a mudança não tem issue.
- Informe os checks executados e qualquer contrato ou consumidor afetado.
- Não declare a engine pronta para integração estável antes de versionar a API
  e executar a conformidade nos bindings relevantes.
