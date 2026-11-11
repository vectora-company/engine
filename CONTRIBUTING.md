# Contribuindo com Vectora Engine

Vectora Engine está em bootstrap. Mudanças de contrato precisam ser revisadas
com a documentação de arquitetura e com os consumidores do contrato. Preserve
a implementação atual do Vectora até a
capability correspondente demonstrar conformidade e rollback.

## Requisitos

- Rust stable via `rustup`, com `rustfmt` e `clippy` (selecionados por
  `rust-toolchain.toml`).
- `protoc` (Protocol Buffers compiler), necessário para compilar a dependência
  LanceDB e os bindings nativos. Em Linux, instale `protobuf-compiler`; no
  macOS, `brew install protobuf`; em Windows, instale o compilador da release
  oficial de Protocol Buffers e deixe-o no `PATH`.
- `scons` para os alvos de desenvolvimento.
- `uv` e Python 3.13 para tooling e verificações dos bindings Python.
- `pnpm` e Node.js 24 para build, lint e testes do binding TypeScript.

## Preparar o ambiente

```bash
scons install
```

O comando instala os componentes Rust selecionados, baixa dependências Cargo
conforme `Cargo.lock`, sincroniza o tooling Python, compila a extensão Python
local e instala as dependências pnpm do binding Node. Não instala dependências
do app Vectora nem sidecars de produto. O alvo verifica se `protoc` está
disponível e informa o requisito antes de iniciar a compilação.

## Verificações

```bash
scons lint
scons tests
```

Os checks diretos de Rust são `cargo fmt`, Clippy com todos os targets e
features, testes com todas as features e geração de documentação sem
dependências. Para Python, o fluxo executa syntax check, Ruff, ty, Bandit e
pytest em `utils/` e `bindings/python/`. Para TypeScript, executa oxlint, `tsc`,
build do addon e testes Node. O CI roda os checks Rust em Linux, macOS e
Windows.

## Pull requests

- Use Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `chore:` etc.).
- Escreva título e descrição em português e descreva o comportamento entregue.
- Vincule uma issue pública correspondente em uma linha `Refs: #N` ou
  `Closes: #N`; não invente vínculo quando a mudança não tem issue.
- Informe os checks executados e qualquer contrato ou consumidor afetado.
- Não declare a engine pronta para integração estável antes de versionar a API
  e executar a conformidade nos bindings relevantes.
