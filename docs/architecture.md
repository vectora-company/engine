# Direção de arquitetura

Vectora Engine evolui por capability a partir de contratos observáveis dos
consumidores. A implementação de referência permanece no backend atual do
Vectora até haver evidência para substituir uma parte específica.

## Fluxo de uma capability

1. Identificar o comportamento atual e todos os seus consumidores.
2. Registrar schema/envelope, erros, paginação, timeout, retry, cancelamento,
   ownership, segurança e telemetria.
3. Criar casos de conformidade que caracterizem os caminhos felizes e de falha
   antes do port.
4. Implementar uma crate Rust pequena, sem acoplar regras de produto.
5. Expor bindings Python e TypeScript somente sobre contratos aprovados.
6. Comparar em workloads isolados e reproduzíveis; shadow mode não duplica
   efeitos externos.
7. Migrar com fallback reversível após conformidade, operação sustentada e
   rollback demonstrados.

## Fronteira da engine

Loop de agentes, protocolos de providers e streaming, tools e políticas,
aprovação humana, orçamento, checkpoints, cancelamento, memória/RAG,
observabilidade e cliente MCP são candidatos reutilizáveis.

Autenticação, contas, billing, RBAC específico, workspaces, UI, workbenches,
catálogo/onboarding e distribuição continuam no produto consumidor. A engine
não concede acesso com base em dados fornecidos pelo cliente e não transforma
o app Vectora em servidor MCP.

## Evidência e compatibilidade

Cada capability deve registrar versão do contrato, versão Rust/Python de
referência, SHA, ambiente, configuração, cobertura de conformidade, latência,
memória e throughput. Não generalize uma medição para cargas ou plataformas que
ela não cobriu. Mudanças incompatíveis exigem uma versão maior explícita e
período de compatibilidade definido pelos consumidores.
