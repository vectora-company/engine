---
title: Execuções duráveis
description: Persistência local, recuperação e reconciliação sem replay automático.
weight: 4
---

Os contratos de lifecycle, loop e armazenamento SQLite são experimentais. Eles ajudam o host a registrar progresso recuperável; não executam o processo do agente, provider ou tool em background.

## Identidade e admissão

O host fornece `RunIdentity`, incluindo IDs de run, thread e turno, chave de idempotência, escopo e capabilities. A validação Rust verifica versão e formato, mas não autentica o usuário nem confirma as permissões declaradas. `SqliteRunLifecycleStore` oferece admissão e transições com escopo, chave idempotente e versão esperada.

## Eventos e checkpoints

`SqliteLedger` registra eventos de domínio com sequência por aggregate, chave de idempotência e controle de versão. Os stores de lifecycle, loop e policy mantêm snapshots/eventos próprios, com operações atômicas dentro da respectiva transação SQLite. Não existe uma transação única entre SQLite, PostgreSQL, um provider e efeitos de tool.

O `ResilientLedger` confirma primeiro a gravação local SQLite e mantém uma outbox para projetar eventos no PostgreSQL. Indisponibilidade remota pode deixar a projeção pendente; conflito de projeção precisa ser tratado como conflito, não como autorização para sobrescrever o ledger local.

## Reinício e efeitos externos

Recovery restaura estado e identifica resultados que precisam de reconciliação. Não reexecuta tools e não oferece entrega exactly-once. Se o processo caiu após um efeito externo e antes do registro do resultado, o host consulta o sistema externo com credenciais e contexto confiáveis, autentica a decisão de reconciliação e só então informa à engine um resultado conhecido ou incerto.

Planeje retenção, backup, permissões do arquivo SQLite e redação de payloads na aplicação host. Consulte os contratos de [lifecycle](https://github.com/vectora-ltda/engine/blob/main/docs/specs/08-run-lifecycle.md), [loop](https://github.com/vectora-ltda/engine/blob/main/docs/specs/11-model-tool-loop.md), [ledger SQLite](https://github.com/vectora-ltda/engine/blob/main/docs/specs/14-sqlite-durable-run-lifecycle.md) e [ledger resiliente](https://github.com/vectora-ltda/engine/blob/main/docs/specs/02-storage-memoria-contexto-rag.md).
