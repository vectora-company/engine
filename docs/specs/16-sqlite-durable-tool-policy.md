# Persistência SQLite de políticas e aprovações de tools

**Estado:** experimental, contrato do store `1`; extensão durável do contrato
de política e aprovação descrito em
[`12-policy-and-approval.md`](12-policy-and-approval.md).

## Escopo

`SqliteToolPolicyStore` persiste snapshots confiáveis fornecidos pelo host,
avaliações de autorização, pedidos de aprovação e recibos vinculados a uma
chamada específica. O ledger e o checkpoint do estado são gravados juntos no
SQLite antes de o pedido de aprovação ou recibo persistido ser devolvido.
Bindings Python e TypeScript chamam o mesmo core Rust.

## Operações

- `save_snapshot` valida run e snapshot, salva o primeiro estado e reinicia a
  lista de pedidos/recibos quando a policy muda. Repetir snapshot idêntico é
  idempotente; reutilizar digest e revisão com conteúdo diferente é conflito.
- `load_latest` carrega o último checkpoint por escopo, run e turno e valida
  identidade, sequência e versão.
- `authorize` exige o estado mais recente, reavalia todo o lote contra o
  snapshot persistido, mantém apenas recibos ainda válidos e grava pedidos de
  aprovação antes de retorná-los.
- `record_approval` aceita apenas pedido persistido e recibo com binding
  correspondente; conflitos de estado ou de recibo são rejeitados.

Cada mutação recebe `operation_id` para idempotência do ledger. O estado tem
limite de 512 KiB. O escopo é obrigatório. Migrations, ledger e checkpoint são
os do adapter SQLite descrito na especificação de storage.

## Fronteira de confiança e limites

O consumidor autentica o usuário/aprovador e valida a origem do recibo antes de
chamar a engine. A engine confere formato, validade temporal e vínculo com
pedido, run, turno, escopo e digest da policy; não prova que a aprovação é
humana nem que o host exibiu uma solicitação. O store não executa adapters,
não encerra processos de tool, não fornece exactly-once para efeitos externos
e não transforma grants em assinaturas criptográficas.

Uma policy revisada exige novo snapshot. Após restart, o consumidor deve
recarregar o estado mais recente e reavaliar; não deve reutilizar cópia antiga
para autorizar efeitos. Resultados incertos de efeitos continuam sujeitos ao
protocolo de reconciliação do loop.

## Conformidade

Os testes devem cobrir persistência/restart, pedido pendente, recibo válido,
pedido incompatível, digest/revisão reutilizados, estado desatualizado,
conflito concorrente, expiração, limite de tamanho e idempotência de
`operation_id`. Testes dos bindings confirmam que as mesmas operações e erros
do core são acessíveis em Python e TypeScript.
