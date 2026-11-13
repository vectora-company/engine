# Auditoria e walkthrough da `main`

> Registro histórico de 2026-11-09. O estado atual está no
> [plano ativo](../plans/03-plano-continuidade.md); não trate as observações
> abaixo como estado corrente sem revalidar o código.

**Data da revisão:** 2026-11-09
**Base auditada:** `main` em `43a26552194d979f64d2fe12f1464f14f23f07a2`
**Escopo:** todos os 12 commits da linha principal, do commit inicial até o loop
de modelo e tools. Os commits foram lidos como código e documentação; aprovação
e merge anteriores não foram tratados como evidência de correção.

## O que existe ao fim desta sequência

A Engine tem um crate Rust com APIs de chat, adapters de providers, embeddings,
rerank e contratos iniciais do harness. As mesmas capabilities de produto
incluídas nesses PRs são expostas por bindings Python/PyO3 e TypeScript/N-API.
Há contratos JSON versionados, testes com fixtures locais, tooling de SCons,
pre-commit e CI para Rust, Python, TypeScript e Windows/macOS/Linux.

O resultado ainda é uma base incremental, não uma substituição pronta do motor
Python do Vectora nem uma engine geral completa. Em particular, os PRs de
lifecycle, turn assembly, dispatch e model-tool loop definem contratos e
coordenação; não implementam persistência durável, autenticação/autorização de
produto, execução de tools do host, migração do backend, armazenamento vetorial
ou publicação dos pacotes no crates.io/PyPI/npm. A spec de storage permanece
planejamento, não capability implementada nesta sequência.

## Walkthrough commit a commit

### 1. `a27c8b5` — `chore: inicializar repositório da engine com licença AGPL-3.0`

Cria a identidade inicial do repositório e publica a licença AGPL-3.0. É a base
de histórico sobre a qual o bootstrap e as capacidades seguintes foram
construídos; não contém runtime de IA.

### 2. `8653a50` — `chore: preparar bootstrap da Vectora Engine em Rust`

Estabelece o workspace Cargo e o crate inicial, documentação de arquitetura,
AGENTS/CONTRIBUTING/SECURITY, plano de portabilidade e specs de referência.
Adiciona `SConstruct`, tooling compartilhado em `utils/`, pre-commit, extensões
do VS Code, workflow de CI e setup reproduzível do ambiente. O bootstrap define
Rust como núcleo e registra as integrações Python/TypeScript como parte do
destino do projeto, ainda sem bindings funcionais neste commit.

### 3. `d63a4c6` — `feat: incluir bindings Python e TypeScript no bootstrap da engine (#3)`

Adiciona o primeiro contrato de chat OpenAI-compatible no núcleo Rust e o
conecta a Python/PyO3 e TypeScript/N-API. Inclui streaming SSE, tipagem e
fixtures/testes locais nos dois bindings; atualiza SCons e CI para compilar e
verificar os consumidores. Este é o primeiro slice vertical compartilhado entre
Rust e as duas linguagens, não uma publicação dos pacotes.

### 4. `4e53c22` — `feat: preservar extensoes de gateways OpenAI compativeis (#4)`

Amplia o contrato OpenAI-compatible para extensões de gateways: prefixos de
endpoint e campos de reasoning/usage/custo que não cabem no subconjunto
OpenAI estrito. Acrescenta fixtures de compatibilidade e validação pelos
bindings. Não adiciona um adapter de protocolo novo.

### 5. `ee40a1d` — `feat: adicionar adapters nativos Anthropic e Google (#5)`

Implementa adapters Rust para Anthropic Messages e Google Gemini, incluindo
completion e streaming; disponibiliza as operações também por Python e
TypeScript. Documenta diferenças de protocolo e mantém o caminho
OpenAI-compatible disponível.

### 6. `9cd670b` — `feat: adicionar suporte nativo ao chat Cohere (#6)`

Adiciona o adapter Cohere Chat v2 ao conjunto de protocolos nativos e expõe sua
capability nos dois bindings, com testes de fixture e documentação atualizada.

### 7. `0c8e15f` — `feat: adicionar fallback explícito de chat (#7)`

