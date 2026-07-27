# Elagix — Pendências técnicas e melhorias

Consolidação de tudo que ficou marcado como "deixado pra depois" ao longo de M0-M8 (antes espalhado em `specs.md` §13 e nas notas de cada milestone em `MILESTONES.md`). Nada aqui bloqueia o uso atual — são lacunas conhecidas, não bugs escondidos.

## Plataforma / instalação

- **`install.ps1` não validado com ativação completa.** Testado só em modo seguro (cópia isolada, sem tocar no PATH real) — esta máquina de desenvolvimento não tem `git`/`cargo`/`npm` nativos no Windows (só existem via WSL), então não dava pra validar a interceptação de um binário nativo de verdade. Precisa rodar numa máquina Windows com toolchain nativa instalada.
- **macOS sem cross-compile.** Bloqueado por falta de SDK/Xcode (`cc: unrecognized -arch/-mmacosx-version-min`, evidência em MILESTONES.md M8). Precisa de um Mac real ou um runner de CI com macOS (ex.: GitHub Actions `macos-latest`) — nenhum dos dois configurado ainda.
- **Caminho `zsh` do `install.sh` nunca testado ao vivo** — só o `bash`, que é o shell real desta máquina. A lógica espelha a do bash (`~/.zprofile` + topo do `~/.zshrc`), mas não foi exercitada de verdade.
- **Sem pipeline de release.** Não existe repositório remoto, CI, nem binário publicado em lugar nenhum — `install.sh`/`install.ps1` sempre buildam a partir do código-fonte. Download de binário pronto é ideia de fase 2 (specs §3, plano original).
- **Repositório local sem nenhum commit** (`git status` mostra "No commits yet" desde o início do projeto). Todo o trabalho de M0-M8 existe só no working tree.
- **Achado crítico já corrigido, mas vale relembrar pra qualquer plataforma nova**: a ativação do shim depende inteiramente de COMO a ferramenta que se quer interceptar invoca shell de verdade (login vs interativo, etc. — ver `specs.md` §5.1 e `MILESTONES.md`, seção "Correção crítica pós-M8"). Antes de declarar "pronto" numa plataforma nova, precisa confirmar experimentalmente o padrão de invocação lá, não presumir que generaliza do WSL/Linux.

## Cobertura de comandos (Camada B / `bornes/comandos`)

- **Só 4 filtros TOML de exemplo existem** (`docker-images`, `git-branch`, `terraform-plan`, `npm-install`). A auditoria do RTK (specs §10) mapeou ~60 comandos de cauda longa que nunca ganharam regra: `go-build`, `tsc`, `rg`, `make`, `jq`, `poetry`, `uv`, `mise`, `jj`, `nx`, `turbo`, `pre-commit`, `grep`, `fd`, `tree`, `wc`, `df`, `stat`, `shellcheck`, `yamllint`, `oxlint`, `ruff-format`, `cargo-clippy`, `ls-la`, `golangci-lint`, entre outros.
- **Ordem do pipeline da Camada B é fixa**, não configurável por filtro individual (sempre `strip_ansi → replace → match_output → keep/strip_lines → dedup → truncate_lines → max_lines → on_empty`, ver `bornes/comandos/camada_b/engine.rs`). Nunca precisou mudar pros 4 filtros existentes, mas pode não servir pra todo caso futuro.
- **Ações do catálogo (specs §5.3) ainda não implementadas**: `group_by`, `json_extract`/`json_schema`/`ndjson_stream`, `regex_extract`, `state_machine`, `aggregate`, `format_template`, `compact_path`. Nenhum dos 4 filtros do v1 precisou delas ainda.
- **Sem parser de `read`/`smart`** (leitura de arquivo) na Camada A — bloqueia o caso de uso "resumir docstring/comentário longo" que `bornes/prosa` poderia cobrir (specs §13, item já resolvido como "fora do v1" por falta de onde plugar).

## Cache, disclosure progressivo e dedup (`core/store`)

