# Contribuindo com Vectora Engine

Vectora Engine está em bootstrap. Mudanças de contrato precisam ser revisadas
com a documentação de arquitetura e com os consumidores do contrato. Preserve
a implementação atual do Vectora até a
capability correspondente demonstrar conformidade e rollback.

## Preparar o ambiente

Em Linux, o bootstrap instala os pacotes de sistema e as toolchains do projeto,
incluindo `protobuf-compiler` (`protoc`), Podman para serviços opcionais de
integração, Rust, Python 3.13, Node.js 24, pnpm, uv e SCons. Ele reconhece
distribuições Debian/Ubuntu, Fedora e Arch.
`--check` apenas diagnostica o host; `--toolchain-only` instala os requisitos
sem compilar os bindings.

```bash
./dotfiles.sh --check
./dotfiles.sh
```

Em macOS e Windows, prepare manualmente os requisitos abaixo. No macOS, instale
`protoc` com `brew install protobuf`; no Windows, instale o compilador da
release oficial de Protocol Buffers e adicione-o ao `PATH`.

O alvo `scons install` instala componentes Rust, baixa dependências conforme
`Cargo.lock`, sincroniza o tooling Python, compila a extensão Python e instala
as dependências pnpm do binding Node. Não instala dependências dos produtos
consumidores nem sidecars.

### Requisitos manuais

- Rust stable via `rustup`, com `rustfmt` e `clippy` (selecionados por
  `rust-toolchain.toml`).
- `protoc` (Protocol Buffers compiler), necessário pela dependência LanceDB.
  No Linux o script instala `protobuf-compiler` em Debian/Ubuntu/Fedora e
  `protobuf` no Arch.
- `scons` para os alvos de desenvolvimento.
- `uv` e Python 3.13 para tooling e verificações dos bindings Python.
- `pnpm` e Node.js 24 para build, lint e testes do binding TypeScript.

Se o pré-requisito `protoc` estiver ausente, `scons install` orienta executar
`./dotfiles.sh --toolchain-only` em Linux ou instalar o pacote equivalente no
macOS/Windows.

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
