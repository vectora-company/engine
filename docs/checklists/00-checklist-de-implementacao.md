# Checklist de implementação

Use este checklist como modelo para cada etapa de plano em execução. Copie os
itens aplicáveis para o plano ou checklist específico e mantenha o estado
atualizado durante o trabalho. Não marque um item como feito sem evidência.

Estados permitidos: `pendente`, `feito`, `bloqueado` e `não aplicável`.

O estado verificado em 2026-11-13 inclui os PRs #92 e #93 abertos e empilhados sobre #91, além do PR #94 em implementação sobre #93. A decisão de brunosrz no chat combina `pg_dump`/backup gerenciado com verify/rebuild da projeção a partir do SQLite; ver evidências e limites na análise. Revisões documentais anteriores são registros históricos. Este checklist alinha o plano à meta de uma primeira release completa. Para implementação de mídia, use também
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

## Reclassificar MIME pelos bytes reidratados — filha do PR #87

- [x] Classificar os bytes verificados e usar o MIME detectado no payload quando a modalidade coincide.
- [x] Rebaixar para texto/referência quando os bytes não correspondem à modalidade declarada; aceitar documento nativo somente quando a assinatura identifica PDF.
- [x] Validar previews reidratados como imagens antes de serializar.
- [x] Adicionar testes para MIME de imagem incorreto, MIME genérico em PDF e divergência entre modalidade declarada e bytes; 22 testes `chat::media::tests` passaram.
- [x] Rodar hooks completos: format, Prettier, Clippy, testes Rust workspace e rustdoc passaram; os 22 testes focados passaram.
- [x] Commit `18c2eca`, push e abrir o [PR #88](https://github.com/vectora-company/engine/pull/88), filho do #87.

## Fronteira de bytes inline — implementação da decisão 3

- [x] Receber no chat a decisão de separar request persistível de envio efêmero.
- [x] Rejeitar `Inline` e `data:` ao desserializar/serializar request persistível.
- [x] Validar envelope efêmero, base64, assinatura/tipo, MIME e limite configurável (20 MiB padrão).
- [x] Expor complete/stream efêmeros em Python e TypeScript para OpenAI-compatible, nativo e fallback.
- [x] Preservar preparo manual, contexto por chamada e contexto padrão.
- [x] Cobrir limites e fronteira em testes Rust e bindings.
- [x] Verificar workspace Rust (171 testes + 3 fixtures e Clippy), Python (52 testes, Ruff/format/ty), TypeScript (build/typecheck; 38 passaram, 10 de serviço sem endpoint foram ignorados) e diff; registrar no PR [#90](https://github.com/vectora-company/engine/pull/90), filho de #89; commit de implementação `d56e864286153b6bee1f8cc67e1ffda4c1ba2afb`.

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

## Diagnóstico da outbox nos bindings — PR #91

- [x] Expor flush da outbox e contagem pendente em Python e TypeScript; manter SQLite como autoridade local e PostgreSQL como projeção retomável.
- [x] Validar limite 1–1000 inclusive quando PostgreSQL não está configurado; flush sem URL retorna zero sem remover pendências.
- [x] Testar contrato offline nos bindings Python e Node e limite no core.
- [x] Rodar workspace Rust (172 testes + 3 fixtures), Python (53), Node (39 aprovados; 10 testes service-real ignorados sem endpoints), Clippy, rustdoc e hooks completos.
- [x] Commit `4f7e980`, push e abrir o PR [#91](https://github.com/vectora-company/engine/pull/91), filho do #90.
- [ ] Criar fixtures comuns de KV, filas e vetores nos três níveis; trabalho continua na Fase 2 do plano.

## Projeção PostgreSQL de checkpoints — PR #92

- [x] Evoluir schema do outbox SQLite para armazenar checkpoint junto ao evento (schema v6; migração aditiva).
- [x] Tornar a projeção opt-in por arquivo: `ResilientLedger` com PostgreSQL habilita-a e faz backfill histórico; stores SQLite-only não acumulam outbox. Admission e stores lifecycle/loop/policy enfileiram eventos/checkpoints na transação local quando habilitados; itens confirmados são removidos da outbox.
- [x] Projetar evento + checkpoint atomicamente e de forma idempotente no PostgreSQL.
- [x] Repetir workspace Rust e matriz de serviços após corrigir a fixture de migração: 175 Rust + 3 fixtures + 1 teste nativo; `storage::` 76/76; 34 testes SQLite focados; PostgreSQL 18.6, Redis 8.10.3, NATS 2.15.3 e Qdrant 1.18.0 reais. Format, Clippy `-D warnings` e rustdoc passaram. Python 53 e Node 39 (10 sem endpoints ignorados) são da rodada anterior; bindings não mudaram.
- [x] Commit `b01b20b`, push e abrir o PR [#92](https://github.com/vectora-company/engine/pull/92), filho do #91.
- [x] Publicar a correção da fixture de migração e a projeção opt-in no mesmo PR #92 (commit `69af992`).
- [x] Projetar reservas/admissions como entidade consultável e criar APIs de leitura PostgreSQL no PR [#93](https://github.com/vectora-company/engine/pull/93).
- [x] Registrar a decisão de backup gerenciado/`pg_dump` e verify/rebuild da projeção a partir do SQLite; implementação no PR #94 em andamento.
- [x] Verificar reconstrução service-real PostgreSQL após apagar a scope isolada; eventos, checkpoint, reservations/admissions e reserva standalone restaurados com contagens iguais.
- [x] Binding Python: `maturin develop` e `pytest` local explícito — 53 passaram.
- [x] Binding TypeScript: `pnpm typecheck`, `pnpm build` e `pnpm test` — 39 passaram, 10 ignorados sem endpoints.
- [x] Simular perda de eventos, checkpoints, reservas/admissions e reserva standalone na scope de teste.
- [ ] Cobrir conteúdo extra/incompatível e indisponibilidade remota durante a reconstrução.
- [ ] Reexecutar a suíte NATS com servidor 2.11+; o contêiner atual é 2.10 e falhou em 3 testes que exigem TTL de KV.
- [ ] Em seguida tratar concorrência de projetores e migrations PostgreSQL com rollback.

## Projeção de reservas e admissions — PR #93, filho do PR #92

- [x] Evoluir SQLite ao schema v8 com outbox para reservation standalone, backfill único e vínculo da reserva ao evento admission.
- [x] Projetar reservation/admission/event/checkpoint atomicamente no PostgreSQL e expor leitura de reserva e admission.
- [x] Testes SQLite cobrem migração v1, opt-in/backfill, replay e reserva standalone; testes service-real cobrem reservation/admission projetados e APIs de leitura (PostgreSQL 18.6).
- [x] Workspace Rust 182 + 3 fixtures + 1 teste nativo; `storage::` 83/83 com PostgreSQL 18.6, Redis 8.10.3, NATS 2.15.3 e Qdrant 1.18.0.
- [x] Clippy `-D warnings`, rustdoc, formatação e hooks completos passaram.
- [x] Commit `58e1619`, push e abrir o PR [#93](https://github.com/vectora-company/engine/pull/93), filho do #92.
