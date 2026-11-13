# Checklist de implementação

Use este checklist como modelo para cada etapa de plano em execução. Copie os
itens aplicáveis para o plano ou checklist específico e mantenha o estado
atualizado durante o trabalho. Não marque um item como feito sem evidência.

Estados permitidos: `pendente`, `feito`, `bloqueado` e `não aplicável`.

Esta revisão documental está empilhada sobre o PR #82 e alinha o plano à meta
de uma primeira release completa. Para implementação de mídia, use também
[`docs/checklists/04-midia-no-chat.md`](04-midia-no-chat.md); este arquivo
permanece geral e não substitui critérios específicos. A branch ativa local `feat/chat-media-provider-limits-docs83` é filha do PR #83 e está implementando limites explícitos por provider/modelo/modalidade.

## Revisão documental completa — `docs/revisao-completa-markdown`

- [x] Inventariar os 55 arquivos Markdown versionados e distinguir documentos
      vigentes de snapshots históricos.
- [x] Revalidar a branch e a stack no GitHub: PR #82 aberto no topo; esta branch
      é sua filha.
- [x] Corrigir referências contraditórias sobre payloads de mídia, bindings,
      estado de storage, PRs abertos e objetivo da primeira release.
- [x] Verificar todos os links Markdown locais; todos os destinos existem.
- [x] Rodar Prettier em todos os Markdown; a segunda execução passou.
- [x] Executar `git diff --check`.
- [x] Revisar o diff final; commits documentais `00e2fe9` e `4c9a329`, push e abrir o [PR #83](https://github.com/vectora-company/engine/pull/83), empilhado sobre `feat/chat-media-bindings` (PR #82).

## Limites de mídia por provider/modelo — etapa seguinte ao PR #83

- [x] Adicionar limites explícitos por modalidade e par provider/modelo no core Rust; teto global continua como máximo absoluto por asset.
- [x] Encaminhar configuração equivalente em Python e TypeScript.
- [x] Verificar 18 testes `chat::media::tests`; teste de binding Python (1) e Node (1); build maturin e `pnpm build`.
- [ ] Registrar limites oficiais por endpoint/protocolo/modelo com fonte e data, sem inferir um teto universal por marca de provider.
- [ ] Avaliar limites totais do payload, contagem de partes, duração/resolução e consistência com gateways OpenAI-compatible.
- [ ] Executar verificações finais, atualizar análise/spec, commit, push e abrir PR empilhado sobre o PR #83.

## Preparação e escopo

- [ ] Confirmar repositório, branch, base da stack e estado da working tree.
- [ ] Listar PRs abertos; tratar o PR do topo como trabalho ativo e confirmar
      a branch atual, pois o checkout é compartilhado.
- [ ] Para storage, inspecionar `podman ps -a` e usar os serviços locais reais;
      registrar as versões efetivamente executadas.
- [ ] Ler `AGENTS.md`, instruções locais e [análise/referências](../analyses/00-analise-e-referencias-de-desenvolvimento.md).
- [ ] Identificar spec, issue local de origem e plano ativo.
- [ ] Registrar objetivo, comportamento esperado, exemplos e fora de escopo.
- [ ] Inspecionar implementação e testes existentes; separar fatos, hipóteses e
      lacunas.
- [ ] Identificar consumidores e versões dos bindings afetados.
- [ ] Registrar decisões pendentes; interromper somente o trabalho que depende
      de uma decisão vital e continuar etapas independentes aprovadas.
- [ ] Para storage, consultar a matriz por pares: implementar a base viável nos
      dois adapters; registrar como especialização apenas incompatibilidade
      técnica comprovada.

## Contratos e desenho

- [ ] Definir tipos, invariantes, erros e versão do contrato.
- [ ] Definir timeout, retry, cancelamento, concorrência e idempotência.
- [ ] Definir ownership, autenticação, autorização e fronteiras de confiança.
- [ ] Definir persistência, consistência, replay, recovery e migrações, quando
      aplicável.
- [ ] Definir observabilidade sem exposição de credenciais ou dados sensíveis.
- [ ] Definir fallback, rollback e compatibilidade com o comportamento existente.
- [ ] Justificar dependências novas e manter pequena a superfície pública.

## Implementação

- [ ] Implementar primeiro o contrato no core Rust.
- [ ] Atualizar bindings Python e TypeScript/Node para expor somente capacidades
      implementadas pelo core.
- [ ] Preservar comportamento do consumidor e caminhos anteriores até o gate de
      adoção, quando aplicável.
- [ ] Adicionar testes de sucesso, erro, limites, concorrência, cancelamento,
      persistência/restart e replay, conforme o contrato.
- [ ] Adicionar testes de conformidade compartilhados entre bindings quando
      aplicável.
- [ ] Atualizar exemplos, stubs, documentação de API e versões de contrato.
- [ ] Atualizar o checklist e o plano sem declarar paridade ou maturidade sem
      evidência.

## Verificação e entrega

- [ ] Rodar `cargo fmt --all -- --check`.
- [ ] Rodar `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`.
- [ ] Rodar testes Rust adequados ao escopo.
- [ ] Rodar checks Python e TypeScript para bindings alterados.
- [ ] Rodar checks compartilhados relevantes, conforme `AGENTS.md` e CI.
- [ ] Examinar cada falha e corrigir as regressões causadas pela mudança.
- [ ] Revisar o diff e confirmar que não há segredos, artefatos ou mudanças
      incidentais.
- [ ] Registrar SHA, comandos, resultados e limitações reproduzíveis.
- [ ] Atualizar o plano com itens restantes, riscos e próximo passo.
- [ ] Para cada Markdown novo, conferir o link com caminho desde a raiz em
      `docs/README.md`, `docs/plans/00-prompt-loop-goal.md`, nos documentos do
      goal que o usam e nos artefatos da capability que dependem dele.
- [ ] Executar Prettier nos Markdown alterados; reaplicar `git add` se formatar.
- [ ] Fazer commit e push para o remoto GitHub; o `origin` local pode ser um
      clone auxiliar.
- [ ] Abrir PR com base na branch do PR anterior da stack e conferir a base no
      GitHub.
- [ ] Iniciar a próxima etapa em nova branch empilhada sem esperar aprovação ou
      CI. Aprovar PRs e criar issues somente mediante pedido explícito.
- [ ] Antes de parar contêiner de teste, verificar `AutoRemove`.

## Critério de conclusão da etapa

A etapa só está concluída quando os critérios de aceite correspondentes estão
implementados, os checks pertinentes passaram ou suas limitações foram
explicitamente registradas, a documentação reflete o código e não há uma decisão
vital pendente. Concluir uma etapa não encerra automaticamente o loop se houver
outras etapas aprovadas no plano ativo.
