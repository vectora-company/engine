---
title: Embeddings e reranking
description: Use inferência vetorial sem confundi-la com um pipeline RAG.
weight: 2
---

A engine oferece clientes experimentais para embeddings e reranking. Eles fazem chamadas de inferência a um provider configurado pelo host; não leem documentos, não geram chunks, não indexam conteúdo e não consultam automaticamente um vector store.

## Embeddings

`embed` recebe um contrato versionado, provider/modelo, textos e parâmetros explícitos. A resposta contém vetores e metadados de uso quando o provider os oferece. Entradas vazias retornam sem chamada de rede. `fallback_embed` recebe uma lista ordenada de candidatos configurada pelo consumidor; não descobre modelos nem credenciais.

## Reranking

`rerank` recebe uma consulta e documentos candidatos e retorna os índices/ordenações e scores previstos pelo contrato do provider. `fallback_rerank` também usa candidatos explícitos. A aplicação decide o que fazer com documentos empatados, ausentes ou abaixo de um limiar próprio.

## Montar uma recuperação

Um consumidor pode combinar estes blocos com seu próprio pipeline: normalização e chunking, embeddings, upsert no adapter vetorial, busca por escopo e reranking. Cada etapa tem falhas e versões próprias. Persistência vetorial não é memória conversacional, e estes clientes não aplicam retenção, ACLs de documentos ou política de acesso.

As interfaces Python e TypeScript refletem as funções Rust por bindings. Consulte a [matriz de capacidades](../../reference/capabilities/) e os contratos [`07-embeddings-rerank.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/07-embeddings-rerank.md) e [`02-storage-memoria-contexto-rag.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/02-storage-memoria-contexto-rag.md) antes de depender de detalhes de schema.
