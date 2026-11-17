# Harness: identidade configurável, tools, isolamento e navegação

## Objetivo

Oferecer a Vectora, GH Bot e outros hosts um runtime Rust reutilizável para
identidades configuradas pelo consumidor, execução auditável, tools de
filesystem/Git/shell, planejamento, memória, subagentes e pesquisa navegável.
Essas superfícies devem compartilhar policy, ownership, orçamento, cancelamento,
ledger e evidências. Uma tool não ganha autoridade por ter sido chamada pelo
modelo.

## Fronteira de responsabilidade

- O host autentica usuários e serviços, define as identidades (“Souls”), emite
  contexto confiável e decide policy de produto. A Engine aceita e valida um
  descriptor versionado fornecido pelo host; não mantém catálogo de Souls,
  credenciais ou contas do Vectora/GH Bot.
- A Engine oferece contratos e implementações reutilizáveis de tools. O host
  fornece adapters/capability brokers, raízes, credenciais e configuração de
  sandbox. API de produto, UI e workbenches continuam no consumidor.
- Uma capability ausente, expirada, ambígua ou revogada nega o efeito. O modelo,
  resultado de busca, memória e conteúdo de arquivos são dados não confiáveis.
- `HostIdentityContext` v1 identifica issuer/subject e a configuração do
  consumidor por ID, revisão e digest. `CapabilityGrant` vincula capability,
  scope, policy digest e validade temporal; o core limita cada contexto a 1024
  grants, rejeita duplicatas e campos desconhecidos. O host autentica o issuer
  e controla revogação; a validação do contrato não substitui isso.
- O ledger SQLite é a autoridade local para execução e recuperação. PostgreSQL
  pode receber projeções; falha ou ausência dele não apaga estado confirmado.

`authorize_tool_dispatch_with_identity` vincula o subject ao actor do run, o
scope ao run e ao snapshot, e o policy digest ao contexto. Somente grants
válidos habilitam a capability requerida; capability ausente vira decisão
`deny` no próprio resultado de dispatch. O método não autentica issuer nem
observa revogação posterior: o host autentica o contexto e chama a validação
imediatamente antes de cada efeito no adapter. O caminho existente baseado em
snapshot continua disponível para hosts que já compilam e autenticam esse
snapshot fora da Engine.

## Contrato comum de tool

Cada tool registra nome e versão, schemas de entrada/saída, capabilities
necessárias, classe de efeito, limites, timeout, cancelamento, política de
retry/idempotência, suporte a dry-run e referência de evidência. O runtime
valida argumentos antes da execução, revalida a policy imediatamente antes do
efeito e persiste intenção/resultado no ledger. Efeitos externos incertos entram
em reconciliação; replay não os executa de novo silenciosamente.

`ToolContract.output_schema` é opcional e descreve o array serializado de
`ToolResult.parts`. O adapter/host chama `validate_tool_result_output` antes de
persistir ou expor o resultado ao modelo; schema ausente mantém compatibilidade
e não remove os limites de bytes já impostos pelo loop. A validação não executa
o adapter nem substitui redaction, limites de tamanho ou verificação de mídia.

### Filesystem e Git

- Filesystem opera somente em raízes concedidas, resolve caminhos reais,
  rejeita traversal/symlink escape e impõe limites de bytes, arquivos e tempo.
  Leitura, listagem, busca, escrita atômica, patch e remoção são capabilities
  distintas. Escrita pode produzir diff e digest antes da aplicação.
- Git usa o repositório canônico resolvido pelo host, serializa mutações por
  repositório e vincula staging/diff a snapshot/hash. Leitura local é distinta
  de alteração local e publicação remota. Push, fetch, merge/rebase e operações
  destrutivas exigem capabilities específicas; integrações de PR ficam no host.
- Credenciais não são passadas ao modelo nem expostas em remotes, argumentos,
  erros ou logs. Inicialização e operações repetíveis informam idempotência;
  resultado remoto incerto exige reconciliação.

### Shell e sandbox

- Shell recebe executável/argv/cwd/env explicitamente aprovados; não interpola
  shell por padrão. Cada processo tem timeout, limite de saída, cancelamento e
  encerramento confirmado de descendentes.
- Um `SandboxBroker` do host executa o processo e devolve um relatório assinado
  ou autenticado pelo host com capabilities efetivas, plataforma, backend,
  restrições, lacunas e motivo. A Engine distingue isolamento efetivo, parcial,
  soft e indisponível; uma configuração não prova isolamento.
