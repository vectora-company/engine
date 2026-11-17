# Conversas, streaming e threads na engine

## Maturidade atual

O core implementa eventos de streaming de provider, montagem de turno, IDs de
run/thread, ledger/checkpoints SQLite e cancelamento/recovery do loop modelo/tool.
Também há um envelope Rust `ConversationEvent` v1 com payloads tipados para
eventos mínimos de turno, run, step, tool, aprovação, progresso, mensagem,
uso e estado terminal, além de validação de correlação. Os bindings Python e
TypeScript encaminham ao mesmo validador Rust e usam uma fixture comum. Esse
contrato ainda não está ligado ao ledger.
Isso não implementa o contrato de Conversation/Session descrito nesta spec: não
há API genérica de admission de turnos/conversas, histórico paginado por cursor,
replay de stream após reconnect, channel adapter, projeção de timeline nem
contrato separado de cancelamento do assinante. A
[`matriz de conformidade`](../analyses/02-matriz-conformidade-capabilities.md)
registra os limites observados. Até essas APIs duráveis existirem, o suporte é
**parcial** e não se deve anunciar retomada de conversa ou replay de eventos.
O plano incremental, checklist e análise desta capability estão em
[`docs/plans/conversation-events.md`](../plans/conversation-events.md),
[`docs/checklists/conversation-events.md`](../checklists/conversation-events.md)
e [`docs/analyses/conversation-events.md`](../analyses/conversation-events.md).

## Objetivo

Definir contratos reutilizáveis para conversas, turnos de agente, eventos de execução, streaming, retomada e cancelamento sem vincular a engine a um chat frontend. Consumidores diferentes podem apresentar a mesma execução como chat no Vectora, comentário/review no GH Bot, interação de voz ou interface de assistente pessoal, ou fluxo de geração em um construtor de apps.

A engine fornece o modelo de estado e eventos. O consumer escolhe canal, UI, notificação e mapeamento para entidades externas.

## Modelo conceitual

- **Conversation/session:** sequência lógica de interação sob um escopo confiável do consumer; não presume componente visual nem igualdade com uma GitHub Issue, workspace ou conta Vectora.
- **Turn:** unidade idempotente de entrada e execução; tem ID, ordem, estado e evento terminal autoritativo.
- **Run:** execução do runtime associada a um turno, com steps/tools/aprovações/evidências próprios, conforme `01-runtime-agentico-e-harness.md`.
- **Event:** fato versionado e correlacionado, ordenável por sequência/cursor e deduplicável por ID.
- **Projection:** representação consultável reconstruída de eventos para histórico/progresso; não é a fonte de verdade se o contrato usa ledger durável.
- **Channel adapter:** tradução do consumer entre seus IDs/eventos de entrada e os contratos de Conversation/Turn/Event da engine.

## Contrato de eventos

### Envelope Rust v1

`harness::ConversationEvent` define `contract_version`, `event_id`,
`conversation_id`, IDs opcionais de turn/run/step, sequência positiva,
timestamp Unix em milissegundos, `source` e `ConversationEventKind` tipado.
O produtor confiável fornece ID, ordem e timestamp; o tipo não gera IDs nem
persiste eventos. A validação exige os IDs de correlação correspondentes ao
tipo do evento e rejeita versão desconhecida, IDs vazios e sequência zero.
`MessageDelta` representa somente texto destinado ao consumidor; raciocínio
interno não é um tipo de evento.

Esta é apenas a primeira camada do contrato: redaction, admission,
armazenamento, cursor/replay, projeção e cancelamento de assinatura ainda não
estão implementados. Os testes de bindings estão escritos, mas ainda aguardam
execução no CI. Os próximos critérios estão no
[`plano da capability`](../plans/conversation-events.md).

- Eventos têm schema versionado, `event_id`, conversation/turn/run IDs relevantes, sequência/cursor e timestamp do produtor.
- Classes mínimas devem representar turn accepted/queued, run/step started, status/progress, tool call/result, approval requested/resolved, artifact/evidence available, message delta, usage update e terminal state.
- Terminal state identifica explicitamente completed, failed, degraded, cancelled ou recovery_required e não pode ser inferido do texto do modelo.
- Eventos podem carregar texto da resposta, mas não expõem raciocínio interno bruto, secrets, argumentos sensíveis ou conteúdo fora da política de divulgação do consumer.
- Distinguir eventos de estado confiável de saída do modelo/dados não confiáveis. Um texto do modelo não pode declarar a execução aprovada/concluída.
- Garantir ordem definida por stream/turn; eventos atrasados/reordenados são ignorados, reconciliados ou expostos como gap conforme o contrato, nunca usados para reabrir um turno terminal.
- Cada evento de progresso deve ser atribuível a um componente/capability e ao run/step correspondente.

