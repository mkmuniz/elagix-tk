# Elagix — Milestones

Cada milestone referencia a seção do `specs.md` que já resolve a técnica — aqui é só sequenciamento de entrega, não decisão nova.

- [x] **M0 — Scaffold**: projeto Rust em `~/projects/elagix` (raiz do repo, fora do `bench/`), `cargo build` limpo. *(feito, 2026-07-26)*
- [x] **M1 — Mecanismo de interceptação**: shim de PATH funcionando — TTY detection, resolução do binário real, passthrough vs captura. (specs §5.1) *(feito, 2026-07-26)*
- [x] **M2 — Camada A completa**: `git status`, `git log`, `git diff`/`git show`, `pytest`, `cargo test` — os 5 comandos validados na auditoria (specs §10). Todos testados ao vivo contra o RTK de verdade (ver seção "Validado" abaixo). (specs §5.4) *(feito, 2026-07-26)*
- [x] **M3 — Camada B (pipeline declarativo)**: motor de regras genérico pra cauda longa, formato TOML (decidido em specs §13). (specs §5.2/5.3) *(feito, 2026-07-26)*
- [x] **M4 — Reversibilidade, cache e dedup**: armazém endereçado por hash, disclosure progressivo, cache com invalidação por tipo, deduplicação entre chamadas. Política de limpeza de disco decidida antes de ligar. (specs §8) *(feito, 2026-07-26 — escopo v1: cache só pra `git show <sha>` explícito, ver detalhe abaixo)*
- [x] **M5 — `bornes/mcp`**: proxy JSON-RPC com lazy-loading de schema. (specs §6.1) *(feito, 2026-07-26)*
- [x] **M6 — `bornes/mcp` resultado**: compressão do resultado de chamada de ferramenta MCP, reusando técnicas JSON de §5.5. (specs §6.2) *(feito, 2026-07-26, implementado junto com o M5 — os dois compartilham o mesmo proxy)*
- [x] **M7 — `bornes/prosa`**: TF-IDF extrativo, integrado no corpo de commit (M2). (specs §7) *(feito, 2026-07-26 — escopo revisado durante a implementação: NÃO integra com `/compress`, ver detalhe abaixo)*
- [x] **M8 — Empacotamento**: instalador cross-platform, binários pra Windows/Linux/Mac. *(feito, 2026-07-26 — Windows/Linux completos e ativados de verdade; Mac com cross-compile explicitamente adiado, ver detalhe abaixo)*

## Validado ao vivo nesta sessão (2026-07-26)

Testado contra o repositório real `bastion-agent`, via shim instalado em `~/.elagix/shims/` (symlinks `git`/`pytest` apontando pro binário release):

| Comando | Bruto | Elagix | Redução | Observação |
|---|---|---|---|---|
| `git status` (branch limpo) | 174B | 28B | 84% | Idêntico ao RTK |
| `git log -5` | 9.313B | 204B | 97,8% | Mesma técnica do RTK (mantém 1º commit, corta resto) |
| `git show HEAD` (diff de "cargo fmt", 6 arquivos) | 32.001B | 5.740B | 82,1% | **Melhor que o RTK** (71,43% no mesmo diff, seção 9) |
| `pytest` (erro de import) | 3.245B | 106B | 96,7% | **Melhor que o RTK**: preserva `ModuleNotFoundError: ...` — RTK só diz "No tests collected", sem motivo |
| `cargo test --lib control_plane` | — | — | — | Resumo limpo extraído ("60 passed; 0 failed... 426 filtered out"), sem o log verboso de compilação |

Exit codes preservados em todos os casos (`pytest` corretamente sai com 2, não 0).

### Camada B (M3, validado ao vivo 2026-07-26)

