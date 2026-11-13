# Plano: [nome da capability]

**Estado:** `proposto | aprovado | em andamento | concluído | bloqueado`  
**Atualizado em:** [AAAA-MM-DD]  
**Responsável pela decisão:** `brunosrz`  
**Spec/requisitos:** [caminhos e referências locais]  
**Análise:** [caminho em docs/analyses/]  
**Checklist:** [caminho em docs/checklists/]

## Objetivo e resultado

[Descreva o problema, consumidores e resultado observável.]

## Escopo

### Inclui

- [capability e contratos dentro do escopo]

### Não inclui

- [responsabilidades do host/consumidor, produto ou infraestrutura que ficam fora]

## Estado de partida

| Item                    | Evidência atual              | Estado     | Gap   |
| ----------------------- | ---------------------------- | ---------- | ----- |
| Core Rust               | `[caminhos/símbolos/testes]` | `pendente` | [gap] |
| Binding Python          | `[caminhos/símbolos/testes]` | `pendente` | [gap] |
| Binding TypeScript/Node | `[caminhos/símbolos/testes]` | `pendente` | [gap] |

## Entregas ordenadas

| Ordem | Entrega   | Dependências   | Critério verificável | Estado     |
| ----- | --------- | -------------- | -------------------- | ---------- |
| 1     | [entrega] | [dependências] | [evidência objetiva] | `pendente` |

## Decisões e gates

- **Decisões já tomadas:** [decisão, fonte e data]
- **Decisões vitais pendentes:** [pergunta, opções, impactos; bloquear trabalho dependente]
- **Gate para a próxima entrega:** [condição mensurável/verificável]
- **Gate de adoção/release:** [compatibilidade, segurança, operação e evidência]

## Critérios de aceite

- [ ] Contrato, versão, erros e limites documentados.
- [ ] Core Rust e bindings aplicáveis expõem comportamento coerente.
- [ ] Testes cobrem sucesso, erro e cenários de falha relevantes.
- [ ] Timeout, cancelamento, concorrência, persistência e recovery cobertos quando aplicáveis.
- [ ] Fallback/rollback do consumidor demonstrado quando aplicável.
- [ ] Checklist específico concluído e análise atualizada com evidências.

## Riscos e rollback

| Risco   | Impacto   | Mitigação   | Como reverter |
| ------- | --------- | ----------- | ------------- |
| [risco] | [impacto] | [mitigação] | [rollback]    |

## Estado e próximo passo

- **Concluído:** [itens com links para evidências]
- **Pendente:** [itens]
- **Bloqueado:** [condição de retomada ou `nenhum`]
- **Próxima etapa autorizada:** [etapa específica]
- **Condição para encerrar o goal:** [conclusão do plano ou decisão/bloqueio que exige pausa]
