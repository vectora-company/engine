# Processo do goal loop da Engine

Este documento descreve um processo estável para desenvolver a Engine fase a
fase até concluir a EPIC da primeira release completa. O roadmap organizado por
capability fica em [`docs/plans/03-plano-continuidade.md`](03-plano-continuidade.md).
Use junto com [`docs/checklists/00-checklist-de-implementacao.md`](../checklists/00-checklist-de-implementacao.md),
[`docs/analyses/00-analise-e-referencias-de-desenvolvimento.md`](../analyses/00-analise-e-referencias-de-desenvolvimento.md)
e o índice operacional em [`docs/goal-loop/README.md`](../goal-loop/README.md).
O prompt reutilizável está em [`docs/plans/00-prompt-loop-goal.md`](00-prompt-loop-goal.md).

## Fontes de verdade

- [`AGENTS.md`](../../AGENTS.md): regras de engenharia, workspace e entrega.
- `docs/specs/`: contratos e limites de cada capability.
- `docs/plans/`: fases, dependências, critérios de aceite e estado real do
  trabalho por capability.
- `docs/checklists/`: passos e evidências verificáveis.
- `docs/analyses/`: investigação, decisões e referências externas.
- [`docs/README.md`](../README.md): índice dos artefatos específicos.
- Código e testes: comportamento implementado. Roadmaps e specs não provam
  implementação.

## Início de cada sessão

1. Confirme checkout, branch, remote e `git status`; preserve todos os arquivos
   modificados ou não rastreados que não pertencem à etapa.
2. Consulte no GitHub todos os PRs abertos, suas branches e bases atuais. A
   stack é estado mutável; descubra qual branch é a base vigente antes de criar
   a próxima, sem fixar números ou nomes no prompt e nos documentos gerais.
3. Se a etapa envolver storage real, confira os serviços locais e suas versões
   antes de interpretar testes condicionais como aprovados.
4. Leia as regras, o plano da EPIC, o checklist e a análise gerais; selecione a
   próxima capability incompleta conforme dependências. Leia também spec,
   plano, checklist e análise específicos dessa capability.
5. Confirme no código e em testes o estado descrito. Separe claramente
   implementado, parcial, pendente de revisão, planejado e não implementado.

## Ciclo de implementação

1. Defina objetivo, consumidores, limites, critérios de aceite e dependências
   no plano e checklist específicos.
2. Resolva contratos de dados, erros, timeout, cancelamento, ownership,
   concorrência, segurança e observabilidade. Decisões vitais são perguntadas
   ao usuário no chat após concluir o trabalho independente.
3. Implemente primeiro o core Rust; depois exponha os bindings e consumidores
   necessários à capability, sem duplicar regras de runtime.
4. Verifique cada camada conforme o checklist. Testes que dependem de serviços
   externos só contam como executados quando os serviços foram realmente
   configurados e alcançáveis.
5. Revise diff e working tree; atualize spec, plano, checklist, análise,
   checklist geral, análise geral, Q&As e índices afetados com evidência
   reproduzível. Documente limitações sem promovê-las a garantias.
6. Faça commit, push e abra o PR. Todo PR novo é empilhado sobre a branch do
   PR anterior ainda aberto, identificada na consulta atual. Comece o próximo
   trabalho sem aguardar aprovação ou CI; falhas do PR anterior são tratadas no
   contexto adequado enquanto a stack avança.

## Organização da documentação

Organize Markdown por domínio, fase e capability. PRs, commits, branches e SHAs
servem como evidência de entrega, não como índice primário, roadmap ou estrutura
de capítulos. O estado de PR é volátil: mantenha-o apenas nos artefatos
específicos que precisem registrar a entrega e confira GitHub ao retomar.

Ao criar Markdown, use templates em [`docs/templates/README.md`](../templates/README.md)
e, no mesmo conjunto de mudanças, vincule o caminho completo a partir da raiz
do repositório em [`docs/README.md`](../README.md), no prompt
[`docs/plans/00-prompt-loop-goal.md`](00-prompt-loop-goal.md), no índice
[`docs/goal-loop/README.md`](../goal-loop/README.md) e nos documentos gerais ou
específicos que dependam dele.

## Equivalência de storage

SQLite/PostgreSQL, LanceDB/Qdrant, Redis/NATS KV,
Redis Streams/JetStream e Redis Pub/Sub/NATS Core são pares. Tudo que ambos
conseguem fazer pertence à base equivalente, com a mesma semântica observável,
APIs comparáveis e suíte de conformidade. Especialização unilateral exige
incompatibilidade técnica verificada e registrada em
[`docs/specs/02-storage-memoria-contexto-rag.md`](../specs/02-storage-memoria-contexto-rag.md)
e `capabilities()`; lacunas de um lado não são especialização.

## Entrega e encerramento

Commit, push e PR empilhado são obrigatórios. Não aprove PR nem crie issue sem
pedido. Não encerre por conveniência, espera de CI, falta de contexto ou PR
pendente.

Encerre somente quando:

- a EPIC estiver concluída com critérios verificados e documentados;
- uma decisão vital sem inferência segura bloquear todo trabalho independente;
  registre opções, impactos e pergunta na análise, pergunte no chat e aguarde;
- um bloqueio externo impedir qualquer avanço independente; registre evidência
  e condição para retomada.

O checklist sucinto de retomada, os artefatos por domínio e os critérios
detalhados estão em [`docs/goal-loop/README.md`](../goal-loop/README.md).