- **Cache (specs §8.2) só cobre `git show <sha explícito>`.** Cache de working-tree (`git status`/`git diff` sem commit fixo, precisaria checar mtime de `.git/index`) e de leitura de arquivo ficaram de fora — nenhum dos dois tem parser de Camada A pra se apoiar ainda.
- **Dedup (specs §8.3) aproxima "sessão" por janela de tempo** (`ELAGIX_DEDUP_WINDOW_SECS`, default 1.800s), não por um id de sessão real — o shim não tem acesso a nenhum identificador estável do Claude Code. Pode deduplicar entre duas sessões próximas no tempo, ou deixar de deduplicar numa sessão muito longa com gaps grandes.
- **Política de limpeza (14 dias, varredura probabilística ~2%) nunca testada em escala real** — só com o volume pequeno gerado nesta sessão de desenvolvimento.

## `bornes/mcp`

- **Só testado contra um servidor MCP fake** (fixture própria, `fake_mcp_server.py`), nunca contra um servidor de produção real. Mecanismo validado, compatibilidade real não.
- **Só stdio.** OAuth e streaming HTTP remoto não suportados (specs §13, decisão explícita de escopo v1).
- **Sem poda de campo por relevância semântica** (paginação, links HATEOAS, timestamps redundantes) — só as 3 técnicas puramente mecânicas (null-strip, truncamento de string, cap de array). Podar por relevância dependeria de conhecer a API específica, o que contrariaria a regra de negócio 5.
- **Requests que o próprio servidor MCP inicia** (ex.: `sampling/createMessage`) passam direto sem interceptação nem compressão — não é o eixo de desperdício de token que motivou o borne, mas também não foi endereçado.
- **Sem tratamento explícito de crash do processo filho** no meio da sessão (o que acontece com o proxy se o servidor real morrer no meio de uma chamada pendente não foi exercitado).

## `bornes/prosa`

- **Divisor de frases é ingênuo**: corta em `.`/`!`/`?` seguido de espaço, sem tratar abreviações (`Sr.`, `v1.2`). Aceitável pro caso de uso real (corpo de commit, prosa curta), ruim pra texto denso de abreviações.
- **TF-IDF pontua por raridade estatística de palavra, não "importância" intuitiva** — validado ao vivo que o resumo às vezes escolhe uma frase que um humano não escolheria primeiro (`elagix compress`, MILESTONES.md M7). Comportamento esperado do algoritmo, não bug, mas vale lembrar ao interpretar um resumo.
- **`/compress` do usuário não usa `bornes/prosa`** — decisão deliberada (specs §7.2/§7.3, são tarefas diferentes), não uma lacuna a fechar.

## Qualidade / processo

- **`bytes/4` como estimativa de token**, nunca um tokenizer real — mesma aproximação que RTK/snip usam, seguida por consistência de comparação (specs §5.4.1), não por precisão. Tokenizer real é ideia de fase 2 (specs §11).
- **Determinismo (specs §8.4) nunca auditado formalmente** — assumido "por construção" (nenhuma camada usa timestamp/ordenação não-determinística de propósito), mas não existe um teste dedicado provando isso pra proteger o cache de prompt do provedor.
- **Sem teste de integração end-to-end no `cargo test`** — toda validação do binário compilado (shim ao vivo, cache de `git show`, ativação de PATH) foi manual/live nesta sessão, não faz parte da suíte automatizada.
- **Teto arquitetural conhecido e aceito, não um bug**: regras estáticas por comando são mensuravelmente piores que poda condicionada à tarefa/intenção do agente (arXiv 2604.04979/2604.19572, specs §11) — exigiria modelo treinado ou contexto de intenção repassado ao filtro, contra a filosofia determinística do projeto. Registrado, não perseguido.
- **Efeito de diluição** (specs §11): redução de token na saída de um comando não equivale a redução no custo total da sessão (prompt, histórico, system prompt também contam) — cuidado ao reportar economia de sessão inteira baseado só em economia por comando.

- **O binário `elagix` em si não está no `$PATH`** — só os nomes de shim (`git`, `cargo`, `pytest`, `docker`, `npm`, `terraform`) são symlinks/cópias em `~/.elagix/shims`. `elagix show`/`elagix store`/`elagix compress`/`elagix mcp` só são alcançáveis hoje pelo caminho completo do binário (`~/projects/elagix/target/release/elagix show ...`), não por um `elagix` nu. Achado ao vivo testando pós-reestruturação (2026-07-26) — não é regressão, já era assim desde o M4/M7, só não tinha sido percebido.

## Decisões ainda em aberto

- **Nome final "Elagix"** nunca formalmente confirmado como definitivo (specs §13) — segue sendo nome de trabalho.