## Streaming, replay e reconexão

- O streaming da engine é um contrato de eventos independente do transporte. Um host pode adaptá-lo a SSE, WebSocket, IPC, webhook ou protocolo de plataforma.
- Cursor/replay tem semântica explícita; reconexão inicia após cursor válido e deduplica eventos repetidos por ID/sequência.
- Depois de perda de conexão, o consumer pode reconciliar o estado persistido e continuar a projeção sem reiniciar efeitos.
- Backpressure, limite de buffer, política de eventos coalescíveis e cancelamento do assinante são definidos por API.
- Separar cancelar a assinatura do stream de cancelar a execução. Cancelamento de run exige ação explícita/autorizada com outcome observável.
- Timeout do transporte não converte run em sucesso ou falha sem consultar estado autoritativo.

## Admissão e idempotência de turnos

- API genérica de submissão informa aceitar, enfileirar, bloquear ou rejeitar um turno segundo capacidade/concurrency policy configuradas pelo host.
- Todo turno enviado tem ID idempotente. Repetição da mesma submissão retorna o turno existente ou conflito explícito, sem iniciar uma segunda execução.
- Estados de admission incluem queued, accepted, running, blocked, rejected e cancelled antes do início, quando aplicável.
- Concorrência por conversation/key é uma policy configurável do consumer/runtime, não uma regra de composer visual.
- A engine registra a confirmação de aceitação que permite ao client afirmar que mensagem foi recebida; falha posterior mantém distinção clara entre accepted e completed.

## Histórico e ownership

- Contratos de history/listagem oferecem paginação e cursores estáveis, ordenação e referências às execuções/events necessários.
- O consumer fornece principal/tenant/namespace confiável e autorização de acesso; parâmetros fornecidos pelo modelo não são autoridade.
- Engine valida o escopo de leitura passado pelo host e associa dados/eventos ao mesmo contexto durante replay.
- Conteúdo grande pode ser referenciado por artifact ID/hash sob as regras de storage; histórico não deve duplicar blobs ilimitados.
- A engine não define política global de retenção ou exportação de produto; define metadata para a retenção aplicada pelo consumer.

## Filas de interação e canais externos

A engine controla idempotência/admission de turnos e estado de execução. Ela não implementa fila de mensagens do composer, botão de reenviar, rascunho de UI, layout, viewport, auto-scroll, badges, notificações locais/push ou garantia de entrega de uma plataforma externa. O channel adapter do consumer pode implementar essas funcionalidades em cima dos recibos e eventos da engine.

GH Bot pode mapear comentário, issue, review ou workflow a uma conversation e preservar IDs/regras da plataforma no adapter. Um assistente desktop pode mapear sessões locais; um app builder pode mapear geração de artefatos. Essas entidades externas não são tipos obrigatórios do core.

## Observabilidade

- Correlacionar conversation → turn → run → step → tool/provider/event sem confundir IDs do consumer com IDs internos.
- Medir latência de admission, primeiro evento, progresso, conclusão e replay; registrar disconnect, cursor inválido, gaps, dedup e cancelamento.
- Expor estado do runtime e usage autorizado; nunca depender de heurística textual/timer para dizer “terminou”.
- Logs e eventos mantêm redaction e minimização; payloads grandes/sensíveis permanecem referenciados conforme a política.

## Critérios de aceite

- Consumidores com canais diferentes conseguem produzir a mesma estrutura sem depender de UI/chat React.
- Repetir turn submission com a mesma idempotency key não cria outro efeito; eventos repetidos não duplicam projeções.
- Evento terminal é autoritativo; turn terminal não reabre por mensagem atrasada.
- Replay após reconnect mantém cursor e ordem, reporta gaps e reconcilia com estado persistido.
- Cancelar stream e cancelar run são operações distintas; ambas têm semântica testada.
- Sem estado persistido não se promete retomada durável; sem transporte disponível não se afirma notificação entregue.
- Testes validam efeitos, IDs, sequência, cursor e estado depois de restart, não apenas status HTTP ou renderização.
- Nenhum contrato exige composer, queue UI, notificação push, workbench ou estrutura de workspace Vectora.

## Dependências e evolução

- Usa os eventos e estados de execução definidos em `01-runtime-agentico-e-harness.md`.
- Durabilidade, ledger, replay, cursor e artifacts dependem dos contratos em `02-storage-memoria-contexto-rag.md`.
- Alterações de schema/eventos definem política de compatibilidade e versão consumida pelo channel adapter.
