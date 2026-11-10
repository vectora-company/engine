# Plano de continuidade da Vectora Engine

**Levantamento:** 9 de novembro de 2026
**Referência remota:** branch `main`, commit `43a2655`
**Objetivo:** transformar as capabilities experimentais já implementadas numa
engine distribuível e adotável com segurança, sem substituir o backend Python
nem presumir paridade.

## Resumo executivo

A engine já deixou o bootstrap: o core Rust implementa chat OpenAI-compatible,
adapters nativos Anthropic/Google/Cohere, fallback, embeddings/rerank e uma
fundação experimental de Harness. Os bindings Python e TypeScript/Node chamam o
mesmo core e passam por CI.

O bloqueio para adoção não é falta de mais providers. É que execução, loop e
dispatch de tools ainda são contratos em memória: não há reserva idempotente
atômica, ledger/checkpoint durável, policy de autorização ou recovery após
restart. O dispatch prepara decisões, mas o consumer ainda executa as tools e
precisa revalidar a policy antes de efeitos. As APIs continuam marcadas como
experimentais.

O app consumidor segue no backend Python e não declara dependência nem uso de
`vectora-engine` nos manifests ou caminhos de runtime examinados. Portanto, o
status atual é **capability implementada na engine, ainda sem adoção comprovada
no Vectora App**. A continuidade deve priorizar contratos conformes,
durabilidade/autorização do Harness e um piloto estreito e reversível; não uma
migração ampla nem mais superfície de provider.

## Estado observado

| Área                | Estado em `main`                                                                | Limite relevante                                                                                      |
| ------------------- | ------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Core e automação    | Workspace Rust, CI multiplataforma, tooling e crate `0.1.0`                     | Não há workflow de release nos workflows examinados.                                                  |
| Chat                | OpenAI-compatible, Anthropic, Google, Cohere e fallback explícito               | Contrato experimental; fallback/seleção de produto continuam no consumer.                             |
| Embeddings/rerank   | Adapters e fallback explícito no Rust, expostos pelos bindings                  | Experimental; não migra storage, indexação nem o RAG do produto.                                      |
| Run lifecycle       | Estados e transições locais determinísticas                                     | A chave de idempotência não é reservada; snapshots não são persistidos.                               |
| Turn assembly       | Acumula eventos, tool calls, usage e limites                                    | Não persiste, autoriza nem executa tools.                                                             |
| Tool dispatch       | Valida schema/capabilities e produz decisões default-deny                       | Não autentica o host, executa adapters, impõe timeout, policy completa ou aprovação.                  |
| Model/tool loop     | Coordena turns e resultados, limita orçamento e interrompe efeito incerto       | Em memória; snapshots recarregados não são confiáveis e recovery durável não existe.                  |
| Storage/memória/RAG | Especificação e sequência de implementação propostas                            | Não há crate/adapters de storage da engine implementados.                                             |
| Distribuição        | Configuração local de Python/PyO3 e TypeScript/N-API                            | Publicação não comprovada nesta auditoria; issue npm aberta e sem workflow de release no repositório. |
| Vectora App         | Backend Python mantém `run_conversation`, registry de tools e clients/fallbacks | Nenhum uso da binding Rust foi encontrado nos manifests/caminhos examinados.                          |

O checkout do app observado estava em `feat/ssh-client-server-tailwind`, com
19 commits à frente e 19 atrás de seu upstream; os commits locais mais recentes
eram de 8 de novembro e tratavam de setup/build desktop, não de integração da
engine. Sincronizar e identificar a base de integração antes de abrir um PR no
app.

## Auditoria do GitHub

