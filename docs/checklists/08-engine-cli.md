# Checklist — engine-cli

- [x] Crate `engine-cli` membro do workspace; rustyline 19.0.0, termimad 0.35.5 e indicatif 0.18.6 foram fixadas no lockfile como stable compatível com MSRV 1.95.
- [x] Config `init/show/validate/set`, com caminho configurável e referência de segredo por environment variable; smoke manual de init/set/validate/show passou.
- [x] Cliente de chat reaproveita `LlamaServerClient` ou `OpenAiCompatibleClient`.
- [x] Loop ReAct preserva mensagens e tool call IDs, limita iterações e executa somente ferramenta local allowlisted (`calculator`).
- [x] Evaluator da calculator rejeita código, sintaxe inválida, divisão por zero e resultado não finito; testes validam precedência e rejeições.
- [x] Decision Model configura separadamente `LayaClient` (`laya-serve`) e `LlamaServerClient` (`/v1/systemone`, llama.cpp/Laya-GGUF); teste valida seleção de ambos os transportes.
- [x] Interface utiliza rustyline, termimad e indicatif; EOF/interrupção termina a conversa.
- [x] `cargo test -p engine-cli --locked`: 5 testes passaram; CLI `--help` e smoke da configuração passaram.
- [x] `scons tests`: 242 testes Engine, 5 testes CLI, 3 fixtures, 1 fronteira Python; pytest 99/1 skip e Node 52/11 skips opcionais.
- [x] `scons lint`.
- [x] Pre-commit dos arquivos da etapa.
- [x] `scons build-cli` gerou o binário otimizado; `engine-cli --help` executou.
- [x] Spec, plano, análise, checklist, índices e documentos do goal-loop referenciam a capability.
- [ ] Commit, push GitHub e PR filho de #120.
