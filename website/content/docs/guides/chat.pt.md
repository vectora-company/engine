---
title: Chat e adapters de modelo
description: Configure chamadas, consuma streams e entenda os limites dos providers.
weight: 1
---

Os contratos de chat são experimentais. O consumidor escolhe provider, modelo, endpoint, credenciais, headers e política de produto. A engine valida requisições, executa o transporte do protocolo e retorna completions ou eventos tipados.

## Chat compatível com OpenAI

O adapter inicial envia `POST {base_url}/chat/completions`, preservando o prefixo de caminho informado em `base_url`. Uma requisição mínima usa contrato versionado e mensagem de usuário:

```python
settings = {
    "base_url": "https://api.example.test/v1",
    "api_key": "...",
    "connect_timeout_ms": 10000,
    "read_timeout_ms": 60000,
    "total_timeout_ms": 60000,
}
request = {
    "contract_version": 1,
    "model": "example-model",
    "messages": [{"role": "user", "content": "Olá"}],
    "options": {"max_tokens": 256},
}
```

O schema exato de settings está em [`03-chat-openai-compatible.md`](https://github.com/vectora-ltda/engine/blob/main/docs/specs/03-chat-openai-compatible.md). Mantenha credenciais fora do código e dos logs. A engine redige chaves e não lê arquivos `.env`.

## Streaming

Use `chat_stream` (Python) ou `chatStream` (TypeScript) para consumir eventos de texto, reasoning, fragmentos de tool call, usage, troca de modelo e conclusão. Os argumentos da tool podem chegar em fragmentos; acumule pelo índice fornecido e valide a resposta completa antes do dispatch. Descartar/cancelar o stream cancela a leitura HTTP. Não misture reasoning ao texto de resposta exibido ao usuário.

## Protocolos nativos e fallback

Os adapters nativos experimentais cobrem Anthropic Messages, Google Gemini e Cohere Chat v2. Embeddings e reranking têm APIs experimentais separadas. O fallback é explícito e ordenado: o consumidor fornece candidatos elegíveis e a engine só troca em falhas classificadas antes de emitir conteúdo no streaming. Ela não descobre credenciais, escolhe modelos nem presume compatibilidade de gateways.

Consulte a [maturidade das capacidades](../../concepts/maturity/) antes de depender da cobertura de protocolos. Em particular, não há promessa de compatibilidade integral com OmniRouter, 9Router, OpenRouter, Ollama ou outros gateways sem perfil verificado e fixtures de conformidade.
