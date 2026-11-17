# Auditoria histórica do cliente OpenAI-compatible

**Base examinada:** `main` em `22c861f` (11 de novembro de 2026). As correções descritas abaixo foram implementadas após a auditoria; este é um
registro histórico de cinco findings do cliente OpenAI-compatible e do fluxo
TypeScript/SCons. Revalide contra o código atual antes de reutilizar como
status. Os findings serviram de pistas, não de especificação.

| Finding                                                     | Estado observado na auditoria                                                                                                                                                      | Evidência e tratamento                                                                                                                                                                                |
| ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Seguir redirects HTTP automaticamente                       | Reproduzido. `Client::builder()` não definia política de redirect.                                                                                                                 | A correção observada desativa redirects; teste local confirma que 302 retorna `ChatError::Provider { status: 302 }`.                                                                                  |
| API key inválida como valor de header                       | Reproduzido. A validação rejeitava chave vazia, mas não o valor completo `Bearer ...`.                                                                                             | A correção observada valida com `HeaderValue` antes de construir o cliente; teste cobre newline.                                                                                                      |
| Serializar `description: None` e `detail: None` como `null` | Reproduzido em `request_payload` e `content_payload`.                                                                                                                              | A correção observada omite as chaves opcionais e testa presença/ausência no JSON.                                                                                                                     |
| Tratar `usage: null` e `tool_calls: null` como ausentes     | Já corrigido na revisão auditada.                                                                                                                                                  | `parse_usage`, `parse_tool_calls` e o delta do stream filtram `null`; `null_optional_usage_and_tool_calls_are_absent_in_completion_and_stream` cobre resposta e evento.                               |
| Typecheck limpo depende de build N-API anterior             | Parcialmente reproduzido. `scons ci` já executa build antes do lint, mas `scons lint` e o hook pre-commit de TypeScript podem rodar sem o `.d.ts` gerado; `scons clean` o apagava. | A correção observada mantém `native/index.d.ts` versionado como contrato de tipos gerado e faz `scons clean` preservar a declaração; validação TypeScript independente confirma o checkout sem build. |

## Fora de escopo e limites

- Esta auditoria não altera semântica de providers, retry, fallback ou
  autenticação além da validação local do valor usado no header.
- O host continua responsável por fornecer, proteger e rotacionar credenciais.
- A declaração N-API versionada deve permanecer alinhada ao output de
  `napi build`; mudanças de superfície nativa precisam atualizar esse arquivo.
- Não foi executado CI do GitHub. Resultados locais daquela execução estão preservados no checklist histórico;
  eles não substituem verificações no checkout atual.
