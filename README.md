# Vectora Engine

Núcleo em Rust para capacidades reutilizáveis de execução de agentes nos
produtos Vectora. O repositório está em bootstrap: os contratos e limites das
APIs ainda estão sendo definidos, portanto esta crate ainda não é um destino
estável de integração.

O desenvolvimento é incremental e guiado por contratos. Cada capacidade deve
ter comportamento de referência, casos de conformidade entre implementações,
medições reproduzíveis e um caminho de rollback antes de substituir qualquer
comportamento existente. As futuras integrações Rust, Python e TypeScript serão
adicionadas conforme seus contratos forem revisados.

## Escopo

As capacidades candidatas incluem loop de agente, provedores e streaming,
ferramentas e políticas de execução, aprovação humana, limites de uso,
checkpoints, cancelamento, interfaces de memória e recuperação, cliente MCP e
observabilidade estruturada. A ordem e os contratos são definidos pela
especificação de arquitetura, não por uma promessa de paridade ou desempenho.

Autenticação, contas, cobrança, workspaces, UI, workbenches, onboarding e
distribuição continuam sendo responsabilidades dos produtos consumidores. A
engine usa MCP como cliente; ela não transforma o app Vectora em servidor MCP.
Rust é uma implementação a avaliar para cada capacidade, sem pressupor que
todo workload será mais rápido ou barato.

## Preparar o ambiente

Instale Rust stable por meio do `rustup`; o `rust-toolchain.toml` seleciona
também `rustfmt` e `clippy`. `uv` e Python 3.13 são usados pelo tooling
compartilhado em `utils/`.

```bash
scons install
```

Esse alvo instala os componentes Rust selecionados, baixa as dependências
Cargo conforme o `Cargo.lock` e prepara o ambiente Python do tooling. Ele não
instala dependências dos apps consumidores, Node.js, navegadores Playwright ou
sidecars de produto.

## Comandos

```bash
scons                 # mostra os alvos disponíveis
scons lint            # rustfmt, Clippy e Ruff
scons tests           # testes Cargo e do tooling em utils/
scons ci              # install, lint e tests
scons update          # atualiza Cargo e uv.lock; revise os lockfiles
scons clean           # remove artefatos de build
```

Comandos Rust equivalentes:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Organização do repositório

- `crates/`: crates Rust da engine.
- `bindings/`: adaptadores de linguagem quando introduzidos com contratos
  revisados.
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