Motor genérico em `src/camada_b/` — `FilterFile` (TOML) + `Step` (enum com tag `action`) + `engine::apply()`. Ações no v1: `strip_ansi`, `replace`, `match_output`, `keep_lines_matching`/`strip_lines_matching`, `dedup`, `truncate_lines`, `max_lines`, `on_empty` (as demais do catálogo §5.3 — `group_by`, `json_extract`/`json_schema`, `state_machine`, `aggregate`, `format_template`, `compact_path` — ficam pra quando algum comando do v1 realmente precisar). `match_output`/`on_empty` só disparam com `exit_code == 0`, mesma regra de negócio 2/3 da Camada A. 4 filtros de exemplo embutidos via `include_str!` (`filters-toml/*.toml`): `docker-images`, `git-branch`, `terraform-plan`, `npm-install`. Extensível sem recompilar via `$ELAGIX_FILTERS_DIR` (default `~/.elagix/filters/*.toml`).

| Comando | Bruto | Elagix | Redução | Observação |
|---|---|---|---|---|
| `git branch -a` (bastion-agent) | 353B | 316B | 10,5% | Removeu só `remotes/origin/HEAD -> origin/main`; lista não passou do teto de 25 linhas |
| `docker images` (21 imagens reais) | 1.782B | 1.234B | 30,8% | `max_lines(15)` cortou 7 imagens; nenhuma `<none>:<none>` presente pra `strip_lines_matching` agir |
| `terraform plan` (fixture local, 1 recurso) | 1.492B | 1.290B | 13,5% | Modesto de propósito: sem refresh de estado real nem recursos "sem mudança" nessa fixture pequena — confirma o teto arquitetural já documentado (specs §10.3, tipo de falha nº 3): regra de ruído específico só ajuda quando o ruído aparece de fato |

6 testes unitários novos em `src/camada_b/engine.rs` (total do projeto: 20 testes, todos passando).

**Quatro bugs reais achados e corrigidos testando ao vivo** (documentados como comentário no código, motivo de cada correção mantido junto):
1. **Recursão do shim** (`src/shim.rs`): a versão inicial tentava descobrir a própria pasta a partir de `argv[0]`, assumindo que o shell sempre passa o caminho completo — bash às vezes só passa o nome nu ("git"), causando o shim se encontrar de novo e filtrar a própria saída duas vezes. Corrigido: a pasta de shims agora é conhecida de antemão (`~/.elagix/shims` ou `$ELAGIX_SHIMS_DIR`), não descoberta.
2. **Filtro desligado no caso de maior valor** (`src/main.rs`): a regra "nenhum atalho de sucesso se o processo falhou" tinha virado, por engano, "só filtra se `exit_code == 0`" — isso desligava o filtro do pytest bem no caso que mais queríamos mostrar (falha de coleta de teste, que sai com código != 0). Corrigido: cada filtro é responsável por nunca fabricar sucesso; a filtragem em si roda sempre.
3. **`git diff` sem cabeçalho de commit** (`src/filters/git_diff.rs`): `git diff` (working tree) começa direto em "diff --git", sem o bloco de commit que `git show` tem antes — a detecção original só procurava `"\ndiff --git"` (com quebra de linha antes), falhando nesse caso real e caindo em fail-open sem filtrar nada.
4. **Corte no meio do diff deixava saída parecendo código quebrado** (`src/filters/git_diff.rs`): ao bater o teto de linhas alteradas por arquivo, a versão inicial continuava mostrando cabeçalhos `@@` e contexto dos hunks seguintes, só escondendo as linhas `+`/`-` — resultado parecia sintaxe quebrada (chamada de função incompleta, chave sem fechar). Corrigido pra parar de vez no primeiro excesso, com contagem clara do que foi omitido.

### M4 — armazém, cache, disclosure progressivo, dedup (validado ao vivo 2026-07-26)

Três decisões pendentes resolvidas antes de implementar (specs §13): armazém em disco (`~/.elagix/store/`, layout arquivo-por-hash, sem índice em RAM), limpeza por expiração de 14 dias com varredura preguiçosa (~2% de chance por escrita, mais `elagix store clear`/`elagix store gc` manuais), e escopo do cache v1 restrito a `git show <sha explícito>` (único caso comprovadamente imutável sem heurística — `HEAD`/branch ficam de fora).

