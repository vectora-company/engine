# Vectora Engine

Núcleo em Rust para capacidades reutilizáveis de execução de agentes nos
produtos Vectora. O repositório está em bootstrap: os contratos e limites das
APIs ainda estão sendo definidos, portanto esta crate ainda não é um destino
estável de integração.

Documentação pública: [docs.vectora.company/engine](https://docs.vectora.company/engine/).

O desenvolvimento é incremental e guiado por contratos. Cada capacidade deve
ter comportamento de referência, casos de conformidade entre implementações,
medições reproduzíveis e um caminho de rollback antes de substituir qualquer
comportamento existente. Rust é a implementação canônica. A engine será
distribuída como crate Rust, pacote Python no PyPI e pacote TypeScript no npm.
Os bindings Python e TypeScript encaminham chamadas ao mesmo núcleo Rust; não
mantêm motores nem regras de execução duplicados.

## Produtos consumidores

| Consumidor      | Pacote                        | Uso                                               |
| --------------- | ----------------------------- | ------------------------------------------------- |
| Vectora Backend | Python no PyPI                | O backend Python delega chamadas ao runtime Rust. |
| Vectora GH Bot  | TypeScript/Node no npm        | O bot TypeScript delega chamadas ao runtime Rust. |
| Aplicações Rust | `vectora-engine` no crates.io | Integração direta com a API Rust canônica.        |

As bindings traduzem tipos, eventos, erros e cancelamento conforme contratos
compartilhados e testes de conformidade. Publicar três pacotes não significa
manter três implementações.

## Escopo

As capacidades candidatas incluem loop de agente, provedores e streaming,
ferramentas e políticas de execução, aprovação humana, limites de uso,
checkpoints, cancelamento, interfaces de memória e recuperação, cliente MCP e
observabilidade estruturada. A ordem e os contratos são definidos pela
especificação de arquitetura, não por uma promessa de paridade ou desempenho.

Autenticação, contas, cobrança, workspaces, UI, workbenches, onboarding e
distribuição continuam sendo responsabilidades dos produtos consumidores. A
engine usa MCP como cliente; ela não transforma o app Vectora em servidor MCP.
Cada capacidade Rust deve ser avaliada sem pressupor que todo workload será
mais rápido ou barato.

## Preparar o ambiente

No Linux, `dotfiles.sh` detecta a distribuição e configura somente o host:
pacotes de sistema, Rust, `protoc`, Podman, Python 3.13, Node.js 24, pnpm, uv,
SCons e pre-commit. Ele não instala dependências do checkout, não chama SCons,
não compila o projeto e não inicia containers.

```bash
./dotfiles.sh --check
./dotfiles.sh
```

Em macOS e Windows, instale os requisitos de `CONTRIBUTING.md` diretamente.
`scons install` instala dependências do projeto, baixa o binário NATS e prepara
imagens Podman de PostgreSQL, Qdrant e Redis para desenvolvimento. Não compila
bindings nem inicia containers. `scons build` é o alvo explícito para compilar o
workspace Rust e os bindings presentes.

## Comandos

```bash
scons                 # mostra os alvos disponíveis
scons install         # instala dependências do projeto e sidecars de desenvolvimento
scons build           # compila Rust e bindings Python/TypeScript (e Go quando existir)
scons lint            # Rust: rustfmt + Clippy; Python: Ruff + ty + Bandit
scons tests           # testes Cargo, pytest e pacote Node nativo
scons ci              # install, build, lint e tests
scons update          # atualiza Cargo e uv.lock; revise os lockfiles
scons clean           # remove artefatos de build
```

Comandos Rust equivalentes:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo doc --workspace --all-features --no-deps --locked
```

O pre-commit e o CI verificam sintaxe, lint, formatação, tipos, segurança e
testes Python e TypeScript. CI também compila os addons nativos e executa os
checks Rust em Linux, macOS e Windows.

## Organização do repositório

- `crates/`: crates Rust da engine e API canônica.
- `bindings/python/`: binding para o Vectora Backend, distribuído no PyPI.
- `bindings/typescript/`: binding Node/TypeScript para o Vectora GH Bot,
  distribuído no npm.
- `utils/`: verificações e automações compartilhadas do repositório.
- `docs/`: decisões de arquitetura e contratos específicos da engine.
- `.github/workflows/`: CI Rust multiplataforma e tooling do repositório.

O backend atual do Vectora continua sendo a referência de comportamento
enquanto as capacidades são avaliadas. Não use este repositório como autorização
para uma reescrita ampla ou para remover o backend existente antes de demonstrar
conformidade, operação sustentada e rollback.

## Licença e governança

Vectora Engine é open source sob a licença GNU AGPL-3.0, a mesma licença do
Vectora. Consulte [`LICENSE`](LICENSE). A organização oficial é
[vectora-ltda](https://github.com/vectora-ltda); o fundador e principal
mantenedor é [brunosrz](https://github.com/brunosrz/).
