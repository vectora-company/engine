---
title: Coordenação do harness
description: Coordene runs, tools, policy, aprovações, cancelamento e verificação.
weight: 2
---

As APIs do harness são mecanismos experimentais de coordenação. Sozinhas, elas não executam um agente autônomo. O host chama providers, executa adapters de tools, persiste o estado quando necessário e é responsável por identidade e permissões.

## Contratos e dispatch de tools

Defina um `ToolContract` versionado com JSON Schema, capability necessária, classe de efeito, timeout e declaração de idempotência. `prepare_tool_dispatch` verifica a saída do modelo, os argumentos, o estado do run e as capabilities concedidas, e retorna decisões ordenadas de invocação/negação. A função **não** executa tools, impõe timeout, autentica permissões nem garante exactly-once para efeitos externos.

## Loop de modelo e tools

O loop coordena turnos de modelo e resultados das tools. O host envia uma completion validada e um dispatch preparado, recebe uma ação, executa adapters autorizados e então submete resultados correlacionados por run, turno, ID da chamada e nome da tool. Resultados fora de ordem são apresentados ao próximo turno na ordem original das chamadas.

Resultados desconhecidos de efeitos externos exigem reconciliação. Após reiniciar, recovery não reproduz tools. O host autentica quem reconcilia e confirma o estado no sistema externo antes de submeter um resultado conhecido. Cancelar um stream ativo do provider ou uma tarefa do host também é responsabilidade do host.

## Policy e aprovações

Avalie uma policy compilada pelo host e vinculada ao run imediatamente antes de executar adapters. Grants de aprovação são vinculados à chamada pendente, argumentos normalizados, escopo, revisão e digest da policy. O host autentica o aprovador e o canal de aprovação; a engine confere o vínculo e persiste o recibo fornecido ao usar o store de policy SQLite.

Não trate essas APIs como sandbox ou fronteira de segurança. Um grant correspondente não é assinatura criptográfica e a engine não comprova que uma pessoa o aprovou.

## Gates de verificação

O host executa checks e envia um report versionado com status e referências opacas de evidência. O gate avalia deterministicamente checks obrigatórios e a policy de evidência. A engine não executa testes ou CI, não busca artifacts referenciados nem verifica se a alegação de redação do host é verdadeira. SQLite pode persistir a verificação junto ao lifecycle ou loop por operações separadas.

Contratos detalhados: [`10-tool-contracts-e-dispatch.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/10-tool-contracts-e-dispatch.md), [`11-model-tool-loop.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/11-model-tool-loop.md), [`12-policy-and-approval.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/12-policy-and-approval.md) e [`15-verification-gates.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/15-verification-gates.md).
