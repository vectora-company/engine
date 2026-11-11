---
title: Gates de verificação
description: Avalie reports do host sem confundir decisão com execução dos checks.
weight: 6
---

O gate recebe um `VerificationReport` versionado que descreve checks executados pelo host. `evaluate_verification` combina status, obrigatoriedade, falha de infraestrutura e policy de evidências para produzir um veredito determinístico.

Os status distinguem sucesso, falha, erro de infraestrutura e check não executado. Evidências usam referências opacas e uma disposition policy explícita. A avaliação pode ser ligada ao lifecycle ou loop durável SQLite, mas o registro não autentica a origem do report.

O host executa comandos/tests, verifica artifacts e associa evidências aos checks. A engine não roda CI, não abre os links de evidência, não verifica a integridade de um artifact e não atesta que o report é verdadeiro. Não permita que texto do modelo se faça passar por evidência confiável: o host deve criar reports a partir dos resultados observados e aplicar as regras de gate antes de concluir a execução.

Veja a spec [`15-verification-gates.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/15-verification-gates.md) e a matriz de [maturidade](../../concepts/maturity/).
