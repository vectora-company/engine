# Análise: [nome da capability]

**Atualizada em:** [AAAA-MM-DD]  
**Plano:** [caminho em docs/plans/]  
**Spec/requisitos:** [referências locais]  
**SHA/branch examinados:** [referência local; não usar estado de CI remoto como prova]

## Pergunta de investigação

[Qual decisão ou lacuna esta análise deve resolver?]

## Fontes consultadas

| Fonte | Localização/símbolo | O que confirma | Limite da fonte |
| --- | --- | --- | --- |
| Spec/requisito | `[caminho]` | [intenção] | Não prova implementação |
| Código | `[caminho:símbolo]` | [comportamento observado] | [limite] |
| Teste | `[caminho:nome]` | [comportamento exercitado] | Não prova cenários não cobertos |
| Documento primário externo | [link/título] | [fato] | [versão/data] |

## Estado observado por camada

| Camada/consumidor | Implementado | Testado | Limites/gaps |
| --- | --- | --- | --- |
| Core Rust | [símbolos e arquivos] | [testes] | [gaps] |
| Python | [API/stub] | [testes] | [gaps] |
| TypeScript/Node | [API/types] | [testes] | [gaps] |
| Host/consumidor | [integração] | [evidência] | [gaps] |

## Contratos e invariantes

- **Entradas/saídas e versão:** [tipos e compatibilidade]
- **Erros e degradação:** [taxonomia/comportamento]
- **Timeout, retry e cancelamento:** [ownership e garantias]
- **Concorrência/idempotência:** [garantias provadas]
- **Dados, persistência, replay e recovery:** [fonte da verdade e limites]
- **Autenticação/autorização/trust boundary:** [responsabilidade do core versus host]
- **Observabilidade/redaction:** [eventos e dados proibidos]
- **Fallback/rollback:** [como preservar comportamento]

Marque cada item `não aplicável` com a justificativa correspondente; não deduza
garantias de nomes de tipos ou da presença de um adapter.

## Findings

| ID | Finding | Evidência reproduzível | Severidade/impacto | Ação/plano |
| --- | --- | --- | --- | --- |
| A1 | [fato ou hipótese claramente identificado] | [caminhos, símbolos, comando/teste] | [impacto] | [ação] |

## Alternativas e decisões

| Opção | Benefícios | Custos/riscos | Decisão e responsável |
| --- | --- | --- | --- |
| [opção] | [benefícios] | [riscos] | [decisão ou pendente de brunosrz] |

## Decisão vital pendente

Se houver decisão que altere contrato, dados, segurança, compatibilidade ou
escopo, registrar a pergunta, opções, impactos, trabalho dependente e condição
para retomada. O loop para até `brunosrz` responder. Se não houver, registrar
`nenhuma`.

## Conclusão limitada pela evidência

[O que a evidência permite concluir, o que não permite concluir e qual é a
próxima verificação necessária.]