- Rede/egress, paths, processos, ambiente e recursos são deny-by-default quando
  o host não concede explicitamente a capability. A Engine não finge fornecer
  uma implementação sandbox universal nem protege contra atacante determinado
  fora do modelo de ameaça do host.

### Planos, memória e subagentes

- Plano é um agregado persistido com etapas, dependências, entradas/saídas
  esperadas, estado, digest de policy e aprovações vinculadas ao conteúdo.
  Editar etapa ou contexto relevante invalida aprovações. Execução retoma por
  etapa sem repetir efeitos confirmados.
- “Remember” usa registros estruturados com ID, tipo, conteúdo, origem,
  proveniência, escopo, sensibilidade, validade, confiança/revisão e estado de
  remoção. O host controla identidade e acesso; conteúdo recuperado não altera
  policy confiável. Escrita e esquecimento são auditáveis e respeitam retenção.
- Subagente é uma execução filha persistida com parent/run IDs, objetivo,
  scope, allowlist de capabilities, budgets e deadline derivados do pai.
  Delegação não amplia privilégios; cancelamento e término propagam e o retorno
  é tipado, limitado e ligado à evidência.

### Navegação e pesquisa profunda

- Busca externa e fetch são adapters do host; o orquestrador da Engine pode
  expandir links descobertos em níveis sucessivos com orçamento explícito de
  profundidade, páginas, bytes, tempo, concorrência e domínios.
- Canonicalização/deduplicação, redirect validation, DNS/IP privados,
  credenciais, tipos MIME e tamanho são validados a cada hop para impedir SSRF
  e traversal para redes internas. Domain allow/deny e egress são fornecidos
  pela policy do host.
- Cada documento conserva URL final, origem, timestamp, digest, links
  descobertos e relações de proveniência. Conteúdo da web é não confiável e
  nunca vira instrução de sistema. Falha/limite parcial aparece no resultado;
  nenhuma expansão ilimitada ou scraping oculto é permitido.

## Ledger e recuperação

Runs, planos, tool calls, aprovações, grants, artifacts e evidências usam IDs
estáveis, sequências, operation/idempotency keys, versões de schema, parent IDs
e estados terminais. Append local SQLite é confirmado antes de projetar para
PostgreSQL. Backup/restore, verify/rebuild e migrações são idempotentes e
observáveis. Artefatos grandes ficam fora do ledger, referenciados por digest,
tamanho, store e política de expiração/acesso.

## APIs e compatibilidade

Rust é canônico; Python e TypeScript encaminham ao mesmo core. Contratos são
versionados, com erros tipados e fixtures comuns. Binding não executa política
própria nem mantém runtime paralelo. Superfícies host-only recebem referências
opacas a capabilities/adapters e nunca serializam secrets.

## Critérios de aceite

- Dois hosts configuram identidades, tools e políticas distintas sem tipos de
  produto embutidos na Engine.
- Tentativas sem capability, com path escape, credencial, egress não permitido,
  approval stale ou sandbox indisponível falham antes do efeito.
- Restart/replay não duplica eventos nem repete efeito externo incerto; ledger
  local permanece recuperável sem PostgreSQL.
- Filesystem, Git e shell cobrem sucesso, erro, limite, concorrência,
  cancelamento, recuperação e paridade dos bindings.
- Plano, memória e subagente respeitam escopo e budgets; remoção/revogação é
  observável e persistente.
- Pesquisa multi-hop aplica limites e bloqueia redirects/endereços internos;
  cada fato retornado pode ser ligado à fonte e digest.
- Garantias de sandbox são reportadas conforme execução observada, não conforme
  configuração solicitada.

## Estado observado em 2026-11-17

O loop, stores SQLite de loop/policy, ledger e APIs genéricas de chat/tools são
fundação experimental. `HostIdentityContext` e `CapabilityGrant` versionados
agora têm validadores no core Rust e nos bindings Python/TypeScript: conferem
campos obrigatórios, validade temporal, unicidade, vínculo de scope/digest,
limite de 1024 grants e rejeição de campos desconhecidos.
Essa validação não autentica o issuer nem torna o grant revogável; o host
precisa autenticar a assertion e impedir uso após revogação. A Engine ainda não
oferece tool packs de filesystem, Git, shell, navegação, plano persistente de
alto nível, memória estruturada ou dispatch de subagentes; não há broker
sandbox. Cada item tem estado e dependências em
[`docs/plans/09-plano-runtime-tools-identidade-e-isolamento.md`](../plans/09-plano-runtime-tools-identidade-e-isolamento.md).
