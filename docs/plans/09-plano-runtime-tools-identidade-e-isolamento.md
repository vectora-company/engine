# Plano: Harness, tools, identidade configurável e isolamento

Este plano transforma necessidades observadas no Vectora em contratos
reutilizáveis da Engine. A Engine entrega runtime e primitives; Vectora, GH Bot
e futuros hosts configuram identidades, credenciais, raízes, policies,
adapters, sandbox e UX. Não se organiza por PR.

## Estado inicial

| Domínio                 | Estado verificado                                                                                                | Próxima lacuna                                                                                                            |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Ledger                  | SQLite append/idempotência, backup/restore, loop/policy e projeção PostgreSQL já existem em partes               | Concorrência multiprocesso completa da projeção, migrations/rollback e recovery operacional validado                      |
| Tool loop/policy        | Core Rust, bindings e stores SQLite experimentais                                                                | Integração segura contínua com brokers, cancelamento real e conformidade entre consumidores                               |
| Identidade host         | Descriptor versionado no core e validadores Python/TypeScript; grants presos ao scope, policy digest e expiração | Autenticação do issuer pertence ao host; falta ligar grants à revalidação antes de cada efeito e persistir/revogar grants |
| Filesystem/Git/shell    | Não há tool pack executável comum da Engine                                                                      | Adapters com capabilities isoladas e evidências; shell depende de broker do host                                          |
| Plano/memória/subagente | Persistência de loop e store de embeddings existem                                                               | Agregados portáveis e operações de produto configuráveis, sem persistência implícita ou privilégio novo                   |
| Navegação               | Não há orquestrador de deep search da Engine                                                                     | Contrato de busca/fetch, expansão bounded e defesa SSRF em cada hop                                                       |

## Sequência de entrega

### A. Fechar a base de durabilidade

1. Completar auditoria do ledger SQLite/PostgreSQL, concorrência multiprocesso,
   migrations versionadas, verify/rebuild e falha no meio da projeção.
2. Definir evento canônico, idempotency key, cursor, versão, parent/correlation
   IDs e referências externas de artifacts.
3. Exercitar restart, backup/restore e divergência com serviços reais e suites
   de contrato nos bindings.

### B. Contexto confiável e registro de tools

1. [x] Definir `HostIdentityContext` e `CapabilityGrant` versionados, fornecidos
       pelo host, com issuer/subject/scope, expiração, digest de policy e grants
       explícitos. O conteúdo de Soul permanece do consumidor. Core e bindings
       validam shape, validade temporal, unicidade e aderência de scope/digest; a
       autenticação do issuer continua exclusivamente no host.
2. Normalizar registro de tool: schemas, risco/efeito, capability, timeout,
   idempotência, cancellation, output limits e evidence.
3. [parcial] Há `authorize_tool_dispatch_with_identity`: vincula subject ao
   actor, scope ao run/snapshot e policy digest, deriva negações para grants
   ausentes e mantém o gate de aprovação existente. O host ainda deve chamar
   essa validação junto ao adapter imediatamente antes do efeito, autenticar e
   revogar o issuer; falta ligar persistência/reconciliação do ledger ao fluxo.

### C. Filesystem e Git

1. Implementar filesystem raiz-confinada, com resolução de symlink segura,
   operações separadas, limites, leitura paginada, busca e escrita atômica.
2. Implementar Git adapter por repositório canônico: status/diff/log/branch,
   stage por digest, commit e mutações locais com lock, cancelamento e recovery.
3. Colocar operações remotas (fetch/push/PR) em capability separada e host
   adapter; redigir credenciais e reconciliar resultado incerto.

### D. Shell e sandbox

1. Criar contrato `SandboxBroker` com capability report de estado efetivo,
   filesystem, egress, processos, ambiente e limites de recursos.
2. Adicionar execução argv sem shell implícito, timeout, limites, sinais e
   encerramento/espera da árvore de processos.
3. Integrar adapters por plataforma somente com isolamento comprovado; exibir
   `effective`, `partial`, `soft` ou `unavailable` conforme evidência. Sem
   adapter seguro, execução fica indisponível.

### E. Plano, memória e delegação

1. Persistir planos, etapas e dependências no ledger; aprovação ligada ao
   digest de entrada/policy; retomar etapa com reconciliação.
2. Definir MemoryRecord portável com proveniência, escopo, sensibilidade,
   validade, revisão, deleção e busca delegada aos stores configurados pelo
   host.
3. Implementar dispatcher de subagente que derive scope/capabilities/budget,
   propague cancelamento, persista parent-child e limite retorno.

### F. Navegação profunda

1. Definir adapters SearchProvider/DocumentFetcher e protocolo de documento
   com fonte, digest, links, redirects e data de coleta.
2. Implementar crawler breadth-first bounded com canonicalização, dedupe,
   limites por domínio, concorrência, bytes, duração e cancelamento.
3. Aplicar policy/SSRF no URL inicial e a cada redirect/link: DNS/IP, loopback,
   ranges privados, credenciais, portas e MIME; registrar fontes e falhas
   parciais.

### G. Conformidade, bindings e adoção

1. Fixtures comuns Rust/Python/TypeScript cobrem sucesso, denies, limites,
   falha, cancelamento, restart, replay e redaction.
2. Validar perfis distintos Vectora/GH Bot sem imports de implementação,
   identidade ou UI de qualquer produto.
3. Publicar somente capacidades que passaram gates; manter sandbox sem prova,
   operação remota sem autorização e capabilities ausentes em deny-by-default.

## Critérios de saída

- Nenhuma execução de tool sem contexto confiável e capability validada.
- Eventos/planos/grants persistem localmente antes de retorno e recuperam sem
  duplicar efeitos; projeção PostgreSQL pode ser reconstruída da cópia SQLite.
- Filesystem/Git não atravessam roots; shell não executa sem broker e report
  efetivo. Pesquisa não acessa redes internas nem expande sem limite.
- APIs equivalentes e fixtures passam em Rust, Python e TypeScript.
- Estado real, limitações por OS/backend e evidências refletem-se na
  [`spec`](../specs/21-harness-tools-identidade-e-isolamento.md),
  [`checklist`](../checklists/09-runtime-tools-identidade-e-isolamento.md) e
  [`análise`](../analyses/09-runtime-tools-vectora.md).

## Decisões já delimitadas

- Engine fornece primitives e contratos; o host é autoridade de identidade,
  Soul, credenciais, policy de produto, configurações, adapters e UX.
- SQLite é autoridade local do ledger; PostgreSQL é projeção recuperável.
- Segurança é deny-by-default. Configuração solicitada de sandbox não conta
  como evidência de isolamento efetivo.