Módulo novo `src/store/` — CAS (`put`/`get`, sha256 truncado a 64 bits) + cache por chave (`put_keyed`/`get_keyed`, chave versionada `git-show:v1:<sha>` pra não servir saída obsoleta se o filtro mudar) + dedup por janela deslizante (`check_and_record_dedup`, limitação documentada em specs §8.3: aproxima "sessão" por tempo, não por id real) + `force_gc`/`clear_all`. Meta-comandos novos: `elagix show <hash>`, `elagix store clear`, `elagix store gc` — tratados em `main.rs` antes de qualquer resolução de shim (não existe "elagix real" no PATH).

| Mecanismo | Teste ao vivo | Resultado |
|---|---|---|
| Cache (`git show <sha>`, bastion-agent) | 1ª chamada roda de verdade, 2ª chamada pega do cache | 0,023s → 0,001s (~23×), saída byte-idêntica (`diff` confirmou) |
| Disclosure progressivo (`git show`, mesmo diff de 32.001B da validação do M2) | Saída filtrada (5.786B) ganhou a dica `(bruto completo: elagix show c8a886791a32083d)` | `elagix show c8a886791a32083d` recuperou os 32.001B originais exatos |
| Dedup (`git branch -a` repetido, janela de 60s) | 1ª chamada: 316B normais. 2ª chamada idêntica: colapsada | 316B → 75B (`(igual à saída anterior — elagix show ... pra ver de novo)`) |

Regra de negócio 6 (nunca inflar) e regra 2/3 (nenhum atalho de sucesso em processo não-bem-sucedido) verificadas no código: cache só grava com `exit_code == 0`; dedup só substitui se a mensagem de referência for mais curta que a saída original. 5 testes unitários novos em `src/store/mod.rs` (total do projeto: 25 testes, todos passando).

**Escopo explicitamente deixado pra depois** (não é lacuna, é decisão de fase): cache de working-tree (`git status`/`git diff` sem commit fixo, precisa checar mtime de `.git/index`) e de leitura de arquivo (não há parser de `read`/`smart` ainda) — specs §8.2 já cataloga os dois como próximos candidatos quando existir uma razão concreta pra priorizá-los.

### M5/M6 — `bornes/mcp`: proxy JSON-RPC (validado ao vivo 2026-07-26)

Diferente do shim de `bornes/comandos` (processo curto, uma chamada só), este é um processo **de vida longa**: `elagix mcp -- <comando do servidor real> [args...]` spawna o servidor MCP de verdade como filho e fica no meio da conversa inteira, sobre stdio (newline-delimited JSON-RPC, sem framing tipo LSP). Módulo novo `src/mcp_proxy/` — `mod.rs` (spawn + duas threads: uma repassa cliente→servidor interceptando `tools/call get_tool_schema` e `tools/list`/`tools/call` pendentes; a principal lê servidor→cliente e aplica a transformação certa por `id` de requisição), `schema.rs` (lazy-loading, specs §6.1) e `compress.rs` (compressão de resultado, specs §6.2).

**Lazy-loading de schema**: `tools/list` devolve wrappers mínimos (nome + primeira frase da descrição + `inputSchema` genérico `{"type":"object"}`) e injeta uma ferramenta sintética `get_tool_schema`. O schema completo original fica cacheado em memória (por processo, dura a sessão MCP); quando o modelo chama `get_tool_schema("X")`, o proxy responde **localmente**, sem nunca repassar essa chamada pro servidor real (que nem conhece essa ferramenta).

**Compressão de resultado**: só as 3 técnicas puramente mecânicas de specs §5.5 (nenhuma tenta adivinhar relevância semântica de campo): remove `null` recursivamente, corta string longa mantendo prefixo + contagem, capa array grande em 10 itens + marcador `_elagix_omitted_items`. Poda de campo por relevância (paginação, HATEOAS) fica de fora do v1 — dependeria de conhecer a API específica.

Testado ao vivo com um servidor MCP de teste próprio (`fake_mcp_server.py`, uma ferramenta `search_docs` com descrição de parágrafo inteiro e resultado com nulls/array de 30 itens/strings longas — fixture controlada, não um servidor de terceiro):