Foi encontrada uma issue pública: [#2 — publicar a binding TypeScript no
npm](https://github.com/vectora-ltda/engine/issues/2), ainda aberta e sem
comentários. Seu escopo inclui mais capacidades do que a engine oferece hoje
(por exemplo, storage e Decision Models); dividir a entrega por APIs realmente
implementadas e não declarar aceite por cobertura futura.

Os 11 PRs encontrados estão fechados e merged:

| PRs                                                                                                       | Entrega                                                        |
| --------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| [#1](https://github.com/vectora-ltda/engine/pull/1)                                                       | Bootstrap Rust, CI e tooling.                                  |
| [#3](https://github.com/vectora-ltda/engine/pull/3)–[#4](https://github.com/vectora-ltda/engine/pull/4)   | Bindings e chat OpenAI-compatible/perfis.                      |
| [#5](https://github.com/vectora-ltda/engine/pull/5)–[#6](https://github.com/vectora-ltda/engine/pull/6)   | Protocolos nativos Anthropic, Google e Cohere.                 |
| [#7](https://github.com/vectora-ltda/engine/pull/7)–[#8](https://github.com/vectora-ltda/engine/pull/8)   | Fallback, embeddings e rerank.                                 |
| [#9](https://github.com/vectora-ltda/engine/pull/9)–[#12](https://github.com/vectora-ltda/engine/pull/12) | Lifecycle, turn assembly, dispatch e coordenação modelo/tools. |

Os checks de CI reportados pelos PRs merged passaram nas plataformas Rust e nos
jobs de tooling/bindings aplicáveis. O histórico de review merece uma ação
específica: PR #1 recebeu aprovação; PR #3 foi marcado `CHANGES_REQUESTED` pelo
CodeRabbit e ainda tem cinco threads de review não resolvidas no GitHub; PRs
#4–#12 não têm review formal registrado. As observações do PR #3 incluem
redirecionamentos HTTP, validação do valor da API key, serialização de campos
opcionais, `null` em usage/tool calls e a ordem de build/typecheck no SCons.
Reproduzir cada observação contra `main` antes de tratá-la como bug atual ou
descartá-la como obsoleta.

O plano de portabilidade já prevê gates para bindings, conformidade, adoção e
rollback; o plano do Harness coloca policy/aprovação, persistência/recovery e
verificação depois das fundações já entregues. A especificação de storage
continua sendo proposta: a issue local é fonte de requisitos, não contrato
Rust pronto.

## Plano priorizado

### 0. Fechar a fotografia de conformidade e review

1. Reproduzir e classificar as cinco threads não resolvidas do PR #3 contra o
   `main` atual; resolver, justificar ou abrir trabalho rastreável para cada
   uma.
2. Consolidar uma matriz por capability com versão do contrato, APIs Rust,
   Python e TypeScript, maturidade, limites, erros, timeout, cancelamento,
   ownership e fallback.
3. Criar/compartilhar fixtures determinísticas para os contratos públicos e
   exercitá-las pelas três entradas. Distinguir cobertura de protocolo de
   diferenças específicas de runtime; sem credenciais, rede paga ou efeitos
   externos.
4. Reconciliar a semântica de fallback documentada e implementada antes de
   congelar o contrato. Registrar alterações locais não merged como trabalho em
   andamento, não como comportamento de `main`.

**Saída:** gaps atuais reproduzíveis, feedback de review triado e uma linha de
base de conformidade para cada binding.

### 1. Especificar storage durável (S0), antes de implementá-lo

Usar a issue local de storage como levantamento e adaptar a intenção para uma
especificação Rust própria. Mapear consumidores/dados e escolher o primeiro
backend somente após identificar as necessidades reais. Definir o contrato de
store para reserva atômica por thread/turn/idempotency key, ledger ordenado,
checkpoint, compare-and-set, migração, retenção, redaction e falhas.

Implementar depois um recorte mínimo para persistir lifecycle e coordenação do
Harness. Testar concorrência, reinício, replay, duplicatas, migração e
rollback. Estado incompatível ou checkpoint ausente deve falhar explicitamente;
não criar uma execução nova como fallback silencioso.

**Saída:** uma capability durável com garantias declaradas, sem dual-write nem
migração dos dados de produto.

### 2. Tornar policy e aprovação uma fronteira efetiva

Definir snapshot versionado emitido pelo host confiável; ligar aprovação a
tool, argumentos normalizados, escopo, digest de policy e expiração; invalidar
aprovação após mudança relevante. Revalidar imediatamente antes de cada efeito
e depois de retomada. Separar estado do run, resultado da verificação e
veredito do gate.

O core não deve autenticar usuários nem importar RBAC do Vectora. O consumer
continua responsável por identidade, policy de produto e adapters. Capability
ausente, policy indisponível e contexto alterado devem falhar fechado segundo
contratos testáveis.

**Saída:** nenhuma tool com efeito externo é executável com base apenas em
capabilities do modelo ou em snapshot não confiável.

### 3. Completar cancelamento e recovery antes de efeitos

Especificar deadlines e cancelamento de run distintos de cancelamento de
assinante/stream; propagar cancelamento aos providers e adapters; persistir o
resultado terminal. Modelar resultado incerto, reconciliação e retomada sem
repetir cegamente um efeito. Impor timeout no ponto de execução, não apenas
transportar o timeout declarado pelo contrato.

**Saída:** testes com falha parcial, restart, timeout, cancelamento, resultado
`unknown` e reconciliação sem duplicação de efeitos.

### 4. Integrar um piloto reversível no Vectora App

Primeiro, sincronizar a branch de integração do app. Adotar uma única capability
por vez e manter Python como caminho padrão/fallback até o gate de operação.
Um piloto de chat pode usar o binding Python com a montagem de turno, sem
executar tools: flag opt-in, fixtures locais compartilhadas, decisão de
provider/credenciais no consumer e rollback direto para o client atual.

Não chamar Python e Rust em paralelo contra providers reais para “comparar”:
isso duplica custo/efeitos. Comparar por fixtures/cassettes redigidos e
workloads sintéticos isolados. Não remover o runtime Python nem mover settings,
fallback de produto, telemetria, RAG ou regras de negócio para a engine.

Depois que storage, policy e recovery passarem seus gates, avaliar uma
capability de Harness com tools read-only. Tools com efeitos destrutivos ficam
fora do piloto inicial.

**Saída:** consumidor identificado, comportamento compatível nos casos
aprovados, métricas e rollback demonstrados, sem declarar paridade global.

### 5. Publicar bindings e integrar consumidores Node

Reduzir a issue #2 a entregas verificáveis: API TypeScript que corresponde às
capabilities existentes; Node/OS/arquiteturas suportados; instalação limpa e
carregamento do addon; versão/ABI vinculada ao mesmo SHA/tag do core; artefatos
reproduzíveis e workflow de release separado do CI. Publicar primeiro como
versão explicitamente experimental, após build e smoke em cada alvo anunciado.

O GH Bot deve ser consumidor independente, não dependência do Vectora App. Não
publicar ou documentar storage/Decision Models como capacidade disponível
enquanto não existirem contratos e cobertura correspondentes. Coordenar a
estratégia de versão/release do crate, PyPI e npm sem afirmar que releases já
existem.

**Saída:** instalação e API comprovadas em projetos Node limpos, sem browser,
Electron ou lógica paralela em TypeScript.

### 6. Medir, amadurecer e expandir

Após o piloto, criar evidência reproduzível por SHA, ambiente, configuração,
dataset/fixture e comando. Medir latência, memória, throughput, custo,
qualidade, recuperação e falhas por capability. Separar `passed`, `failed`,
`blocked`, `skipped` e `not_run` da maturidade da API. Usar essa evidência para
decidir se amplia adoção, começa adapters de storage adicionais ou trabalha em
mídia/STT/TTS; não antecipar esses ports por semelhança de diretório.

## Gates de adoção e release

Uma capability só avança de experimental quando:

- contratos e erros são versionados, documentados e testados nas entradas
  aplicáveis;
- timeout, cancelamento, redaction e falhas/degradação têm resultado observável;
- concorrência/restart e idempotência são testados quando houver efeitos;
- o fallback e rollback do consumidor são testados sem duplicar efeitos;
- métricas são reproduzíveis e limitadas ao workload que realmente mediram;
- os consumidores e limites de compatibilidade estão identificados.

## Próxima entrega recomendada

Abrir o próximo PR como **triagem dos cinco comentários pendentes do PR #3 e
conformidade compartilhada dos contratos já implementados**. Isso reduz risco
antes de estabilizar/publicar API; em paralelo, produzir a especificação S0 de
storage a partir dos consumidores reais. Não começar pela substituição do
loop Python, pela execução de tools com efeitos ou por mais adapters LLM.

## Nota de proveniência local

Durante esta auditoria, a branch `audit/main-commit-walkthrough` partiu do
commit `43a2655` de `main`. Ela corrige a política de fallback identificada
acima, alinha sua spec e registra o walkthrough em
`docs/audits/01-auditoria-e-walkthrough-da-main.md`. `scons ci` passou nessa
branch; o CI do GitHub ainda deve validar o PR.
