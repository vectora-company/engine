# Fallback de chat

**Estado: implementado como capability do núcleo Rust; adoção pelos produtos não incluída.**

Esta capability tenta uma lista ordenada e explícita de modelos quando uma
chamada falha por motivos elegíveis. A implementação é compartilhada pelo
crate Rust e pelas bindings Python e TypeScript/N-API do bootstrap.

## Responsabilidades

- O consumidor escolhe a ordem e fornece apenas candidatos configurados e
  elegíveis, incluindo protocolo, endpoint, credenciais e timeouts de cada um.
- A engine valida os identificadores `provider:model`, ignora IDs repetidos e
  remove candidatos do mesmo provider do primeiro candidato.
- A engine não lê variáveis de ambiente, settings, catálogo, assinatura,
  disponibilidade de credenciais nem política de produto. Não escolhe um
  fallback por conta própria.
- A mesma requisição de chat é enviada aos candidatos em ordem; somente o
  campo `model` muda para o nome extraído do identificador do candidato.

## Erros e política

Em qualquer candidato, o fallback ocorre para quota, rate limit, conexão,
timeout, indisponibilidade temporária do provider (503/529 ou mensagem de
sobrecarga reconhecida), incompatibilidade conhecida de `tool_plan` e
incompatibilidade explícita de imagem/visão. Erros de autenticação, contrato,
protocolo ou configuração propagam sem tentar outro provider. Uma falha
elegível posterior à primeira tentativa pode avançar para o próximo candidato;
erros permanentes nunca são convertidos em `FallbackExhausted`.

Em completion, uma resposta bem-sucedida inclui `selected_model_id`. Se todos
os candidatos elegíveis falharem, retorna `FallbackExhausted` com o último
modelo tentado; incompatibilidade multimodal final preserva seu erro tipado.
Os corpos HTTP são limitados e inspecionados apenas para classificar alguns
erros conhecidos; não são incluídos em erros ou logs.

## Streaming e cancelamento

O consumidor recebe `model_switched` quando a engine passa a tentar o próximo
candidato. A troca só é permitida antes de qualquer evento da resposta do
provider. Depois do primeiro evento de conteúdo, usage ou conclusão, qualquer
falha é propagada e o texto parcial não é repetido nem concatenado com outra
geração. Descartar/cancelar o stream descarta a chamada ativa e interrompe o
transporte HTTP pelo caminho de cancelamento existente das bindings.

## Conformidade

Fixtures locais devem exercitar a ordem configurada, classificação de erro,
modelo selecionado, evento de troca, preservação de saída parcial, ausência de
chamada ao próximo provider após saída parcial e cancelamento. Os mesmos
contratos devem ser exercitados via API Rust, binding Python e binding
TypeScript; testes não usam credenciais nem serviços externos.

Esta API não substitui a política de seleção de providers do Vectora nem
implementa descoberta automática de credenciais ou modelos. A adoção pelo
Vectora Backend e pelo GH Bot é uma etapa posterior, reversível e própria de
cada consumidor.

## Auditoria da implementação atual

O código Rust corresponde à classificação acima: quota (402), rate limit (429),
falha de conexão, timeout, overload reconhecido, incompatibilidade de
`tool_plan`, incompatibilidade multimodal explícita e respostas 503/529 são
elegíveis. Autenticação, demais erros de provider/servidor, resposta inválida,
protocolo e cancelamento não tentam outro candidato. O fluxo de streaming
propaga falha depois de emitir qualquer evento e mantém a resposta parcial sem
replay.

Python e TypeScript têm testes locais com servidores HTTP de fixture para troca
antes da saída e ausência de replay após saída parcial. A fixture comum
`tests/fixtures/conformance-v1.json` (chave `fallback_chat`) é exercitada pelos
três níveis, inclusive em Rust com servidores TCP locais
(`crates/vectora-engine/tests/conformance_fixtures.rs`,
`shared_fallback_fixture_conforms_to_completion_and_streaming`), e cobre o
cancelamento da iteração do stream (`cancel_after_event_type`). Ainda falta na
fixture um caso de erro não elegível, e interromper a leitura não comprova o
tempo de fechamento do socket upstream. Esses limites impedem afirmar
conformidade completa da capability.
