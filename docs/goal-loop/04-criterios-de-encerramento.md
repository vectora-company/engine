# Critérios de encerramento do goal loop

O loop continua pela próxima etapa aprovada e desbloqueada. Encerre-o somente
quando ocorrer uma das condições abaixo.

## 1. Plano ativo concluído

Todos os critérios de aceite e entregas obrigatórias do plano ativo foram
implementados, verificados localmente e documentados. Atualize o plano e os
checklists, informe limitações e resultados, e encerre o goal. Confira também
os links dos Markdown criados durante o ciclo em
`docs/plans/00-prompt-loop-goal.md` e nos documentos do goal que os usam.

## 2. Decisão vital aguardando `brunosrz`

Uma escolha não inferível altera materialmente contrato público, arquitetura,
segurança, dados, compatibilidade, dependências obrigatórias, escopo ou
comportamento externo. Antes de parar:

1. documente a pergunta em `docs/analyses/`, com opções, consequências e etapa
   bloqueada;
2. complete todo trabalho independente da decisão;
3. faça uma pergunta objetiva a `brunosrz` no chat (não em issue nem em
   comentário de PR, sem marcá-lo no GitHub) e aguarde a resposta;
4. não implemente a opção presumida nem avance em trabalho dependente.

## 3. Bloqueio externo sem trabalho independente

Uma credencial, serviço, dependência ou condição do ambiente impede o trabalho
restante, e nenhuma etapa independente aprovada pode avançar. Registre a
evidência observada e a condição específica de retomada; então encerre a sessão
até essa condição mudar.

## Não são condições de encerramento

- contexto insuficiente: reconstrua-o pelos documentos e pelo código;
- uma etapa acabou, mas outras etapas aprovadas continuam pendentes;
- o trabalho é demorado ou exige corrigir falhas locais;
- checks remotos de CI estão executando ou falharam: este loop não os acompanha;
- o PR anterior da stack aguarda aprovação: abra o próximo PR empilhado sobre
  ele e continue;
- falta de conveniência ou de uma nova instrução quando o próximo passo já está
  definido e aprovado.
