# Análise: Harness e capabilities observadas no Vectora

## Escopo e método

Em 17/11/2026, foram consultados o `CHANGELOG.md`, o registro de capabilities,
código do backend e títulos/corpos dos 7 PRs e 22 issues abertos do repositório
`vectora-company/vectora`. Não foram lidos comentários nem checks de PR. O
Vectora é referência de comportamento, não implementação a copiar: a Engine
deve expor contratos reutilizáveis e manter UI, credenciais, identidade de
produto e adapters particulares no host.

## Evidência do produto

O changelog 0.1.24–0.1.26 e os PRs abertos descrevem ferramentas de filesystem,
terminal/Git, navegador e pesquisa; contexto/memória e grafo; planos persistidos
com aprovação; subagentes; HITL e quality gates; sandbox/egress; identidade
local; extensões isoladas; providers e Decision Models; e eventos de conversa
retomáveis. O código observado inclui `backend/tools/fs.py`, `git.py`,
`workspace.py`, `planning.py`, `memory.py`, `web.py`, `browser.py`,
`subagent_delegate.py`, `backend/engine/conversation_loop.py`,
`backend/persistence/native/ledger.py` e `backend/services/agent_factory.py`.

Issues com maior valor arquitetural para a Engine incluem #457 (Harness
auditável/policy/HITL/subagentes), #458 (ledger/memória/recuperação), #459
(Git e workspace), #460 (eventos, fila, histórico), #461/#599 (extensões
isoladas com grants mínimos), #551 (sandbox e egress), #464 (contratos do
backend) e #466 (avaliação integrada). #537 documenta código já existente no
Vectora que ainda precisa ser ligado; não prova que as mesmas capabilities
existem na Engine. PRs #601/#603 também reforçam seleção explícita de modelo e
ausência de fallback silencioso; #595/#593 tratam runtimes locais e instalação.

## Comparação com a Engine

| Domínio            | Engine agora                                                                                                     | Gap para release                                                                                |
| ------------------ | ---------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| Ledger             | SQLite authority, backup, loop/policy; projeção PostgreSQL parcial                                               | migrations/rollback, concorrência multiprocesso e recuperação operacional integral              |
| Runtime/policy     | contratos e stores experimentais; descriptor versionado; dispatch com identity grants nega capabilities ausentes | autenticação do issuer, revogação/revalidação dentro do adapter, cancelamento e sandbox         |
| Tools              | schemas e dispatch do loop                                                                                       | ferramentas reutilizáveis filesystem/Git/shell/navegação com grants, limites e evidence         |
| Plano              | estado de loop/checkpoint                                                                                        | plano multi-etapa editável/aprovável, digest-bound e retomável                                  |
| Remember/memória   | embeddings, stores KV/vector e base de RAG                                                                       | record semântico versionado, proveniência, escopo, revisão/deleção e integração Context Planner |
| Subagentes         | não há dispatcher comum executável                                                                               | execução filha persistente com capabilities/budget derivados e cancelamento                     |
| Pesquisa/navegação | transporte de provider e ferramenta não incluídos                                                                | search/fetch adapter, deep traversal bounded, fontes e SSRF/egrss a cada hop                    |
| Identidade/Soul    | produto fornece contexto confiável em forma parcial                                                              | descriptor/configuração versionada sem Engine possuir prompt, conta ou catálogo de Souls        |

## Decisões de arquitetura já sustentadas

- O host emite identidade e policy; a Engine valida versão, scope, grants,
  expiração e integridade, sem autenticar a pessoa ou definir a Soul.
- `HostIdentityContext` e `CapabilityGrant` agora validam estrutura, validade
  temporal, unicidade e vínculo a scope/policy digest. Isso não autentica o
  host nem substitui revalidação e revogação antes do efeito.
- O novo dispatch autenticado por contexto cruza actor/scope/policy digest e
  transforma grants ausentes em deny. É um gate fornecido pela Engine, não um
  broker de execução: o host ainda controla o tempo entre validação e efeito.
- Engine deve fornecer tool contracts e implementações compartilháveis, mas o
  host concede filesystem/repos/processos, credenciais, busca externa e sandbox.
- O ledger SQLite é autoridade local e o PostgreSQL é projeção recuperável.
- Sandbox é reportado pelo efeito observado; flag de configuração não conta
  como isolamento. Network e operações externas são deny-by-default.
- Pesquisa profunda é orquestração bounded sobre adapters, não um fetcher com
  acesso livre a rede privada.

## Riscos e próximos passos

1. Não expandir o escopo em uma única implementação monolítica. A ordem segura é
   durabilidade e identidade, registry/policy, filesystem/Git, sandbox/shell,
   plano/memória/subagentes e navegação profunda.
2. Separar casos de uso do Vectora (UI/workbenches, MCP hosts, OAuth) dos
   contratos da Engine; não adicionar Electron, Pydantic ou APIs privadas ao
   crate Rust.
3. Em cada capability, verificar cancelamento/timeout real do adapter, não só
   dos futures do runtime; efeitos desconhecidos entram em reconciliação.

## Verificação do descriptor de identidade

No checkout local de 17/11/2026, `scons lint` passou. `scons tests` passou com
300 testes da biblioteca Rust, 3 fixtures Rust, 8 testes CLI, 1 teste nativo
Python, 107 testes Python (10 skips) e 64 testes Node (18 skips). Os testes
específicos verificam validade, versão, scope/policy digest, grants duplicados,
limite de 1024 grants, campos desconhecidos e serialização. O validador é
estrutural: não atesta criptograficamente o issuer nem substitui
autorização/revogação do host antes do efeito. O dispatch com identidade nega
capabilities sem grant e foi testado nos três níveis. Os skips permanecem
condicionais à disponibilidade/configuração de serviços externos.

## Validação do schema de saída de tools

Em 17/11/2026, `ToolContract` passou a aceitar `output_schema` JSON Schema
opcional. `validate_tool_result_output` valida a identidade do nome da tool e
os `ToolResult.parts`; testes cobrem saída válida, schema incompatível e tool
incorreta no Rust, Python e Node. O host deve chamar o gate antes de persistir
ou expor o conteúdo. O loop continua responsável pelo limite de bytes, e
schemas ausentes não alteram o comportamento existente. `scons lint` passou;
`scons tests` passou com 301 testes Rust da biblioteca, 3 fixtures, 8 testes
CLI, 1 teste nativo Python, 107 testes Python (10 skips) e 64 testes Node (18
skips). Os skips dependem de serviços externos configurados.

## Referências locais

- [`docs/specs/21-harness-tools-identidade-e-isolamento.md`](../specs/21-harness-tools-identidade-e-isolamento.md)
- [`docs/plans/09-plano-runtime-tools-identidade-e-isolamento.md`](../plans/09-plano-runtime-tools-identidade-e-isolamento.md)
- [`docs/checklists/09-runtime-tools-identidade-e-isolamento.md`](../checklists/09-runtime-tools-identidade-e-isolamento.md)
- [Plano geral da EPIC](../plans/03-plano-continuidade.md)