| Mensagem | Bruto | Elagix | Redução |
|---|---|---|---|
| `tools/list` (2 ferramentas verbosas) | 1.047B | 566B | 45,9% |
| `get_tool_schema("search_docs")` | — | 962B | recupera o schema **completo e exato** — round-trip validado |
| `tools/call("search_docs")` | 16.658B | 4.530B | 72,8% |

**Bug real achado e corrigido testando ao vivo**: deadlock de pipe no encerramento — o `Arc<Mutex<ChildStdin>>` tinha uma cópia extra presa no escopo de `run()` além da cópia da thread de encaminhamento; o stdin do processo filho só fecha de vez quando a ÚLTIMA cópia do `Arc` cai, então o servidor real (lendo stdin até EOF) nunca recebia esse EOF, nunca saía sozinho, e `child.wait()` travava pra sempre. Corrigido com um `drop()` explícito da cópia do escopo principal logo após spawnar a thread.

8 testes unitários novos (`schema.rs` + `compress.rs`, total do projeto: 32 testes, todos passando). Regra de negócio 6 verificada em três pontos: `transform_tools_list` e `compress_tools_call_result` comparam o tamanho da mensagem JSON-RPC inteira antes/depois e caem pro original se a transformação não render (teste `tiny_single_tool_falls_back_to_original` prova isso: 1 ferramenta minúscula não amortiza o custo fixo de injetar `get_tool_schema`).

**Escopo deixado pra depois**: OAuth e streaming HTTP remoto (specs §13 já cataloga isso como fora do v1); poda de campo por relevância semântica; requests que o PRÓPRIO servidor inicia (ex.: `sampling/createMessage`) passam direto sem interceptação, já que não é esse o eixo de desperdício de token que motivou o borne.

### M7 — `bornes/prosa` (validado ao vivo 2026-07-26)

Módulo novo `src/prosa/` — `summarize(text, max_sentences)`: TF-IDF clássico (tf normalizado por frase, idf suavizado `ln(N/df)+1`, stopwords EN+PT já que os commits deste projeto misturam os dois idiomas), extrai as frases de maior pontuação preservando a ORDEM ORIGINAL (não a ordem de score — resumo fora de ordem cronológica confundiria mais do que ajudaria). Fail-open: texto já dentro do teto de frases volta sem modificação.

Integrado nos dois lugares que já discartavam corpo de commit por completo:
- `filters/git_log.rs`: o corpo do primeiro commit agora aparece como `resumo: <frase>` (quando de fato encolheu) ou `corpo: <frase>` (quando já era 1 frase só, mostrado por completo em vez de rotulado como se tivesse sido comprimido — regra de negócio 5).
- `filters/git_diff.rs` (`git show`): achado real durante a implementação — o filtro descartava até o **hash e o assunto do commit** junto do corpo, não só o corpo. Ninguém tinha notado (a saída de `git show` filtrada nunca dizia qual commit era aquele diff). Corrigido: mantém `commit <hash> — <assunto>` + resumo do corpo antes dos hunks.

**Decisão revista durante a implementação** (specs §7.2/§7.3, §13): `/compress` **não** vira uma chamada a `bornes/prosa`, ao contrário do que a especificação anterior presumia. Reexaminando `~/.claude/commands/compress.md` de verdade na hora de integrar, ficou claro que são tarefas diferentes — `/compress` precisa cortar redundância DENTRO de cada frase preservando números/nomes/restrições (julgamento semântico), enquanto TF-IDF extrativo só sabe descartar frases INTEIRAS (arriscaria perder uma restrição que caiu numa frase de score baixo — indo contra a regra de negócio 5). `bornes/prosa` ganhou um utilitário standalone equivalente, `elagix compress` (lê stdin, resume, imprime — teto de frases automático de ~1/3 do original ou `--sentences N` explícito), útil pra prosa que tolera esse tipo de perda, mas não é o motor do slash command do usuário.

Testado ao vivo contra o repositório real `bastion-agent` (mesmo commit "cargo fmt" de 2 frases usado na validação do M2/M4):

