# Fotografia histórica do bootstrap da Engine

> Registro histórico de 2026-11-09. Não descreve o estado atual. Para capacidades
> e gaps atuais, consulte [`docs/plans/03-plano-continuidade.md`](../plans/03-plano-continuidade.md)
> e [`docs/analyses/02-matriz-conformidade-capabilities.md`](../analyses/02-matriz-conformidade-capabilities.md).

**Base auditada:** `main` em `43a26552194d979f64d2fe12f1464f14f23f07a2`

**Escopo:** bootstrap Rust, providers, contratos iniciais do harness e testes registrados naquele snapshot.

## Núcleo e distribuição

O workspace Cargo e o crate Rust estabeleceram Rust como implementação
canônica. `SConstruct`, `utils/`, pre-commit e CI compunham o fluxo de build e
verificação. Python/PyO3 e TypeScript/N-API encaminhavam operações ao mesmo
núcleo. Havia contratos JSON versionados e fixtures locais; isso não significava
que os pacotes já estavam publicados em crates.io, PyPI ou npm.

## Providers, chat e modelos

O snapshot continha chat OpenAI-compatible, extensões de gateways, adapters
nativos Anthropic, Gemini e Cohere, streaming e fallback explícito. A regra de
fallback distinguia falhas temporárias de falhas permanentes e evitava repetir
uma resposta depois de saída parcial. Embeddings e reranking cobriam providers
selecionados, incluindo OpenAI-compatible, OpenRouter, Cohere, Voyage e Ollama,
conforme cada operação.

Essas implementações não comprovavam paridade integral entre providers,
modalidades ou bindings. Também não implicavam suporte universal a visão, áudio,
vídeo, decisão ou modelos específicos.

## Harness e ferramentas

Os contratos iniciais incluíam lifecycle em memória, montagem de turnos a
partir do stream, schema/capabilities de tools, validação e dispatch, além da
coordenação entre rodadas do modelo e resultados de tools. O host continuava
responsável por executar adapters, autenticar identidade, autorizar ações e
persistir o estado duravelmente. Os contratos não constituíam sandbox nem
motor de produto completo.

## Limites do snapshot

Storage vetorial e relacional, ledger durável, RAG, publicação dos pacotes e
adoção pelo Vectora/GH Bot não estavam demonstrados nessa base. Os planos e
specs da época eram intenção de trabalho, não evidência de implementação.
Este snapshot tampouco provava equivalência integral com o backend Python ou
vantagem de performance; essas conclusões exigem conformidade e benchmarks
reproduzíveis.

## Evidência histórica

A auditoria original inspecionou a cadeia de commits da base indicada e seus
arquivos alterados. O resultado de `scons ci` registrado naquela data foi lint
para Rust/Python/TypeScript, 44 testes Rust, 89 Python e 17 TypeScript. Esses
números descrevem apenas aquele ambiente e snapshot; não são gates nem estado
atual. A regra de fallback tinha testes unitários para categorias temporárias,
permanentes e posição do candidato.
