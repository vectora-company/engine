---
title: Storage e durabilidade
description: Escolha adapters preservando as garantias que eles realmente oferecem.
weight: 3
---

Os contratos de storage estão evoluindo e as garantias dos backends não são intercambiáveis. O consumidor seleciona cada categoria de forma independente e precisa considerar as capabilities e os limites do adapter escolhido.

| Categoria | Adapters presentes no código | Qualificação atual |
| --- | --- | --- |
| SQL / ledger | SQLite, ledger PostgreSQL | SQLite é a autoridade local durável dos stores implementados de lifecycle, loop, verificação e policy. PostgreSQL oferece operações de ledger e suporte a projeção; não substitui diretamente todos os stores SQLite. |
| Busca vetorial | LanceDB, Qdrant | Existem adapters, mas não se presume paridade de filtros, ranking, indexação ou operação. |
| KV / mensageria | NATS JetStream, Redis | Existem adapters de KV, fila e pub/sub; semânticas de entrega, retenção, replay e confirmação são específicas de cada backend. |

## Estado durável

Stores SQLite persistem eventos e checkpoints versionados transacionalmente quando o contrato assim define. Existem stores especializados para admissão de lifecycle, transições e verificação do loop de modelo/tools, além de snapshots e recibos de policy. O ledger PostgreSQL e a outbox local-first resiliente são capabilities separadas; não tornam todas as transições portáveis para PostgreSQL.

As transações cobrem a operação no banco escolhido, não uma chamada ao provider nem um efeito externo de tool. Se o processo parar depois de um efeito externo e antes de registrar o resultado, o host deve reconciliar, sem repetir às cegas. Não há garantia exactly-once entre backends.

## Status de memória e RAG

A engine ainda não oferece uma API completa de memória, pipeline de context planner/compactação, indexação documental, chunking ou RAG ponta a ponta. As APIs de inferência de embeddings e reranking não indexam documentos nem gravam vetores em um store. Essas responsabilidades continuam com o consumidor.

Use testes reais de integração para PostgreSQL, Qdrant e Redis ao alterar adapters. Baixar imagens de serviço não é o mesmo que iniciar ou validar os serviços. Consulte a [matriz de capacidades](../../reference/capabilities/) para detalhes de cobertura.
