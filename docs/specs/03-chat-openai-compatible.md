# Contrato de chat OpenAI-compatible

**Estado:** experimental, contrato `1`. A implementação Rust é a única fonte de
semântica. Este documento descreve a primeira fatia do protocolo Chat
Completions; não afirma compatibilidade integral com qualquer gateway que use
o rótulo “OpenAI-compatible”.

## Fronteira

O consumidor escolhe o modelo, endpoint, credenciais, headers e política de
produto. A engine valida a configuração, serializa mensagens e ferramentas,
executa HTTP assíncrono e converte resposta ou SSE em tipos versionados. Ela
nunca lê `.env`, carrega credenciais de conta, executa tool calls ou faz
fallback entre modelos.

Python/PyPI e TypeScript/Node/npm são bindings do mesmo núcleo Rust. Eles devem
preservar versão do contrato, ordem dos eventos, erros classificados,
cancelamento e redaction. Não devem reimplementar transporte ou protocolo.

## Requisição canônica

```json
{
  "contract_version": 1,
  "model": "example-model",
  "messages": [{ "role": "user", "content": "Explique este trecho." }],
  "options": { "temperature": 0.2, "max_tokens": 512 },
  "tools": [],
  "extensions": { "reasoning": { "effort": "high" } }
}
```

`messages[].content` aceita texto ou uma lista discriminada de blocos `text` e
`image_url`. Chamadas de ferramenta são dados de saída; a engine não as
executa. Argumentos de ferramenta válidos são objetos JSON. Argumentos
malformados recebidos do provider são preservados explicitamente como estado
`invalid_json`, sem desaparecer nem serem reinterpretados.

O contrato é enviado aos bindings, mas não é incluído no JSON de wire do
provider. Campos opcionais ausentes permanecem ausentes. `max_tokens` deve ser
positivo; temperatura finita e não negativa; nome de modelo e nomes de tools
não podem ser vazios.

`extensions` é um mapa opcional de campos específicos do provider, mesclado no
top-level da requisição de wire por cada adapter. A engine rejeita campos
canônicos reservados para evitar que extensões sobrescrevam partes controladas
pelo contrato. Cada adapter também protege os campos reservados próprios do
protocolo. O consumidor fornece somente extensões suportadas pelo endpoint
configurado; em Gemini, por exemplo, `safetySettings` é configuração explícita
do consumidor e não recebe defaults de política de um produto específico.

## Eventos de streaming

O consumidor recebe eventos discriminados, cada qual com `contract_version`:

```json
{"contract_version":1,"type":"text_delta","text":"Olá"}
{"contract_version":1,"type":"reasoning_delta","text":"análise"}
{"contract_version":1,"type":"tool_call_delta","call":{"index":0,"id":"call-1","name":"search","arguments_fragment":"{\"q\":"}}
{"contract_version":1,"type":"usage","usage":{"input_tokens":12,"output_tokens":3,"total_tokens":15}}
{"contract_version":1,"type":"completed","finish_reason":"stop"}
```

Os fragmentos de tool call são incrementais e devem ser acumulados pelo
consumidor usando `index`; um fragmento pode conter somente parte do nome ou
dos argumentos. A engine emite `completed` uma única vez, incluindo no EOF sem
marcador terminal. O marcador SSE `[DONE]` encerra o stream sem duplicar esse
evento. O stream aplica backpressure; descartá-lo cancela a leitura HTTP.
Campos de reasoning recebidos em `reasoning` ou `reasoning_content` são eventos
separados e nunca são concatenados ao texto. Em resposta não-stream, o campo
`reasoning` é preservado separadamente quando fornecido. `usage.cost`, quando
numérico, é preservado como metadado opcional junto aos contadores de tokens.

## Erros e segurança

Os erros públicos têm categorias estáveis: configuração/versão inválida,
autenticação, quota, rate limit (com `Retry-After` opcional), rejeição do
provider, erro do servidor, conexão, timeout, tamanho excedido, resposta
inválida, violação de protocolo e cancelamento. Corpos e mensagens recebidas do
provider não são expostos nos erros. `Debug` da configuração redige API key e
valores de headers; o endpoint exibido não inclui userinfo, query ou fragmento.

Retry é limitado a falha de conexão antes de obter headers/resposta. Erros HTTP,
timeouts e falhas após começar a stream não reiniciam geração. A engine não
registra request/response nem segredos.

## Limites desta fatia

- O transporte usa `POST {base_url}/chat/completions`, bearer auth opcional,
  headers adicionais não reservados e SSE para stream. O path prefixo do
  `base_url` é preservado, permitindo endpoints como `/v1` e `/engines/v1`; o
  consumidor escolhe autenticação opcional e headers específicos por endpoint.
- Não há matriz de paridade aprovada para OmniRouter, 9Router, OpenRouter,
  llama.cpp, DMR ou Ollama. Só fixtures verificadas podem adicionar um perfil;
  Ollama exige seu protocolo próprio.
- Formatos multimodais além de `image_url`, tool choice, JSON mode, logprobs e
  usage final em trailers não são prometidos até serem caracterizados.
- Fallback, seleção de provider, telemetria e integração de produto continuam
  sob responsabilidade dos consumidores.

## Perfis observados no backend de referência

Os perfis abaixo orientam a configuração do contrato comum. A matriz descreve
o que o backend Python configura hoje; fixtures locais verificam as regras de
path, headers e payload sem afirmar disponibilidade ou paridade de serviços
remotos.

| Perfil     | `base_url` configurável                     | Autenticação                       | Detalhe preservado                                                               |
| ---------- | ------------------------------------------- | ---------------------------------- | -------------------------------------------------------------------------------- |
| OpenRouter | `https://openrouter.ai/api/v1`              | Bearer                             | Headers de atribuição são enviados pelo consumidor, nunca inseridos pela engine. |
| 9Router    | Endpoint definido pelo usuário              | Bearer                             | A engine acrescenta `/chat/completions` ao prefixo informado.                    |
| OmniRouter | Endpoint definido pelo usuário              | Configuração do consumidor         | Usa o mesmo contrato OpenAI-compatible, sem URL ou identidade codificada.        |
| llama.cpp  | Endpoint externo ou local com prefixo `/v1` | Opcional                           | Sem attribution headers; prefixo do path continua intacto.                       |
| DMR        | Endpoint com prefixo `/engines/v1`          | Ausente no perfil atual do Vectora | O consumidor passa o prefixo completo, que a engine preserva.                    |

Reasoning de resposta e `usage.cost` são mantidos em campos/eventos separados.
As extensões de requisição permitem ao consumidor enviar opções específicas
do modelo, como configuração de reasoning, sem colocar essa política dentro da
engine.

## Conformidade

Fixtures locais sem credenciais devem cobrir serialização e validação,
conclusão, sequência de eventos SSE, tool calls fragmentadas, usage, erros HTTP,
SSE inválido, limites, timeout, disconnect e cancelamento. Os mesmos vetores de
entrada e saída serão usados por Rust, Python e TypeScript; comportamento
específico de runtime deve ser testado separadamente sem mudar o contrato.
