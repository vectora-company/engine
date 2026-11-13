# Checklist de implementação

Use este checklist como modelo para cada etapa de plano em execução. Copie os
itens aplicáveis para o plano ou checklist específico e mantenha o estado
atualizado durante o trabalho. Não marque um item como feito sem evidência.

Estados permitidos: `pendente`, `feito`, `bloqueado` e `não aplicável`.

O estado atual, verificado em 2026-11-13, é PR #86 aberto no topo da stack; esta revisão Markdown está sendo preparada como filha dele. A revisão documental anterior (#83) continua como etapa histórica. Este checklist alinha o plano à meta de uma primeira release completa. Para implementação de mídia, use também
[`docs/checklists/04-midia-no-chat.md`](04-midia-no-chat.md); este arquivo
permanece geral e não substitui critérios específicos. A etapa de limites explícitos por provider/modelo/modalidade foi publicada no PR [#84](https://github.com/vectora-company/engine/pull/84), filho do #83 (head local/remoto `58980b3`). O retry de incompatibilidade multimodal foi publicado no [PR #85](https://github.com/vectora-company/engine/pull/85), filho do #84.

## Revisão documental anterior — PR #83

- [x] Inventariar os 55 arquivos Markdown versionados e distinguir documentos
      vigentes de snapshots históricos.
- [x] Revalidar a branch e stack daquela etapa: PR #82 estava no topo.
- [x] Corrigir referências contraditórias sobre payloads de mídia, bindings,
      estado de storage, PRs abertos e objetivo da primeira release.
- [x] Verificar todos os links Markdown locais; todos os destinos existem.
- [x] Rodar Prettier em todos os Markdown; a segunda execução passou.
- [x] Executar `git diff --check`.
- [x] Revisar o diff final; commits `00e2fe9`, `4c9a329` e `9db6d45`; abrir o [PR #83](https://github.com/vectora-company/engine/pull/83) sobre `feat/chat-media-bindings` (#82).

## Limites de mídia por provider/modelo — PR #84, filho do #83

- [x] Adicionar limites explícitos por modalidade e par provider/modelo no core Rust; teto global continua como máximo absoluto por asset.
- [x] Encaminhar configuração equivalente em Python e TypeScript.
- [x] Verificar 18 testes `chat::media::tests`; teste de binding Python (1) e Node (1); build maturin e `pnpm build`.
- [ ] Registrar limites oficiais por endpoint/protocolo/modelo com fonte e data, sem inferir um teto universal por marca de provider.
- [ ] Avaliar limites totais do payload, contagem de partes, duração/resolução e consistência com gateways OpenAI-compatible.
- [x] Atualizar plano, spec, análise e checklist; executar verificações; commit `d4ccd3d`, push e abrir o [PR #84](https://github.com/vectora-company/engine/pull/84) empilhado sobre o PR #83.

## Retry textual para rejeição multimodal — PR #85, filho do #84

- [x] Classificar incompatibilidades multimodais HTTP 400 sem reclassificar autenticação.
- [x] Repetir uma vez como texto no cliente OpenAI-compatible e no cliente nativo, inclusive ao abrir stream antes de devolvê-lo.
- [x] Aplicar a mesma regra ao wrapper fallback antes de trocar de candidato.
- [x] Testar classificação e retry complete/stream no cliente OpenAI-compatible.
- [x] Cobrir o wrapper fallback; cliente direto cobre que o retry de stream ocorre antes da abertura e preserva o evento de evidência.
- [x] Rodar Clippy, format, suíte Rust workspace, rustdoc, Prettier e hooks de pre-commit; revisar docs/diff.
- [x] Commit `0767636`, push e abrir o [PR #85](https://github.com/vectora-company/engine/pull/85), empilhado sobre `feat/chat-media-provider-limits-docs83` (#84).
- [ ] Aguardar review/CI sem bloquear o início da próxima etapa; PRs continuam sem aprovação pelo agente.
- [x] Registrar decisão de brunosrz: substituir diretamente `ToolResult.output: String` por partes tipadas, sem compatibilidade v1.

## Revisão documental ampla — filha do PR #86

- [x] Inventariar os 55 Markdown versionados; separar afirmações atuais de snapshots históricos.
- [x] Conferir a stack no GitHub: PR #86 aberto com head `b188a94`; branch desta revisão criada como sua filha.
- [x] Procurar versões de contratos obsoletas e atualizar loop v3 / `ToolResult` v2 nos documentos de runtime e verificação.
- [x] Corrigir estado da mídia, stack de PRs e verificações locais nos índices, plano, análise, checklist, spec e sabatina.
- [x] Verificar destinos relativos de links nos 55 Markdown versionados: 0 quebrados.
- [x] Rodar Prettier, hooks aplicáveis e `git diff --check`; links relativos válidos nos 55 Markdown.
- [x] Commit `4ab3df3`, push e abrir o [PR #87](https://github.com/vectora-company/engine/pull/87), filho do #86 (head `4ab3df3`).

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

## Partes multimodais em `ToolResult` — PR #86, filho do #85

- [x] Substituir a string por `parts` tipadas e rejeitar formatos/snapshots legados.
- [x] Incrementar as versões do resultado e do loop; expor as versões aos bindings.
- [x] Converter resultado em mensagem de tool por helper no núcleo Rust, compartilhado pelos bindings.
- [x] Serializar imagens de resultado em Anthropic, Gemini e OpenAI Chat Completions; manter fallback textual de Cohere.
- [x] Cobrar texto e metadados serializados nos limites do loop; limitar bytes ao reidratar no contexto de mídia.
- [x] Rodar suíte Rust workspace (163 testes + 3 fixtures), Clippy, `cargo check` de todos os targets, rustdoc, testes Python, build/typecheck e 37 testes Node (10 testes de serviços ignorados sem URLs).
- [x] Rodar todos os hooks de pre-commit, corrigir stubs Python ausentes e revisar diffs.
- [x] Abrir PR #86 sobre o #85: https://github.com/vectora-company/engine/pull/86 (head `b188a94`).