Adiciona seleção ordenada e explícita de candidatos, filtragem de IDs repetidos
e providers duplicados, classificação tipada de erros, `selected_model_id` e
evento de troca em streaming. A troca de provider não repete uma resposta após
saída parcial. Os bindings expõem completion e streaming. A auditoria encontrou
um defeito nesta política e a branch desta revisão o corrige: antes, qualquer
erro de um candidato posterior (exceto cancelamento) autorizava outra tentativa
e podia mascarar falhas permanentes. A política agora é uniforme em todos os
candidatos e coincide com a spec corrigida em `docs/specs/06-chat-fallback.md`.

### 8. `a6e151a` — `feat: adicionar embeddings e rerank nativos (#8)`

Adiciona embeddings e rerank ao Rust: endpoints OpenAI-compatible, OpenRouter,
Cohere, Voyage e Ollama para embeddings; Cohere, Voyage e OpenRouter para
rerank, conforme a capability documentada. O fallback de embeddings e rerank é
restrito a quota/rate limit. Inclui limites HTTP, respostas tipadas, exposição
em Python/TypeScript e testes locais. Isso não transfere o storage/RAG do
backend: a spec de storage ainda descreve trabalho futuro.

### 9. `01d0625` — `feat: iniciar contrato do harness de execução (#9)`

Introduz contratos versionados para ciclo de execução e estado em memória,
expostos por Python e TypeScript. A spec deixa claro que isso ainda não oferece
persistência, idempotência durável, autenticação nem execução de tools.

### 10. `90eb1a0` — `feat: montar turnos a partir do stream nativo (#10)`

Adiciona montagem de turnos a partir dos eventos do modelo: texto, fragments de
tool call, usage, término e modelo selecionado. Expõe a função nas bindings e
documenta o contrato. Isso agrega eventos em um turno, mas não decide sozinho
como o produto persiste a conversa.

### 11. `6462493` — `feat: definir contratos e dispatch de tools (#11)`

Adiciona contratos de tools com schema, capabilities, limites e despacho
determinístico. O modo nega por padrão tools/capabilities desconhecidas e
valida argumentos; separa chamadas concorrentes somente de leitura das
operações com efeitos laterais. O dispatch produz trabalho para o host, mas não
executa adapters nem concede autorização de produto por si só.

### 12. `43a2655` — `feat: coordenar rodadas de modelo e tools (#12)`

Adiciona o loop que conecta rodada do modelo, dispatch do host, resultados
correlacionados e nova rodada, com limites e caminhos de recuperação para
efeitos desconhecidos. Os bindings Python e TypeScript expõem o protocolo. O
contrato continua em memória; persistência, execução concreta e fronteiras de
confiança continuam responsabilidade do consumidor/host.

## Achados da auditoria

1. **Corrigido nesta branch — fallback podia esconder erro permanente.**
   `can_fallback` considerava qualquer erro de candidatos após o primeiro como
   elegível. Agora somente as categorias temporárias/incompatibilidades
   explicitamente reconhecidas permitem avançar, independentemente da posição.
   Autenticação, erros de request, resposta/protocolo e cancelamento propagam.
2. **Corrigido nesta branch — spec divergia da implementação desejada.**
   A spec de fallback dizia que qualquer erro posterior permitia continuar; foi
   alinhada à classificação explícita para cada candidato.
3. **Limite de produto, não falha de implementação escondida:** storage/RAG,
   adoção pelo backend Vectora/GH Bot e publicação de pacotes não fazem parte
   dos 12 commits auditados. Seus planos/especificações não devem ser
   interpretados como funcionalidades entregues.
4. **Sem alegação de equivalência/performance:** esta sequência não prova
   paridade integral com o backend Python nem ganho de performance. Isso exige
   testes de conformidade e benchmarks reproduzíveis por SHA, ambiente e
   configuração.

## Evidências e validação

- O histórico foi verificado pela cadeia first-parent de `main`; cada commit
  acima foi revisado individualmente, incluindo seu escopo e os arquivos
  alterados.
- A correção de fallback tem testes unitários que enumeram categorias
  elegíveis e permanentes e confirmam que a decisão independe da posição.
- `scons ci` passou nesta branch: lint/checks Rust, Python e TypeScript; 44
  testes Rust, 89 testes Python e 17 testes TypeScript passaram. O workflow do
  GitHub ainda precisa validar o commit/PR desta correção; o CI verde observado
  na `main` valida apenas o estado anterior.
