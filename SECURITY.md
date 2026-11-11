# Política de segurança

Não publique detalhes de uma vulnerabilidade em uma issue pública. Use um
[GitHub Security Advisory privado](https://github.com/vectora-company/engine/security/advisories/new)
para reportar o problema. Inclua o commit afetado, o impacto observado e os
passos necessários para reproduzi-lo, removendo tokens, dados pessoais e
segredos.

O projeto confirmará o recebimento pelo próprio advisory e combinará por lá a
validação, correção e divulgação coordenada. Não inclua exploit funcional nem
credenciais reais no repositório.

Mudanças no núcleo devem manter as fronteiras de confiança explícitas, validar
entradas em cada binding, evitar unsafe sem revisão e preservar autorização,
cancelamento e tratamento de erros dos contratos consumidores.
