# Goal loop contínuo da Engine

Desenvolva a Vectora Engine fase a fase até cumprir a EPIC da primeira release
completa. A EPIC e suas fases vivem em
[`docs/plans/03-plano-continuidade.md`](03-plano-continuidade.md); organize o
trabalho por capacidades, dependências e critérios de aceite, nunca por número
ou posição de PR.

Siga [`AGENTS.md`](../../AGENTS.md), o processo em
[`docs/plans/05-loop-de-desenvolvimento.md`](05-loop-de-desenvolvimento.md),
[`docs/checklists/00-checklist-de-implementacao.md`](../checklists/00-checklist-de-implementacao.md)
e [`docs/analyses/00-analise-e-referencias-de-desenvolvimento.md`](../analyses/00-analise-e-referencias-de-desenvolvimento.md).
Para a próxima capability incompleta, leia também sua spec, plano, checklist e
análise específicos, indexados em [`docs/README.md`](../README.md) e
[`docs/goal-loop/README.md`](../goal-loop/README.md).

No início de cada sessão, confirme checkout, branch, alterações locais e
remote. Preserve mudanças locais alheias ao escopo. Ao entregar cada etapa,
siga o fluxo vigente em `AGENTS.md` e determine naquele momento a branch e a
base apropriadas; estado de GitHub não pertence a este prompt nem ao roadmap.

Implemente incrementalmente, começando pelos contratos e pelo core Rust; exponha
os bindings e consumidores necessários conforme a capability. Verifique cada
etapa, atualize o plano, checklist, análise, spec e demais referências afetadas
com o estado observado, e registre lacunas sem converter planejamento em
capacidade implementada. Organize documentação por domínio e capability; PRs
são apenas evidência de entrega no histórico.

Commit, push e abertura do PR empilhado são obrigatórios. Comece a etapa
seguinte sem esperar CI ou aprovação do PR anterior. Não aprove PRs nem crie
issues sem pedido. Pergunte decisões vitais a brunosrz neste chat, nunca o
mencione para decidir em GitHub.

Continue enquanto houver trabalho independente no plano. Encerre somente
quando a EPIC estiver concluída, quando uma decisão vital bloquear todo avanço
independente ou quando um bloqueio externo impedir qualquer trabalho útil.
Registre o motivo nos documentos de estado e, para decisão vital, pergunte no
chat e aguarde a resposta.
