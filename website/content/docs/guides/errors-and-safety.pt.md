---
title: Erros, cancelamento e segurança
description: Trate falhas previsíveis e mantenha a fronteira de confiança no host.
weight: 7
---

Os erros são tipados por capability e desenhados para não transportar headers, credenciais ou corpos brutos de respostas de provider. Os bindings convertem falhas nativas para exceções Python ou erros JavaScript; preserve a categoria e o contexto operacional útil ao traduzi-los para a UX do produto.

## Respostas a falhas

Chat distingue configuração/contrato inválido, autenticação, quota, rate limit com `retry_after` quando disponível, falha de provider/servidor, sobrecarga, incompatibilidade de payload, conexão, timeout, resposta excessiva, resposta/protocolo inválido e cancelamento. Embeddings e rerank têm categorias tipadas correspondentes. Não faça retry indiscriminado: respeite a classificação, limites e política do consumidor.

O fallback de chat usa apenas os candidatos que o host configurou e erros explicitamente elegíveis. Durante streaming, depois de eventos visíveis ao consumidor, uma troca de provider pode duplicar ou contradizer conteúdo; siga as regras do contrato e não faça retry transparente de efeitos já emitidos.

## Segredos e confiança

Credenciais e endpoints são fornecidos pelo host. A engine não lê `.env`, não busca segredos em serviços de produto e não decide quem pode acessar um run. Logs do host devem excluir chaves, headers, prompts privados e payloads sensíveis; os stores persistem o payload que recebem, então redija-o antes de gravar.

Identidades, capabilities, grants de aprovação e reports são entradas a validar, não prova criptográfica de autoridade. O host autentica pessoas, autoriza ações, executa tools em ambiente isolado e impõe limites de recurso. A engine ainda não fornece sandbox universal, secret manager, rate limiter de produto ou runtime geral de plugins.

Veja [arquitetura e responsabilidades do host](../../concepts/architecture/), [política e aprovações](../tool-policy/) e os contratos de [chat](https://github.com/vectora-ltda/engine/blob/main/docs/specs/03-chat-openai-compatible.md) e [fallback](https://github.com/vectora-ltda/engine/blob/main/docs/specs/06-chat-fallback.md).
