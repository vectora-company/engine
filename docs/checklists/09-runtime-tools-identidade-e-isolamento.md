# Checklist: Harness, tools, identidade e isolamento

Estados: `feito`, `parcial`, `pendente`, `bloqueado`. Itens feitos apontam para
implementação e teste; planejar não equivale a implementar.

## Estado inicial verificado — 2026-11-17

- [parcial] Ledger SQLite local, backup/restore e loop/policy persistidos; veja
  [`docs/plans/04-plano-storage-ledger.md`](../plans/04-plano-storage-ledger.md).
- [parcial] Tool schemas, loop, policy snapshot e aprovação existem; a Engine
  ainda não executa tool packs seguros por si.
- [parcial] Descriptor versionado `HostIdentityContext` e `CapabilityGrant`;
  validadores Rust/Python/TypeScript conferem expiração, unicidade e vínculo
  de scope/policy digest e impõem limite de 1024 grants. O host ainda precisa
  autenticar o issuer e garantir revogação imediatamente antes do efeito.
- [pendente] Filesystem root-confined e ferramentas de busca/edição no core.
- [pendente] Git adapter seguro, serializado e com fronteira local/remota.
- [pendente] Shell com encerramento de descendentes e broker sandbox efetivo.
- [pendente] Plan agregado persistente com dependências e aprovação por digest.
- [pendente] MemoryRecord com proveniência, escopo, revisão e deleção.
- [pendente] Dispatcher de subagentes com escopo/orçamento derivados.
- [pendente] Navegação multi-hop limitada e protegida contra SSRF.

## Gatilhos comuns

- [ ] Contexto de identidade autenticado pelo host, versão, escopo, expiração e
      digest validado; nenhum segredo entra no prompt.
- [ ] Policy é revalidada antes de efeito, retomada, batch e delegação.
- [parcial] `authorize_tool_dispatch_with_identity` nega capabilities sem grant
  e vincula actor/scope/policy digest; o host ainda precisa autenticar o issuer
  e revalidar imediatamente antes de cada adapter/efeito.
- [ ] Tool tem schemas input/output, capability, risco, timeout, limite,
      cancellation, retry/idempotência, redaction e evidence definidos.
- [ ] Ledger confirma intenção/resultado antes de liberar ação/resultado; falha
      incerta fica reconciliável, sem retry cego.
- [ ] Rust, Python e TypeScript compartilham fixtures e erros sem duplicar regra.

## Filesystem e Git

- [ ] Path traversal, symlink escape, roots múltiplas, links quebrados, arquivo
      grande, binário, concorrência, cancelamento e atomic write cobertos.
- [ ] Read/list/search/diff/write/patch/delete requerem grants separados.
- [ ] Git serializa por repo, usa snapshot/hash no staging e trata repo/worktree
      selecionado sem misturar identidade ou credenciais.
- [ ] Operação local e remota são capacidades distintas; push/rebase/merge e
      resultado de rede incerto têm policy e recovery explícitos.
- [ ] Prompt de credencial está desabilitado; token/URL userinfo nunca aparece
      em args, erros, eventos ou logs.

## Shell e sandbox

- [ ] Execução argv não interpola shell sem capability específica.
- [ ] Timeout/cancelamento mata e aguarda a árvore de processos.
- [ ] Ambiente, filesystem, processos, rede/egress e recursos são limitados e
      reportados por estado efetivo, backend e lacunas.
- [ ] Testes demonstram deny-by-default; soft/unavailable nunca são apresentados
      como isolamento efetivo.

## Plano, memória e subagentes

- [ ] Etapas e dependências persistem; mudança de alvo, conteúdo ou policy
      invalida aprovação anterior.
- [ ] Resume/replay não duplica efeitos e registra `recovery_required` quando
      necessário.
- [ ] Memória tem fonte, digest/proveniência, scope, sensibilidade, validade,
      revisão, deleção, autorização e auditoria.
- [ ] Conteúdo lembrado/recuperado não altera policy ou identidade confiável.
- [ ] Subagentes têm parent IDs, allowlist derivada, budget/deadline, cancelamento
      e saída tipada/limitada.

## Navegação profunda

- [ ] Busca/fetch usam adapters do host com timeout, limites e egress autorizado.
- [ ] Cada redirect e cada link submetem URL/DNS/IP/porta novamente a SSRF guard.
- [ ] Profundidade/páginas/bytes/duração/concurrency/domínios limitados e
      configuráveis; dedupe/canonicalização estáveis.
- [ ] Fontes e digests acompanham cada resultado; conteúdo externo é não
      confiável e falhas parciais são explícitas.

## Gate de entrega

- [ ] Testes de Rust, Python e Node e cenários de integração relevantes passaram.
- [ ] Restart/replay/concorrência e testes negativos de segurança passaram.
- [ ] Plano, análise e spec registram SHA/comandos/resultado e lacunas atuais.
- [ ] Perfis de pelo menos dois hosts passaram sem código de produto na Engine.
