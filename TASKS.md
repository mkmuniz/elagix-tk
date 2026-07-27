# Elagix — Backlog de tasks

Cada item aqui vem de uma lacuna já registrada em `PENDENCIAS.md` (que explica o *porquê* de cada uma) — este arquivo é só a versão acionável, pra puxar quando for desenvolver. Sem prioridade/ordem implícita ainda.

## Plataforma / instalação

- [ ] Validar `install.ps1` com ativação completa numa máquina Windows com toolchain nativa de verdade (`git`/`cargo`/`npm` no PATH do Windows, não só WSL).
- [ ] Cross-compilar e testar Elagix em macOS de verdade — precisa de Mac físico ou runner de CI com macOS (ex.: GitHub Actions `macos-latest`).
- [ ] Testar ao vivo o caminho `zsh` do `install.sh` (`~/.zprofile` + topo do `~/.zshrc`) — hoje só o `bash` foi validado.
- [ ] Montar pipeline de release (CI + binário publicado) pra parar de depender de build a partir do código-fonte em toda instalação.
- [ ] Fazer o primeiro commit do repositório (hoje `git status` mostra "No commits yet" desde o M0).
- [ ] Repetir a checagem de "como o Claude Code invoca shell de verdade" (login/interativo etc.) em qualquer plataforma nova, antes de declarar ativação pronta lá — não presumir que generaliza do WSL/Linux.
- [ ] Adicionar o binário `elagix` em si ao PATH (hoje só os nomes-shim estão; `elagix show`/`store`/`compress`/`mcp` só funcionam pelo caminho completo).

## Cobertura de comandos (Camada B / `bornes/comandos`)

- [ ] Escrever filtros TOML pros comandos de cauda longa ainda sem regra (auditoria do RTK, specs §10): `go-build`, `tsc`, `rg`, `make`, `jq`, `poetry`, `uv`, `mise`, `jj`, `nx`, `turbo`, `pre-commit`, `grep`, `fd`, `tree`, `wc`, `df`, `stat`, `shellcheck`, `yamllint`, `oxlint`, `ruff-format`, `cargo-clippy`, `ls-la`, `golangci-lint`, entre outros.
- [ ] Avaliar se a ordem fixa do pipeline da Camada B (`strip_ansi → replace → match_output → keep/strip_lines → dedup → truncate_lines → max_lines → on_empty`) precisa virar configurável por filtro.
- [ ] Implementar as ações do catálogo (specs §5.3) ainda faltando: `group_by`, `json_extract`/`json_schema`/`ndjson_stream`, `regex_extract`, `state_machine`, `aggregate`, `format_template`, `compact_path`.
- [ ] Criar parser de Camada A pra leitura de arquivo (`read`/`smart`) — desbloqueia o caso de uso "resumir docstring/comentário longo" via `bornes/prosa`.

## Cache, disclosure progressivo e dedup (`core/store`)

- [ ] Estender cache pra comandos dependentes de working-tree (`git status`/`git diff` sem commit fixo — precisa checar mtime de `.git/index`).
- [ ] Avaliar cache de leitura de arquivo (chave: caminho + mtime + tamanho, ou hash de conteúdo).
- [ ] Trocar a janela de tempo do dedup por um id de sessão real, se/quando existir um jeito confiável de obter isso do Claude Code.
- [ ] Validar a política de limpeza (14 dias, varredura ~2% por escrita) em volume de uso real, não só o volume gerado em desenvolvimento.

## `bornes/mcp`

- [ ] Testar contra um servidor MCP de produção real (hoje só validado contra um servidor fake escrito pra teste).
- [ ] Automatizar ou pelo menos documentar formalmente o passo de reescrever a config de servidor MCP pra `elagix mcp -- <comando real>` — hoje é 100% manual e não está feito em nenhum servidor real.
- [ ] Suporte a OAuth e streaming HTTP remoto (hoje só stdio).
- [ ] Poda de campo por relevância semântica (paginação, links HATEOAS, timestamps redundantes) — hoje só as 3 técnicas mecânicas (null-strip, truncamento, cap de array).
- [ ] Tratar/comprimir requests iniciados pelo próprio servidor MCP (ex.: `sampling/createMessage`) — hoje passam direto.
- [ ] Testar e tratar o comportamento do proxy se o servidor real cair no meio de uma chamada pendente.

## `bornes/prosa`

- [ ] Melhorar o divisor de frases pra tratar abreviações comuns (`Sr.`, `v1.2`, etc.).
- [ ] Avaliar heurística de importância além de TF-IDF puro (ou aceitar formalmente a limitação atual — escolhe por raridade de palavra, não "importância" intuitiva).
- [ ] Considerar tokenizer real em vez da estimativa `bytes/4`.

## Qualidade / processo

- [ ] Escrever testes de integração end-to-end contra o binário compilado (hoje toda validação de comportamento real é manual/ao vivo, não faz parte do `cargo test`).
- [ ] Auditar determinismo formalmente (protege o cache de prompt do provedor, specs §8.4) — hoje é só "por construção", sem teste dedicado.
- [ ] Configurar CI (build + test cross-platform).
- [ ] Confirmar o nome final do projeto (ver conversa em andamento sobre naming).
