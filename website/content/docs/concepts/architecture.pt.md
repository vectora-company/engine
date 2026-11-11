---
title: Arquitetura e responsabilidades do host
description: Como produtos compartilham o runtime sem delegar suas políticas de produto.
weight: 1
---

## Uma implementação, vários consumidores

Rust define os contratos e a implementação canônica. Os pacotes Python e TypeScript/Node são bindings que chamam o mesmo núcleo Rust; não são motores agênticos separados. Entre os consumidores previstos estão o backend Python do Vectora e o GH Bot TypeScript, que pode ser implantado de forma independente, além de produtos futuros.

```text
Host do produto (UI, identidade, configuração, integrações)
             │ contrato tipado / binding
             ▼
       Vectora Engine (Rust)
       ├── providers e protocolos de modelo
       ├── coordenação do harness e contratos de policy
       └── adapters de storage e mensageria
```

## Responsabilidades do host

O host autentica usuários e aprovadores, define escopo e capabilities confiáveis, gerencia credenciais e segredos, escolhe providers/modelos/storage e executa tools específicas do produto. Também é dono da UX, contas, billing, workspaces, dados, políticas de release e implantação. Ele aplica timeout e cancelamento às próprias tarefas, protege arquivos locais, redige dados antes da persistência e confirma efeitos externos durante a reconciliação.

## Responsabilidades da engine

A engine oferece mecanismos reutilizáveis e tipados: adapters de protocolo de provider, eventos de streaming, montagem de turnos, validação e preparação de dispatch de tools, máquinas de estado de runs/loops, avaliação de policy, vínculo de aprovação, gates de verificação e adapters de storage. Cada capability precisa definir erros, limites, ownership, cancelamento, compatibilidade e conformidade antes de ser contrato público de integração.

## Fronteira de extensibilidade

A engine pode validar e coordenar uma chamada sem conhecer a implementação da tool do produto. Um adapter pode executar no host, em sandbox, subprocesso ou serviço remoto; o local de execução não muda o contrato comum da chamada. A engine ainda não oferece um runtime geral de plugins dinâmicos, sandbox universal nem catálogo de todas as tools de produto.

A mesma regra vale para skills, workflows, backends de memória, hooks e extensões de provider: contratos reutilizáveis podem pertencer à engine, enquanto o comportamento específico permanece no host. Não presuma que uma extensão planejada do framework já esteja disponível.

## Fronteira de confiança

Snapshots JSON e identidades fornecidas ao binding são dados, não comprovação de identidade. A engine pode conferir se um recibo fornecido pelo host corresponde à chamada pendente, mas somente o host autentica a pessoa e o canal de aprovação. Da mesma forma, reports de verificação descrevem checks feitos pelo host; a engine não executa CI, inspeciona artifacts nem atesta que um resultado alegado é verdadeiro.
