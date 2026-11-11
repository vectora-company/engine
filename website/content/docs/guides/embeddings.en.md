---
title: Embeddings and reranking
description: Use vector inference without mistaking it for an end-to-end RAG pipeline.
weight: 2
---

The engine provides experimental embeddings and reranking clients. They make inference requests to a host-configured provider; they do not read documents, chunk text, index content, or automatically query a vector store.

## Embeddings

`embed` accepts a versioned contract, provider/model, texts, and explicit parameters. The response contains vectors and usage metadata when the provider supplies it. Empty inputs return without a network request. `fallback_embed` takes an ordered candidate list configured by the consumer; it does not discover models or credentials.

## Reranking

`rerank` accepts a query and candidate documents and returns the indexes/ranks and scores defined by the provider contract. `fallback_rerank` also uses explicit candidates. The application decides how to handle ties, missing documents, or scores below its own threshold.

## Building retrieval

A consumer can combine these pieces into its own pipeline: normalization and chunking, embeddings, vector-adapter upsert, scoped search, and reranking. Each stage has its own failures and versions. Vector persistence is not conversational memory, and these clients do not enforce document retention, ACLs, or access policy.

Python and TypeScript interfaces mirror Rust functions through bindings. See the [capability matrix](../../reference/capabilities/) and contracts [`07-embeddings-rerank.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/07-embeddings-rerank.md) and [`02-storage-memoria-contexto-rag.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/02-storage-memoria-contexto-rag.md) before relying on schema details.
