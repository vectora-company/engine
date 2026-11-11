---
title: Pré-requisitos e instalação
description: Instale ferramentas do sistema separadamente das dependências do projeto.
weight: 1
---

## Configuração do Linux

O `dotfiles.sh` do repositório prepara somente as ferramentas globais de desenvolvimento no Linux. Ele identifica a distribuição e instala pacotes e toolchains do host, como Rust, Protocol Buffers, Podman, Python via pyenv, Node.js, uv, pnpm, SCons e pre-commit. O script não instala dependências deste checkout, não compila o projeto e não inicia containers.

```bash
./dotfiles.sh --check  # informa pré-requisitos ausentes
./dotfiles.sh          # instala/configura pré-requisitos do host
```

No macOS e Windows, instale os pré-requisitos de `CONTRIBUTING.md` usando os gerenciadores de pacotes da plataforma.

## Instalar dependências do checkout

Na raiz do repositório:

```bash
scons install
```

Esse comando sincroniza dependências bloqueadas de Rust/Python/Node, instala o sidecar de desenvolvimento NATS, baixa as imagens de desenvolvimento PostgreSQL, Qdrant e Redis com Podman e baixa módulos Go caso existam bindings Go. Ele não compila a engine nem inicia esses serviços.

As dependências Python são gerenciadas com `uv`; as dependências Node usam pnpm. No Linux, `pyenv` serve apenas para instalar/selecionar o interpretador Python. Use `uv` para ambientes e comandos Python do projeto.

## Serviços opcionais de integração

Os testes de integração de storage podem usar serviços locais no Podman. `scons install` baixa as imagens, mas não inicia os serviços. Inicie-os somente quando precisar desses testes, seguindo as instruções de desenvolvimento do repositório. Testes unitários e o build normal não exigem esses servidores ativos.