| Comando | Antes do M7 | Depois do M7 |
|---|---|---|
| `git log -5` | corpo do commit descartado por completo, silenciosamente | `resumo: Never ran cargo fmt this session...` (a frase de maior sinal segundo TF-IDF, não a primeira por padrão) |
| `git show HEAD` | hash/assunto/corpo do commit todos descartados (perda não documentada até agora) | `commit cb93a3bd... — chore: cargo fmt (fix CI fmt-check failure)` + `resumo: ...` antes dos hunks |
| `elagix compress` (utilitário, 3 frases de teste) | N/A | escolheu a 3ª frase, não a 1ª — TF-IDF pontua por raridade de palavra, não por "importância" intuitiva; comportamento esperado do algoritmo, documentado como limitação conhecida, não bug |

6 testes unitários novos em `src/prosa/mod.rs` + 2 atualizados em `git_log.rs`/`git_diff.rs` pra refletir o novo comportamento (total do projeto: 38 testes, todos passando).

### M8 — Empacotamento (validado ao vivo 2026-07-26 — **Elagix ativado de verdade nesta sessão**)

Escopo v1: build a partir do código-fonte (`cargo build --release`), não download de binário pronto — não existe pipeline de release/CDN ainda, e não faz sentido fingir que existe. Dois instaladores na raiz do repo:

