---
title: Política e aprovações de tools
description: Vincule decisões a chamadas concretas mantendo a autenticação no host.
weight: 5
---

O contrato de policy é uma verificação de autorização no runtime, não uma sandbox nem uma fronteira autônoma de segurança. O host continua responsável por autenticação, execução e imposição das permissões no sistema de destino.

## Fluxo recomendado

1. O host compila a policy a partir de configuração confiável e cria um `PolicySnapshot` versionado para o escopo do run.
2. A engine avalia um `ToolContract` e a chamada do modelo. A decisão é default-deny para capability ausente ou inválida.
3. Quando a policy exige aprovação, o host apresenta o pedido ao usuário por um canal autenticado.
4. O host autentica o aprovador, cria um recibo/grant e a engine valida o vínculo com run, turno, chamada, argumentos normalizados, escopo, revisão e digest da policy.
5. Imediatamente antes da execução, o host verifica novamente que a policy e o grant ainda são válidos e impõe capability, timeout e isolamento.

`SqliteToolPolicyStore` persiste snapshots, avaliações, solicitações e recibos de aprovação com revalidação do estado mais recente. Os campos fornecidos pelo host não provam que houve aprovação humana; a engine não autentica usuários, não assina recibos e não executa adapters.

Não reutilize grants após alteração de argumentos, escopo, chamada, run ou policy. Expiração, revogação e replay precisam ser tratados conforme o contrato versionado e pelo host. Veja [`12-policy-and-approval.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/12-policy-and-approval.md) e [`16-sqlite-durable-tool-policy.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/16-sqlite-durable-tool-policy.md).
