# Checklist do goal loop

Use este checklist em cada etapa. O checklist específico da capability pode
acrescentar requisitos, mas não deve remover verificações aplicáveis daqui.

Estados permitidos: `pendente`, `feito`, `bloqueado` e `não aplicável`. Todo
item bloqueado ou não aplicável precisa de uma justificativa curta.

## Preparação

- [ ] Confirmar repo, branch, base da stack e working tree; listar os PRs
      abertos e usar o PR aberto como base. Em checkout compartilhado, conferir
      a branch atual antes de agir.
- [ ] Storage: conferir `podman ps -a`, iniciar os contêineres de teste e
      exportar as variáveis `VECTORA_TEST_*` necessárias.
- [ ] Storage: consultar a matriz de equivalência do plano de storage; a
      capability vale para os dois lados do par ou tem motivo de
      incompatibilidade registrado.
- [ ] Ler `AGENTS.md`, plano ativo, spec, análise e checklist específicos.
- [ ] Identificar consumidores, bindings e contratos afetados.
- [ ] Para capabilities destinadas ao GH Bot, revisar
      `../gh-bot/docs/engine-integration.md` e validar o binding TypeScript
      local em `../engine/bindings/typescript`; não exigir pacote npm publicado.
- [ ] Isolar a validação experimental do GH Bot em conta GitHub nova e
      repositórios de teste, sem credenciais ou repositórios de produção.
- [ ] Descrever objetivo, comportamento esperado e exclusões.
- [ ] Revalidar implementação e testes existentes; separar fatos de hipóteses.
- [ ] Registrar dependências e decisões pendentes.
- [ ] Interromper o trabalho dependente se houver decisão vital pendente.

## Contratos e desenho

- [ ] Definir entradas, saídas, invariantes, erros e versão do contrato.
- [ ] Definir timeout, retry, cancelamento, concorrência e idempotência.
- [ ] Definir ownership, fronteiras de confiança, autenticação e autorização.
- [ ] Definir persistência, consistência, replay, recovery e migrações.
- [ ] Definir eventos/logs/métricas necessários sem expor dados sensíveis.
- [ ] Definir fallback, rollback e compatibilidade incremental.
- [ ] Justificar dependências novas e limitar a superfície pública.

## Implementação e cobertura

- [ ] Implementar o core Rust antes de expor a capability nos bindings.
- [ ] Manter bindings Python e TypeScript coerentes com o mesmo contrato.
- [ ] Quando a capability for consumida pelo GH Bot, exercitar a integração
      local Engine + Bot e corrigir lacunas em ambos na mesma stack, sem criar
      runtime paralelo em TypeScript.
- [ ] Preservar fallback do consumidor e evitar efeitos duplicados.
- [ ] Testar sucesso, erros, limites e compatibilidade.
- [ ] Testar cancelamento, concorrência, persistência/restart e replay quando
      fizerem parte do contrato.
- [ ] Compartilhar fixtures de conformidade entre linguagens quando aplicável.
- [ ] Atualizar documentação da API, exemplos e versões de contrato.

## Verificação e entrega local

- [ ] Executar formatadores, linters, type checks, builds e testes pertinentes
      conforme `AGENTS.md` e o checklist da capability.
- [ ] Investigar e corrigir falhas locais causadas pela etapa.
- [ ] Revisar diff, arquivos staged e ausência de segredos/artefatos incidentais.
- [ ] Registrar comandos, resultados, SHA, ambiente e limitações reproduzíveis.
- [ ] Atualizar plano, análise e checklist com estado verdadeiro.
- [ ] Para cada Markdown novo, conferir o link com caminho desde a raiz em
      `docs/README.md`, `docs/plans/00-prompt-loop-goal.md`, nos documentos do
      goal que o usam e nos artefatos da capability que dependem dele.
- [ ] Rodar o Prettier nos Markdown alterados e repetir o `git add` se o hook
      reformatar.
- [ ] Commitar e fazer push para a URL do GitHub (o `origin` local pode apontar
      para um clone).
- [ ] Abrir o PR com base na branch do PR anterior da stack (não `main`).
- [ ] Antes de parar um contêiner de teste, conferir que não tem `AutoRemove`.
- [ ] Confirmar que o próximo passo está no plano ativo aprovado; abrir sua
      branch empilhada imediatamente, sem esperar CI ou aprovação do PR anterior.

## Saída de etapa

Uma etapa está concluída quando todos os critérios de aceite aplicáveis têm
implementação ou justificativa explícita, verificação local registrada,
documentação sincronizada e nenhuma decisão vital pendente para aquela etapa.
Isso não encerra o goal se restarem etapas aprovadas no plano.