- **`install.sh`** (Linux/Mac/WSL): builda com `cargo`, cria symlinks em `~/.elagix/shims/{git,cargo,pytest,docker,npm,terraform}` (a lista cobre tudo que já tem filtro, Camada A + Camada B), garante `~/.elagix/shims` na frente do `$PATH` via `~/.bashrc`/`~/.zshrc` (idempotente — não duplica a linha rodando de novo).
- **`install.ps1`** (Windows nativo): não faz symlink de propósito (precisaria de modo desenvolvedor/admin) — copia o `.exe` pra cada nome de comando dentro de `~\.elagix\shims\`, já que o Elagix decide o que filtrar pelo NOME do arquivo (`argv[0]`), não por ser link ou cópia. Procura um `elagix.exe` já compilado (3 caminhos candidatos) antes de tentar compilar na hora; ajusta o PATH do usuário via `[Environment]::SetEnvironmentVariable(...,"User")`, também idempotente.

**Windows — cross-compilado e testado rodando DE VERDADE no PowerShell nativo** (não só `file`/inspeção estática): instalado `mingw-w64` + target `x86_64-pc-windows-gnu` no ambiente de build (WSL); `cargo build --release --target x86_64-pc-windows-gnu` compilou limpo (nenhuma dependência do projeto usa C/FFI, só crates Rust puros — `regex`/`serde`/`serde_json`/`toml`/`sha2` — por isso o mingw bastou, sem precisar de `cargo-zigbuild` nem nada mais elaborado). Copiado o `.exe` pro lado Windows e executado nativamente via PowerShell: `elagix compress`, `elagix show`, `elagix store gc` rodaram e saíram com o código de saída certo.

**Bug real achado e corrigido nesse teste**: `"texto" | elagix.exe compress` chegava com um BOM UTF-8 (`U+FEFF`) na frente do texto — artefato conhecido de como o PowerShell nativo encoda string literal ao mandar pro stdin de um processo, não um bug de lógica do Elagix. Corrigido com `strip_prefix('\u{feff}')` em `run_compress` (specs §7, `main.rs`) — remover BOM nunca perde conteúdo substantivo, então não violava regra de negócio 5.

**`install.ps1` só testado em modo seguro (cópia isolada), não com ativação completa**: esta máquina de desenvolvimento não tem `git`/`cargo`/`npm` nativos no PATH do Windows — todo o desenvolvimento acontece via WSL (confirmado com `Get-Command`, nenhum resolveu). Rodar o `install.ps1` completo aqui não teria efeito prático nenhum (não existe ferramenta nativa real pra interceptar). O script está correto e testado até onde dá nesta máquina; validação de ativação completa (PATH real + interceptação de um `git.exe`/`npm.exe` nativo de verdade) fica pendente até rodar numa máquina Windows com toolchain nativa instalada.

**Linux/WSL — ativado de verdade, não só testado**: `install.sh` rodado sem sandbox, `~/.bashrc` alterado de fato. Achado de metodologia (não de bug do produto): validar isso via `wsl -e bash -lc "..."` dá falso negativo — é shell de login NÃO-interativo, e o guard de "só roda o resto se for interativo" no topo do `.bashrc` padrão do Ubuntu barra a leitura da parte que a gente anexou. Testado certo via `bash -ic` (interativo, o que uma aba de terminal de verdade usa):

```
$ git status | cat
clean — nothing to commit
```

Confirmado: `which git`/`which cargo`/`which pytest` resolvem pra `~/.elagix/shims/`, e `git --version | cat` (comando sem filtro definido) passa direto sem modificação — só os subcomandos com filtro real são tocados.

**macOS — cross-compile adiado, com evidência concreta do motivo** (specs §9 já sinalizava isso como fricção conhecida da escolha de Rust): `rustup target add aarch64-apple-darwin` funciona, mas o link falha —

```
warning: invoking "xcrun" "--sdk" "macosx" "--show-sdk-path" ... failed: No such file or directory
error: linking with `cc` failed
cc: error: unrecognized command-line option '-arch'
cc: error: unrecognized command-line option '-mmacosx-version-min=11.0.0'
```

Precisa do SDK/Xcode de verdade (ou uma ferramenta tipo `cargo-zigbuild`/`osxcross`, nenhuma configurada) — não é algo pra resolver de graça num ambiente Linux puro. Deferido pra quando houver acesso a um Mac real ou um runner de CI com macOS (ex.: GitHub Actions `macos-latest`), nenhum dos dois configurado ainda (não existe repositório remoto/CI pra este projeto — só local, sem commits ainda).

### Correção crítica pós-M8: a ativação de ontem não valia pra como o Claude Code de fato roda comando (2026-07-26)

O usuário perguntou, com razão, "mas o objetivo é economizar token com as IAs, certo? por que 'qualquer terminal WSL novo'?" — isso expôs que a primeira ativação (linha só no `~/.bashrc`) **não tinha efeito nenhum** no jeito real que eu (Claude Code, rodando como extensão VSCode no Windows) invoco comando: sempre via `wsl -e bash -lc "..."` (shell de **login, não-interativo**). Confirmado ao vivo: `which git` mostrava o binário real, `$-` mostrava `hBc` (sem `i`).

Causa raiz: o `~/.bashrc` padrão do Ubuntu tem um guard no topo (`if not interactive, exit`) que descarta qualquer linha anexada no fim do arquivo quando o shell não é interativo — exatamente o caso de `-lc`. bash usa arquivos diferentes conforme a combinação login/interativo, e nenhum arquivo sozinho cobre as duas combinações que importam:
- login (interativo ou não, inclui `-lc`, o caso real) → `~/.profile`
- não-login mas interativo (ex.: `bash -ic`) → `~/.bashrc`

Duas tentativas intermediárias descartadas com evidência, não só por teoria:
1. **`/etc/environment`** (nível PAM, deveria valer pra tudo) — editado com sudo, mas confirmado que `wsl -e bash -lc` (modo `-e`/exec direto) **nem chega a consultar esse arquivo**: o `$PATH` observado não tinha nenhum traço do valor lá, mesmo depois de `wsl --shutdown` completo pra forçar releitura. Revertido.
2. Simplesmente mover a linha pro `.profile` sozinho — resolveu o caso real (`-lc`) mas **quebrou** o caso `bash -ic` (não-login+interativo, que não lê `.profile` nunca, é regra do próprio bash).

**Fix final, validado nas três combinações que existem de verdade** (`wsl -e bash -lc`, `wsl -e bash -ic`, e confirmado que só falta o caso teórico `bash -c` sem `-l` nem `-i`, que nenhuma dotfile cobre por design do bash — não é o padrão observado de invocação real, então não perseguido):
- `~/.profile`: linha simples no fim (convenção sem guard).
- `~/.bashrc`: linha no **topo do arquivo**, antes do guard de interatividade — não no fim.

`install.sh` reescrito pra fazer as duas coisas desde a primeira instalação (com a mesma lógica espelhada pra zsh via `~/.zprofile`/`~/.zshrc`, não testada ao vivo nesta sessão por falta de shell zsh disponível, mas mesma regra do bash se aplica).

```
$ wsl -e bash -lc 'git status | cat'
clean — nothing to commit
$ wsl -e bash -ic 'git status | cat'
clean — nothing to commit
```

**Lição maior que o bug em si**: "ativar o shim" não é uma checkbox binária — depende inteiramente de COMO a ferramenta que a gente quer interceptar (aqui, o próprio Claude Code) invoca shell de verdade. Vale checar isso explicitamente em qualquer plataforma nova antes de declarar M8 pronto lá também.

### Reestruturação de pastas: `core/` + `bornes/{comandos,mcp,prosa}/` (2026-07-26)

`src/` finalmente reflete o diagrama de arquitetura que o `specs.md` §3 já descrevia desde o início do projeto (módulos autocontidos numa pasta `bornes/`) — até aqui o código real estava mais achatado (`src/filters/`, `src/camada_b/`, `src/mcp_proxy/`, `src/prosa/`, `src/store/`, `src/shim.rs` todos soltos direto em `src/`).

```
src/
  main.rs              — ponto de entrada fino: só decide meta-comando (core::meta) vs shim (bornes::comandos)
  core/
    store.rs            — armazém endereçado por hash (specs §8), cross-cutting: hoje só bornes/comandos usa, mas é infra pros 3
    meta.rs              — roteamento de `elagix show/store/compress/mcp`
  bornes/
    comandos/            — o "tipo RTK": shim de $PATH + Camada A + Camada B
      shim.rs
      filters/           — Camada A (git_status, git_log, git_diff, pytest, cargo_test)
      camada_b/          — motor de pipeline declarativo (engine.rs) + tipos (mod.rs)
      filters-toml/       — dados das regras Camada B (movido de filters-toml/ na raiz)
      mod.rs              — orquestração (era o corpo grande do antigo main.rs)
    mcp/                  — compressor de MCP (era mcp_proxy/)
    prosa/                — resumo TF-IDF
