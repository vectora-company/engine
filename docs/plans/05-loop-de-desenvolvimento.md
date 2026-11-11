# Plano do loop de desenvolvimento

Este plano define como conduzir trabalho contínuo na Engine sem perder contexto,
contratos ou critérios de conclusão. Use junto com o [checklist de
implementação](../checklists/00-checklist-de-implementacao.md) e a [análise e
referências](../analyses/00-analise-e-referencias-de-desenvolvimento.md). Para
iniciar um goal com instruções compactas, use o [prompt curto](00-prompt-loop-goal.md).
Para criar os artefatos de uma etapa, copie os [templates de plano, análise e
checklist](../templates/README.md).

O pacote operacional atualizado do goal está indexado em
[`docs/goal-loop/README.md`](../goal-loop/README.md); consulte-o para o checklist
do loop e os critérios sucintos de encerramento.

## Fonte de verdade

- `AGENTS.md`: regras do repositório e checks obrigatórios.
- `docs/specs/`: escopo e contratos propostos para cada capability; o índice em
  [`docs/specs/README.md`](../specs/README.md) aponta as referências locais.
- `docs/plans/`: ordem de execução, dependências, gates e estado das etapas.
- `docs/checklists/`: critérios verificáveis de preparação, implementação e
  entrega.
- `docs/analyses/`: investigação, fontes, evidências e decisões.
- `docs/templates/`: modelos para criar esses artefatos por capability sem
  deixar escopo, evidência ou critérios de saída implícitos.
- `../issues/`: requisitos originais locais quando disponíveis; não substituem
  as specs da Engine.
- Código e testes atuais: evidência do comportamento implementado, nunca
  substituídos por texto histórico.

## Início de cada sessão

1. Confirmar branch/base e working tree.
2. Ler `AGENTS.md`, este plano, a análise, o checklist e a spec/plano ativos.
3. Revalidar no código e nos testes as afirmações documentadas; marcar findings
   antigos como resolvidos, atuais ou não reproduzidos.
4. Selecionar a menor etapa pendente e aprovada no plano ativo.
5. Se não existir etapa, spec, análise ou checklist suficiente, criar os
   documentos aplicáveis a partir dos templates antes de uma implementação
   ampla. Um artefato geral não substitui o checklist específico da etapa.

## Loop por etapa

1. Atualizar objetivo, escopo, consumidores, critérios de aceite e dependências
   no plano/checklist.
2. Resolver contratos e questões de segurança, dados, compatibilidade,
   cancelamento, concorrência e observabilidade conforme a análise.
3. Implementar incrementalmente, core Rust primeiro e bindings correspondentes
   depois.
4. Verificar conforme o checklist; corrigir regressões atribuíveis à etapa.
5. Revisar diff, documentar evidências e atualizar o estado real do plano.
6. Commitar e abrir/atualizar o PR segundo o pedido do mantenedor. Criar a
   próxima branch empilhada somente quando uma nova etapa for autorizada e a
   base correta estiver confirmada.

Não avance para trabalho fora do plano ativo, não faça alterações paralelas em
consumidores sem escopo e não declare paridade ou performance sem medições
reproduzíveis.

## Critérios para encerrar o loop

Encerre o loop quando ocorrer uma destas condições:

### Plano ativo concluído

Todos os critérios de aceite e itens obrigatórios do plano ativo estão feitos,
verificados e documentados. Informe os resultados, limitações e estado dos PRs.

### Decisão vital aguardando `brunosrz`

Uma decisão muda contrato público, arquitetura, segurança, dados, compatibilidade,
escopo ou comportamento externo e não pode ser inferida com segurança. Registre
em `docs/analyses/` a pergunta, opções, impactos e trabalho bloqueado; pergunte a
`brunosrz` e pare. Retome somente após a resposta.

### Bloqueio externo sem avanço independente

Uma credencial, serviço, dependência ou ambiente externo impede o trabalho
restante e não há etapa independente útil. Registre o bloqueio, evidências e
condição para retomada; pare até a condição mudar.

Não encerre por conveniência ou falta de contexto: reconstrua o contexto pelos
arquivos acima. Não continue após um critério de encerramento até receber nova
orientação ou a decisão necessária.
