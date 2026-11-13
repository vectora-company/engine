# Contribuindo com Vectora Engine

Vectora Engine está em bootstrap. Mudanças de contrato precisam ser revisadas
com a documentação de arquitetura e com os consumidores do contrato. Preserve
a implementação atual do Vectora até a
capability correspondente demonstrar conformidade e rollback.

## Preparar o ambiente

Em Linux, `dotfiles.sh` configura o host: pacotes de sistema, `protoc`, Podman,
Rust, Python 3.13, Node.js 24, pnpm, uv, SCons e pre-commit. Ele reconhece
distribuições Debian/Ubuntu, Fedora e Arch. `--check` apenas diagnostica o host.
O script não chama SCons, não instala dependências do checkout e não compila.

```bash
./dotfiles.sh --check
./dotfiles.sh
```

Em macOS e Windows, prepare manualmente os requisitos abaixo. No macOS, instale
`protoc` com `brew install protobuf`; no Windows, instale o compilador da
release oficial de Protocol Buffers e adicione-o ao `PATH`.

`scons install` baixa dependências Cargo, sincroniza o ambiente Python, instala
as dependências pnpm, baixa o sidecar NATS e prepara as imagens Podman de
PostgreSQL, Qdrant, Redis e NATS (o TTL por chave do KV exige NATS Server
2.11+). Não compila o projeto nem inicia containers.
`scons build` compila o workspace Rust, gera o wheel Python e compila o addon e
os arquivos TypeScript; quando existir `bindings/go/go.mod`, também compila Go.
Cargo e TypeScript mantêm caches incrementais locais entre builds.

### Requisitos manuais

- Rust stable via `rustup`, com `rustfmt` e `clippy` (selecionados por
  `rust-toolchain.toml`).
- `protoc` (Protocol Buffers compiler), necessário pela dependência LanceDB.
  No Linux o script instala `protobuf-compiler` em Debian/Ubuntu/Fedora e
  `protobuf` no Arch.
- `scons` para os alvos de desenvolvimento.
- `uv` e Python 3.13 para tooling e verificações dos bindings Python.
- `pnpm` e Node.js 24 para build, lint e testes do binding TypeScript (o pacote
  aceita Node.js 22.13 ou superior).
- Podman para preparar as imagens usadas pelos testes de integração com serviços.

Se o pré-requisito `protoc` estiver ausente, `scons install` orienta executar
`./dotfiles.sh` em Linux ou instalar o pacote equivalente no
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
- Informe os checks executados localmente e qualquer contrato ou consumidor
  afetado; resultado de CI remoto não substitui a verificação local.
- PRs são empilhados: cada PR novo usa como base a branch do PR anterior aberto
  (veja `docs/plans/05-loop-de-desenvolvimento.md`).
- Não declare a engine pronta para integração estável antes de versionar a API
  e executar a conformidade nos bindings relevantes.
