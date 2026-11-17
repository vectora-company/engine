# Checklist geral de implementação

Este arquivo define gates reutilizáveis por etapa; não é um diário de PRs nem
um inventário cronológico. O estado das capabilities está no plano da EPIC e no inventário transversal,
[`docs/plans/03-plano-continuidade.md`](../plans/03-plano-continuidade.md), e
nos checklists específicos. O checklist transversal é
[`docs/checklists/07-checklist-completo-da-engine.md`](07-checklist-completo-da-engine.md). Confirme a stack ao vivo no GitHub em cada sessão.

Use os seguintes checklists por domínio:

- Mídia no chat: [`docs/checklists/04-midia-no-chat.md`](04-midia-no-chat.md).
- Providers e modalidades: [`docs/checklists/05-providers-e-modalidades-llama-cpp-v1.md`](05-providers-e-modalidades-llama-cpp-v1.md).
- Storage de grafos: [`docs/checklists/06-storage-grafos-falkordb.md`](06-storage-grafos-falkordb.md).
- Inventário transversal: [`docs/checklists/07-checklist-completo-da-engine.md`](07-checklist-completo-da-engine.md).
- CLI: [`docs/checklists/08-engine-cli.md`](08-engine-cli.md).

Estados permitidos: `pendente`, `feito`, `bloqueado` e `não aplicável`. Não
marque um item como feito sem evidência local identificável.

## Preparação e escopo

Estado da etapa: determine a capability e a base da stack no início de cada
sessão. O escopo, os testes e os limites estão registrados no
[plano de providers](../plans/06-plano-provedores-e-modalidades-llama-cpp-v1.md),
[spec](../specs/18-providers-e-modalidades-llama-cpp-v1.md),
[checklist da capability](05-providers-e-modalidades-llama-cpp-v1.md) e
[auditoria](../analyses/06-auditoria-providers-modalidades.md), ou nos
artefatos do domínio correspondente. Consulte GitHub para a posição atual da
stack e prossiga sem aguardar CI dos PRs anteriores.

- [ ] Confirmar repositório, branch, base da stack e estado da working tree.
- [ ] Listar PRs abertos e suas bases; confirmar qual branch da stack está
      ativa e se corresponde ao checkout compartilhado.
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
- [x] NACK agendado, enqueue idempotente e replay read-only paginado por cursor
      estão implementados no core e expostos pelas bindings Python e TypeScript;
      testes reais dos dois brokers confirmam paginação sem alterar consumers. O estado por domínio está em
      `docs/checklists/03-auditoria-storage-s0-s5.md` e as evidências em
      `docs/analyses/05-auditoria-storage-s0-s5.md`.
- [x] Pub/sub Redis/NATS oferece assinatura por padrão com semântica comum
      (`*` para um segmento, `>` terminal para vários); os testes reais Rust,
      Python e Node confirmam filtragem e publicação entregue antes do retorno.
      Reconexão observável permanece pendente.
- [x] LanceDB e Qdrant oferecem filtros de metadados escalares equivalentes
      antes do top-k, expostos em Python e TypeScript. O contrato vetorial 3
      registra a incompatibilidade com coleções LanceDB legadas em `Utf8`.
- [x] Chat multimodal aceita limites agregados opcionais de bytes/assets no
      asset map e em requests efêmeros inline, compartilhados pelo turno e
      validados antes da leitura/envio; testes direcionados em Rust, Python e
      TypeScript. O checklist específico registra o estado por operação.
- [x] Decision Model efêmero limita bytes totais e quantidade de arquivos; as
      entradas multimodais de embeddings e rerank no llama-server compartilham
      orçamento entre todos os inputs. Rust/Python/TypeScript cobertos; detalhes
      e lacunas no checklist de providers.
- [x] Resolução de mídia usa override do consumidor > metadata específica do
      modelo/gateway > perfil genérico > desconhecido. Metadata de `/v1/models`
      pode ser convertida explicitamente nos bindings; discovery/cache
      automático permanece pendente.
- [x] OpenAI Responses serializa input audio WAV/MP3; capability fica
      desconhecida sem metadata/override por modelo nos perfis OpenAI. Fixtures
      verificam formatos aceitos e rejeição de outros. Ver os checklists de
      mídia e providers para os limites por modelo e smoke real pendente.
- [x] Gating da etapa multimodal local: `scons lint` e `scons tests` passaram;
      contagens e limitações específicas estão em
      `docs/analyses/06-auditoria-providers-modalidades.md`.
- [x] Testes Python e Node exercitam o asset map efêmero no OpenAI Responses;
      a extensão Python foi rebuildada no checkout antes do teste para evitar
      validar um binário local antigo.

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

## Atualização documental

Acompanhe por capability, fase e critério de aceite. Registre evidências em
código, testes e execuções reproduzíveis; não organize planos, specs ou
checklists por PR, commit, branch ou SHA. Consulte o estado de entrega no
GitHub dinamicamente. Ao adicionar uma capability, atualize
[`docs/README.md`](../README.md),
[`docs/plans/00-prompt-loop-goal.md`](../plans/00-prompt-loop-goal.md),
[`docs/goal-loop/README.md`](../goal-loop/README.md) e os documentos gerais
que a referenciam.
