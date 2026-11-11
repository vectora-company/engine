---
title: Matriz de capacidades
description: O que existe hoje no núcleo e bindings e o que permanece fora do escopo.
weight: 1
---

Esta matriz resume o código e as especificações atuais. “Implementada” significa que há código para o recorte declarado; não significa estabilidade, endurecimento para produção nem conformidade integral entre todos os backends.

| Área | Status | Rust | Python | TypeScript/Node | Limite importante |
| --- | --- | --- | --- | --- | --- |
| Chat compatível com OpenAI e SSE | Experimental | Sim | Sim | Sim | Sem promessa de paridade integral de gateways; host escolhe provider, endpoint e credenciais. |
| Chat nativo Anthropic, Google Gemini, Cohere | Experimental | Sim | Sim | Sim | Recortes de protocolo; host fornece configurações e policy. |
| Fallback explícito de chat | Capability implementada; API experimental | Sim | Sim | Sim | Consumidor fornece candidatos ordenados; não há descoberta automática de modelo. |
| Embeddings e reranking | Experimental | Sim | Sim | Sim | Somente inferência; sem índice documental ou RAG ponta a ponta. |
| Montagem de turnos | Experimental | Sim | Sim | Sim | Monta eventos; não executa tools nem persiste a resposta. |
| Lifecycle de runs | Experimental | Sim | Sim | Sim | Persistência SQLite existe; host é dono da identidade e do cancelamento de tarefas externas. |
| Contratos de tools e preparação de dispatch | Experimental | Sim | Sim | Sim | Valida e retorna decisões; adapters são executados no host. |
| Loop modelo/tool, recovery e cancelamento | Experimental | Sim | Sim | Sim | Host chama providers/tools e precisa persistir/reconciliar efeitos corretamente. |
| Policy, vínculo de aprovação e recibos SQLite | Experimental | Sim | Sim | Sim | Host autentica aprovadores; não é sandbox nem prova criptográfica de aprovação. |
| Gates de verificação | Experimental | Sim | Sim | Sim | Host executa checks e autentica evidências. |
| Ledger/checkpoints SQLite duráveis | Experimental | Sim | Sim | Sim | Atomicidade cobre gravações locais, não efeitos externos. |
| Ledger PostgreSQL e projeção local-first | Parcial / experimental | Sim | Parcial | Parcial | Não substitui diretamente todos os stores SQLite. |
| Adapters vetoriais LanceDB e Qdrant | Experimental | Sim | Sim | Sim | Upsert, busca e delete estão expostos; filtros, ranking e operação variam entre adapters. |
| Adapters NATS e Redis para KV/queue/pub-sub | Experimental | Sim | Sim | Sim | APIs estão expostas; semânticas de entrega, retenção e confirmação variam por backend. |
| API completa de memória, planejamento/compactação de contexto, indexação, RAG | Capability completa indisponível | Planejada/incompleta | Não há API completa | Não há API completa | Embeddings não constituem sozinhos um sistema de memória ou recuperação. |
| Framework geral de plugins/tools dinâmicas, sandbox universal, computer use, browser automation, servidor MCP | Indisponível / específico do host | Não há capability completa | — | — | Tools e isolamento de produto continuam no host; MCP é uma direção de integração cliente. |

Os bindings Python e npm compilam do código-fonte, mas a publicação dos pacotes ainda não está habilitada. Bindings Go ainda não existem. Consulte cada spec de contrato e seus testes antes de depender de uma operação.
