---
title: Maturidade das capacidades
description: Interprete o status de implementação sem presumir estabilidade ou paridade.
weight: 2
---

O repositório distingue quatro estados:

| Estado | Significado |
| --- | --- |
| Implementada | Há código e testes para o escopo declarado. Isso não significa API estável nem prontidão ampla para produção. |
| Experimental | Existe contrato versionado, mas compatibilidade, cobertura operacional ou adoção por consumidores ainda podem mudar. |
| Parcial | Existem alguns adapters, bindings ou casos de conformidade; a capability completa ainda não está estabelecida. |
| Planejada / indisponível | Pode haver desenho, mas consumidores não devem depender de implementação. |

O status por área está na [matriz de capacidades](../../reference/capabilities/). Status de API pública e publicação são coisas diferentes: um binding pode compilar do código-fonte enquanto o pacote ainda não está publicado no npm ou PyPI.

Não infira desempenho, paridade, fronteira de segurança ou exactly-once pela presença de código ou por um teste unitário aprovado. Essas afirmações exigem conformidade delimitada e evidência operacional reproduzível.