```

Só 5 arquivos precisaram de mudança de conteúdo de verdade (o resto foi mover sem tocar): `main.rs` (reescrito, ficou fino), `camada_b/mod.rs` (`include_str!` de `../../filters-toml/` pra `../filters-toml/`, já que os dados moveram junto), `camada_b/engine.rs` (path do `Step` no teste), `filters/git_log.rs` e `filters/git_diff.rs` (path de `bornes::prosa::summarize`). `cargo build --release` limpo de primeira, 38 testes passando sem nenhuma mudança de asserção, e validado ao vivo de novo contra o `bastion-agent` — mesmo comportamento de antes da reestruturação (`git status`/`git log` filtrados igual, cache/disclosure/dedup intactos).

## Próximos passos
- **Todos os 8 milestones planejados (M0-M8) estão feitos, a ativação foi validada no padrão de invocação real do Claude Code, e a estrutura de pastas reflete a arquitetura documentada desde o início.**
- Ver `PENDENCIAS.md` (novo, 2026-07-26) pra lista completa e consolidada de dívida técnica e melhorias conhecidas — antes espalhada em notas soltas neste arquivo e no `specs.md` §13.
- Pendências reais, não bloqueantes: (1) validar `install.ps1` numa máquina Windows com toolchain nativa de verdade; (2) build/teste real em macOS (precisa de Mac físico ou CI); (3) o repositório do projeto ainda não tem nenhum commit (`git status` mostra "No commits yet") — vale organizar isso antes de ir mais longe; (4) observar o uso real por alguns dias e ver se algum filtro precisa de ajuste com dado de produção de verdade, não só fixture; (5) o caso `zsh` do `install.sh` não foi testado ao vivo (só o `bash`, que é o shell real desta máquina).
- Testar o shim em uso real (adicionar `~/.elagix/shims` no `$PATH` persistente, não só por chamada) e observar por alguns dias antes de avançar
- `bornes/mcp` também precisa de um teste ao vivo contra um servidor MCP real (não só o fake de teste) antes de considerar M5/M6 prontos pra uso de verdade — validado só o mecanismo, não a compatibilidade com servidores de produção
