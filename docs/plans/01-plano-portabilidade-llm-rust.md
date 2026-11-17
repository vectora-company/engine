# Plano: contratos de inferência Rust e bindings

Este plano descreve a evolução por capability da camada de inferência. A
implementação canônica fica em Rust; Python e TypeScript encaminham ao mesmo
core. Estado de branch/PR é consultado no GitHub e não estrutura o roadmap.
Plano ativo da EPIC: [`03-plano-continuidade.md`](03-plano-continuidade.md).

## Objetivo

Construir APIs tipadas e reutilizáveis para inferência e operações relacionadas,
preservando comportamento de consumidores e sem duplicar protocolo nos
bindings. Suporte deve ser afirmado por endpoint, operação e modalidade com
fixtures ou integração real adequada.

## Estado atual

- Chat, stream, perfis nativos, fallback e montagem de turno existem no núcleo.
- Anthropic, Gemini, Cohere, Ollama, OpenAI/OpenRouter e endpoints
  OpenAI-compatible têm implementações específicas nos limites descritos na
  [spec de providers](../specs/18-providers-e-modalidades-llama-cpp-v1.md).
- Embeddings e rerank cobrem providers selecionados; llama-server oferece
  superfícies adicionais de texto e multimídia.
- Decision Models incluem clientes Laya e llama-server; áudio realtime e TTS
  têm contratos próprios do llama-server.
- Rust é exposto via bindings Python e TypeScript/Node. Build/distribuição,
  conformidade integral e adoção por consumidores continuam gates da EPIC.

## Fases por capability

### Contratos de chat e transporte

- Tipos versionados, erros discriminados, timeouts, limites, cancelamento e
  streaming com backpressure.
- SSE e NDJSON são decodificados conforme cada protocolo.
- Retry/fallback só antes da primeira saída observável; não reiniciar geração
  parcial.
- Credenciais e headers sensíveis são redigidos; configuração inválida falha
  antes da chamada externa.

### Perfis de provider

- Manter cliente genérico OpenAI-compatible para endpoints realmente
  compatíveis e perfis dedicados para diferenças semânticas.
- Preservar reasoning, usage, tools, extensões e eventos terminais quando o
  contrato comum representa esses dados; rejeitar perda silenciosa.
- Validar OpenAI Chat Completions e Responses separadamente; não inferir
  suporte de uma API a partir da outra.
- Validar 9router, OmniRouter e Kimi individualmente antes de perfil nominal.

### Operações de inferência

- Decision Models permanecem distintos de chat.
- Embeddings e rerank têm requests/responses próprios, ordenação, dimensão,
  scores e fallback explícitos.
- Mídia separa request persistível de envio efêmero; bytes vêm do asset map do
  consumidor e passam por verificações de integridade/limite.
- TTS, STT, imagem e vídeo exigem contratos por operação; não são decorrências
  automáticas de um provider ou de um modelo multimodal.

### Bindings, consumidores e gates

- Core Rust primeiro; ambos os bindings acompanham a mesma capability e usam
  fixtures equivalentes.
- Consumidores adotam capability por capability, com fallback e rollback; a
  Engine não substitui UI, credenciais de produto nem workbenches.
- Integrações remotas pagas ficam opt-in; registrar versão/modelo quando
  testes reais forem executados.
- Release completa exige instalação limpa, matriz de plataformas, segurança,
  versionamento e publicação reproduzível, conforme o plano da EPIC.

## Critérios de aceite

- Request/response, erros, timeouts, retry, cancelamento e streaming testados no
  Rust e nos bindings afetados.
- Fixtures compartilham casos e deixam diferenças de protocolo explícitas.
- Não há segredo em logs/fixtures; falhas parciais não são repetidas.
- Matriz de suporte por provider/operação/modalidade tem evidência atual e
  limites visíveis.
- Mudanças de adoção incluem comparação de comportamento e caminho reversível.

## Referências

- [`docs/specs/03-chat-openai-compatible.md`](../specs/03-chat-openai-compatible.md)
- [`docs/specs/05-native-chat-providers.md`](../specs/05-native-chat-providers.md)
- [`docs/specs/18-providers-e-modalidades-llama-cpp-v1.md`](../specs/18-providers-e-modalidades-llama-cpp-v1.md)
- [`docs/specs/17-midia-no-chat.md`](../specs/17-midia-no-chat.md)
- [`docs/plans/03-plano-continuidade.md`](03-plano-continuidade.md)
