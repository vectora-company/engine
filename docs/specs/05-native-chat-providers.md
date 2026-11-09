# Contratos de chat para protocolos nativos

**Estado:** experimental, contrato de chat `1`.

Este documento registra a primeira fatia de adapters nativos Anthropic Messages
e Google Gemini. As implementações vivem no núcleo Rust; Python e TypeScript
encaminham a mesma requisição e os mesmos eventos pelos bindings já incluídos
no bootstrap.

## Responsabilidades

- O consumidor escolhe provider, modelo, endpoint, credenciais, headers e
  configuração específica. A Engine não lê arquivos `.env`, não seleciona
  modelos, não executa tool calls e não faz fallback.
- `NativeChatSettings` aceita `provider` (`anthropic` ou `google`), chave,
  endpoint opcional, headers adicionais e timeouts. O endpoint padrão é o
  endpoint público oficial de cada protocolo; um endpoint configurado pode
  incluir um prefixo de path.
- Erros HTTP não incluem corpo remoto, headers ou URL com query. Chaves não
  aparecem em `Debug` nem em mensagens de erro.
- `ChatRequest.extensions` contém campos específicos do protocolo e é mesclado
  no top-level do corpo. Campos canônicos e campos de wire reservados pelo
  adapter não podem ser substituídos. O consumidor é responsável pela validade
  semântica das extensões aceitas pelo serviço.

## Anthropic Messages

- `POST /messages`; autenticação `x-api-key`, versão `anthropic-version:
2023-06-01` e `Accept: text/event-stream` em streaming.
- Mensagens `system` são concatenadas em `system`; conteúdo de texto e imagens
  mantém a ordem. Imagens `data:` base64 são convertidas para `source.base64`,
  URLs para `source.url`.
- Respostas de tool são mensagens `user` com bloco `tool_result`; mensagens do
  assistant preservam blocos `tool_use`. `is_error` é carregado no resultado
  de tool.
- A API exige `max_tokens`; quando ausente no contrato comum, o adapter usa o
  default compatível com o cliente de referência atual, `4096`.
- O stream mantém `text_delta` separado de `tool_call_delta`. Fragmentos
  `input_json_delta` são emitidos em ordem e não são parseados parcialmente.
  `message_delta.usage` é cumulativo e substitui o snapshot anterior; o
  snapshot é emitido em `message_stop`.
- `end_turn` e `stop_sequence` mapeiam a `stop`; `max_tokens` a `length`;
  `tool_use` a `tool_calls`.

## Google Gemini

- `POST /models/{model}:generateContent` ou
  `POST /models/{model}:streamGenerateContent?alt=sse`; autenticação por query
  `key`, como no cliente de referência. O endpoint base pode incluir prefixo.
- Mensagens `system` são movidas para `systemInstruction`; `assistant` vira
  `model`; respostas de tool viram `functionResponse` em mensagem `user`.
  Imagens base64 viram `inlineData`, URLs viram `fileData`.
- Schemas de tools convertem `$defs`/`$ref` recursivamente para formas inline e
  removem `additionalProperties`, que não é aceito pelo protocolo utilizado.
  As declarações usam `functionDeclarations`.
- `safetySettings` e outros campos específicos são enviados somente se o
  consumidor os fornecer em `extensions`. A Engine não copia o default
  permissivo usado pelo produto Vectora.
- Cada evento SSE contém um response completo. `parts[].text` é emitido como
  deltas em ordem; `functionCall.args` é emitido como JSON completo por chamada
  com índices crescentes entre eventos. `usageMetadata` é cumulativo; somente
  o snapshot mais recente é emitido no fim.
- `STOP`, `MAX_TOKENS` e razões de bloqueio são preservadas como razões de
  término tipadas. Stream sem candidates é erro de protocolo, inclusive quando
  há `promptFeedback` de bloqueio.

## Limites e conformidade

- Os adapters compartilham validação da requisição, limites de resposta,
  timeout total/leitura, cancelamento ao descartar o stream e classificação de
  erros HTTP. Não repetem automaticamente chamadas remotas.
- Fixtures locais verificam corpo, endpoint, autenticação, conversão de
  mensagens e ferramentas, completion, streaming, usage e eventos de erro pelos
  bindings Python e TypeScript. Não usam credenciais reais nem serviços pagos.
- Os testes não afirmam paridade integral com os SDKs ou serviços remotos.
  Modos multimodais, campos de segurança e extensões fora dos casos cobertos
  continuam dependentes do endpoint e devem ser validados pelo consumidor.
