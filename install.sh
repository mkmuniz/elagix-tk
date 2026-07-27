#!/usr/bin/env bash
# Instalador Elagix — Linux/Mac/WSL (specs.md §5.1, M8).
#
# v1 (2026-07-26): builda a partir do código-fonte com `cargo`, não baixa
# binário pronto — não existe pipeline de release/CDN ainda (specs §13,
# decisão registrada em MILESTONES.md). Faz três coisas, todas em $HOME, sem
# sudo: (1) `cargo build --release`; (2) cria symlinks em `~/.elagix/shims/`
# pra cada comando da lista abaixo, todos apontando pro mesmo binário — quem
# decide o que filtrar é o `invoked_name` (argv[0]) dentro do próprio elagix,
# não o instalador; (3) garante `~/.elagix/shims` na frente do $PATH em DOIS
# arquivos, não um só — achado real testando ao vivo (2026-07-26): o Claude
# Code de fato invoca comando como `wsl -e bash -lc "..."` (login,
# NÃO-interativo). `~/.bashrc` sozinho (onde a v0 deste instalador colocava a
# linha) NUNCA roda nesse caso — o guard padrão "if not interactive, exit" no
# topo do `.bashrc` do Ubuntu retorna antes de chegar em qualquer linha
# anexada no fim do arquivo. bash usa arquivos diferentes conforme a
# combinação login/interativo, e nenhum arquivo sozinho cobre as duas
# combinações que importam aqui:
#   - login (interativo ou não, inclui `-lc`)        -> ~/.profile (ou ~/.zprofile)
#   - não-login mas interativo (ex.: `bash -ic`)      -> ~/.bashrc (ou ~/.zshrc)
# Por isso a linha vai nos DOIS, com o `.bashrc`/`.zshrc` recebendo a linha
# ANTES do resto do conteúdo (não no fim) — se ficasse depois de um guard de
# interatividade que o arquivo já tenha, não teria efeito no caso login+não-
# interativo mesmo estando lá.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SHIMS_DIR="${ELAGIX_SHIMS_DIR:-$HOME/.elagix/shims}"

# Comandos com filtro conhecido hoje (Camada A: git/pytest/cargo — specs §5.4;
# Camada B: docker/npm/terraform — filters-toml/*.toml). Adicionar um comando
# novo aqui não precisa de código novo se já existir uma regra Camada B pra
# ele; só precisa de um symlink a mais.
DEFAULT_COMMANDS=(git cargo pytest docker npm terraform)

if ! command -v cargo >/dev/null 2>&1; then
    echo "elagix: precisa do cargo (Rust) instalado — https://rustup.rs" >&2
    exit 1
fi

echo "elagix: compilando (cargo build --release)..."
(cd "$SCRIPT_DIR" && cargo build --release)

BIN_PATH="$SCRIPT_DIR/target/release/elagix"
if [ ! -x "$BIN_PATH" ]; then
    echo "elagix: build terminou mas não achei o binário em $BIN_PATH" >&2
    exit 1
fi

mkdir -p "$SHIMS_DIR"
for cmd in "${DEFAULT_COMMANDS[@]}"; do
    ln -sf "$BIN_PATH" "$SHIMS_DIR/$cmd"
done
echo "elagix: shims criados em $SHIMS_DIR para: ${DEFAULT_COMMANDS[*]}"

# Detecta os arquivos certos pelo shell de login do usuário, não pelo shell
# que está rodando este script agora (que pode ser só "bash" via `sh install.sh`).
case "$(basename "${SHELL:-bash}")" in
    zsh) LOGIN_FILE="$HOME/.zprofile"; INTERACTIVE_FILE="$HOME/.zshrc" ;;
    *) LOGIN_FILE="$HOME/.profile"; INTERACTIVE_FILE="$HOME/.bashrc" ;;
esac

PATH_LINE="export PATH=\"$SHIMS_DIR:\$PATH\""

# Arquivo de login (~/.profile): append simples — convencionalmente não tem
# guard de interatividade, e cobre o caso `wsl -e bash -lc` de verdade.
if [ -f "$LOGIN_FILE" ] && grep -Fq "$SHIMS_DIR" "$LOGIN_FILE"; then
    echo "elagix: PATH já configurado em $LOGIN_FILE (nada a fazer)"
else
    {
        echo ""
        echo "# Elagix — shim de \$PATH (specs.md §5.1)"
        echo "$PATH_LINE"
    } >> "$LOGIN_FILE"
    echo "elagix: adicionado ao PATH em $LOGIN_FILE"
fi

# Arquivo interativo (~/.bashrc): prepend no TOPO do arquivo, de propósito —
# cobre o caso não-login+interativo (`bash -ic`), que não lê ~/.profile. Só
# fica antes de qualquer guard de interatividade que o arquivo já tenha se a
# nossa linha for a PRIMEIRA coisa no arquivo.
if [ -f "$INTERACTIVE_FILE" ] && grep -Fq "$SHIMS_DIR" "$INTERACTIVE_FILE"; then
    echo "elagix: PATH já configurado em $INTERACTIVE_FILE (nada a fazer)"
elif [ -f "$INTERACTIVE_FILE" ]; then
    TMP_FILE="$(mktemp)"
    {
        echo "# Elagix — shim de \$PATH (specs.md §5.1). Tem que vir ANTES de"
        echo "# qualquer guard de interatividade que o resto do arquivo já tenha"
        echo "# (ex.: o \"if not interactive, exit\" padrão do Ubuntu) — senão não"
        echo "# tem efeito no caso não-login+interativo (\`bash -ic\`)."
        echo "$PATH_LINE"
        echo ""
        cat "$INTERACTIVE_FILE"
    } > "$TMP_FILE"
    mv "$TMP_FILE" "$INTERACTIVE_FILE"
    echo "elagix: adicionado ao topo de $INTERACTIVE_FILE"
else
    { echo "# Elagix — shim de \$PATH (specs.md §5.1)"; echo "$PATH_LINE"; } > "$INTERACTIVE_FILE"
    echo "elagix: criado $INTERACTIVE_FILE com o PATH do elagix"
fi

echo ""
echo "elagix: instalado. Abra um terminal novo pra ativar (cobre uso interativo"
echo "e o jeito que o Claude Code invoca comando via shell de login)."
echo "elagix: teste com 'git status | cat' — se filtrar, funcionou."
