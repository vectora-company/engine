#!/usr/bin/env bash
# Prepara uma máquina Linux para desenvolver a Vectora Engine.
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
MODE=configure
case "${1:-}" in
  "") ;;
  --check) MODE=check ;;
  -h|--help)
    printf '%s\n' 'Uso: ./dotfiles.sh [--check]' \
      '  padrão             configura dependências globais do host Linux' \
      '  --check            apenas verifica as dependências globais do host' \
      '  não instala dependências do projeto nem chama SCons'
    exit 0
    ;;
  *) printf 'Opção desconhecida: %s\n' "$1" >&2; exit 2 ;;
esac

if [[ "$(uname -s)" != Linux ]]; then
  printf 'Este bootstrap prepara Linux; sistema detectado: %s.\n' "$(uname -s)" >&2
  exit 1
fi
if [[ ! -r /etc/os-release ]]; then
  printf 'Não foi possível identificar a distribuição: /etc/os-release ausente.\n' >&2
  exit 1
fi
# shellcheck disable=SC1091
. /etc/os-release
if [[ -e /run/ostree-booted ]]; then
  printf 'Sistema imutável detectado. Instale as dependências em um toolbox/container.\n' >&2
  exit 1
fi
if (( EUID == 0 )); then
  printf 'Execute como usuário normal; o script solicita sudo ou doas quando necessário.\n' >&2
  exit 1
fi

DISTRO=${ID:-unknown}
DISTRO_LIKE=${ID_LIKE:-}
case "$DISTRO" in
  ubuntu|pop|debian|linuxmint|elementary) PACKAGE_MANAGER=apt ;;
  fedora|fedora-asahi-remix) PACKAGE_MANAGER=dnf ;;
  arch|manjaro|endeavouros) PACKAGE_MANAGER=pacman ;;
  *)
    case " $DISTRO_LIKE " in
      *" debian "*|*" ubuntu "*) PACKAGE_MANAGER=apt ;;
      *" fedora "*|*" rhel "*) PACKAGE_MANAGER=dnf ;;
      *" arch "*) PACKAGE_MANAGER=pacman ;;
      *)
        printf 'Distribuição não mapeada (%s; ID_LIKE=%s). Consulte CONTRIBUTING.md.\n' \
          "$DISTRO" "${DISTRO_LIKE:-ausente}" >&2
        exit 1
        ;;
    esac
    ;;
esac

NODE_MAJOR=24
PYTHON_SERIES=3.13
NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
UV_TOOL_BIN_DIR="${UV_TOOL_BIN_DIR:-$HOME/.local/bin}"
export NVM_DIR
export UV_TOOL_BIN_DIR
export PATH="$HOME/.cargo/bin:$UV_TOOL_BIN_DIR:$HOME/.pyenv/bin:$HOME/.pyenv/shims:$PATH"
for node_bin in "$NVM_DIR"/versions/node/v"$NODE_MAJOR"*/bin; do
  [[ -d "$node_bin" ]] && { PATH="$node_bin:$PATH"; break; }
done
export PATH

command_exists() { command -v "$1" >/dev/null 2>&1; }
has_uv_tool() {
  command_exists uv && uv tool list 2>/dev/null | awk -v name="$1" '$1 == name { found=1 } END { exit !found }'
}
system_packages() {
  case "$PACKAGE_MANAGER" in
    apt)
      printf '%s\n' build-essential ca-certificates curl git pkg-config cmake \
        libssl-dev zlib1g-dev libbz2-dev libreadline-dev libsqlite3-dev libncurses-dev \
        xz-utils tk-dev libxml2-dev libxmlsec1-dev libexpat1-dev uuid-dev libffi-dev \
        liblzma-dev libgdbm-dev libnss3-dev protobuf-compiler podman
      ;;
    dnf)
      printf '%s\n' gcc gcc-c++ make ca-certificates curl git pkgconf-pkg-config cmake \
        openssl-devel zlib-devel bzip2-devel readline-devel sqlite-devel ncurses-devel \
        xz-devel tk-devel libffi-devel libxml2-devel xmlsec1-devel expat-devel \
        libuuid-devel gdbm-devel nss-devel protobuf-compiler podman
      ;;
    pacman)
      printf '%s\n' base-devel ca-certificates curl git pkgconf cmake openssl zlib bzip2 \
        readline sqlite ncurses xz tk libxml2 xmlsec expat util-linux-libs libffi gdbm \
        protobuf podman
      ;;
  esac
}
package_is_installed() {
  case "$PACKAGE_MANAGER" in
    apt) dpkg-query -W -f='${db:Status-Abbrev}' "$1" 2>/dev/null | grep -q '^ii' ;;
    dnf) rpm -q "$1" >/dev/null 2>&1 ;;
    pacman) pacman -Q "$1" >/dev/null 2>&1 ;;
  esac
}
report() {
  local label=$1 status=$2 detail=${3:-}
  if [[ "$status" == ok ]]; then
    printf '  [ok]       %-28s %s\n' "$label" "$detail"
  else
    printf '  [pendente] %-28s %s\n' "$label" "$detail"
    ((PENDING += 1))
  fi
}

PNPM_VERSION=$(sed -nE 's/.*"packageManager"[[:space:]]*:[[:space:]]*"pnpm@([^"]+)".*/\1/p' \
  "$ROOT_DIR/bindings/typescript/package.json")
if [[ -z "$PNPM_VERSION" ]]; then
  printf 'Não foi possível ler a versão do pnpm em bindings/typescript/package.json.\n' >&2
  exit 1
fi

printf 'Vectora Engine Linux bootstrap — distro=%s (%s), modo=%s\n' \
  "$DISTRO" "$PACKAGE_MANAGER" "$MODE"
printf '\nToolchain e dependências do projeto:\n'
PENDING=0
MISSING_PACKAGES=()
while IFS= read -r package; do
  package_is_installed "$package" || MISSING_PACKAGES+=("$package")
done < <(system_packages)
if ((${#MISSING_PACKAGES[@]})); then
  report 'pacotes de sistema' missing "${MISSING_PACKAGES[*]}"
else
  report 'pacotes de sistema' ok 'dependências de build instaladas'
fi
for tool in git curl cmake pkg-config podman rustup cargo uv scons pre-commit; do
  if command_exists "$tool"; then report "$tool" ok "$(command -v "$tool")"
  else report "$tool" missing; fi
done

if command_exists protoc; then
  report 'Protocol Buffers compiler' ok "$(protoc --version)"
else
  report 'Protocol Buffers compiler (protoc)' missing
fi
if command_exists node && [[ "$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || true)" == "$NODE_MAJOR" ]]; then
  report "Node.js $NODE_MAJOR" ok "$(node --version)"
else
  report "Node.js $NODE_MAJOR via nvm" missing
fi
if command_exists pnpm && [[ "$(pnpm --version 2>/dev/null || true)" == "$PNPM_VERSION" ]]; then
  report "pnpm $PNPM_VERSION" ok "$(command -v pnpm)"
else
  report "pnpm $PNPM_VERSION" missing
fi
if command_exists pyenv && pyenv versions --bare 2>/dev/null | grep -Eq "^${PYTHON_SERIES//./\\.}\\.[0-9]+$"; then
  report "Python $PYTHON_SERIES via pyenv" ok "$(pyenv versions --bare | grep -E "^${PYTHON_SERIES//./\\.}\\.[0-9]+$" | tail -n 1)"
else
  report "Python $PYTHON_SERIES via pyenv" missing
fi
if command_exists rustup && rustup component list --toolchain stable --installed 2>/dev/null | grep -q '^rustfmt-'; then
  report 'Rust stable + rustfmt' ok "$(rustup run stable rustc --version 2>/dev/null || true)"
else
  report 'Rust stable + rustfmt' missing
fi
if command_exists rustup && rustup component list --toolchain stable --installed 2>/dev/null | grep -q '^clippy-'; then
  report 'Rust Clippy' ok 'componente stable instalado'
else
  report 'Rust Clippy' missing
fi
for tool in scons pre-commit; do
  if has_uv_tool "$tool"; then report "uv tool: $tool" ok 'instalado'
  else report "uv tool: $tool" missing; fi
done

if [[ "$MODE" == check ]]; then
  if (( PENDING )); then printf '\n%d requisito(s) pendente(s).\n' "$PENDING"; exit 1; fi
  printf '\nToolchain Linux pronta para Vectora Engine.\n'
  exit 0
fi

if command_exists sudo; then SUDO=(sudo)
elif command_exists doas; then SUDO=(doas)
else
  printf 'Instale sudo ou doas para instalar dependências de sistema.\n' >&2
  exit 1
fi
run_privileged() { "${SUDO[@]}" "$@"; }

install_system_packages() {
  local packages=() package
  while IFS= read -r package; do
    package_is_installed "$package" || packages+=("$package")
  done < <(system_packages)
  if ((${#packages[@]} == 0)); then return 0; fi
  case "$PACKAGE_MANAGER" in
    apt)
      run_privileged apt-get update
      run_privileged env DEBIAN_FRONTEND=noninteractive apt-get install -y "${packages[@]}"
      ;;
    dnf) run_privileged dnf install -y "${packages[@]}" ;;
    pacman) run_privileged pacman -S --needed --noconfirm "${packages[@]}" ;;
  esac
}

install_system_packages

if [[ ! -s "$NVM_DIR/nvm.sh" ]]; then
  mkdir -p "$NVM_DIR"
  curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
fi
# nvm modifies shell state and assumes nounset is disabled while sourced.
set +u
# shellcheck disable=SC1090,SC1091
. "$NVM_DIR/nvm.sh"
set -u
nvm install "$NODE_MAJOR"
nvm alias default "$NODE_MAJOR" >/dev/null
nvm use "$NODE_MAJOR" >/dev/null
PNPM_CURRENT=$(pnpm --version 2>/dev/null || true)
if [[ "$PNPM_CURRENT" != "$PNPM_VERSION" ]]; then
  npm install --global "pnpm@$PNPM_VERSION"
fi

if ! command_exists pyenv; then git clone https://github.com/pyenv/pyenv.git "$HOME/.pyenv"; fi
export PATH="$HOME/.pyenv/bin:$HOME/.pyenv/shims:$PATH"
eval "$(pyenv init - bash)"
PYTHON_VERSION="${VECTORA_ENGINE_PYTHON_VERSION:-$(pyenv latest -k "$PYTHON_SERIES")}"
if ! pyenv versions --bare | grep -Fxq "$PYTHON_VERSION"; then pyenv install "$PYTHON_VERSION"; fi
pyenv global "$PYTHON_VERSION"

if ! command_exists uv; then curl -LsSf https://astral.sh/uv/install.sh | sh; fi
export PATH="$UV_TOOL_BIN_DIR:$PATH"
uv python install "$PYTHON_VERSION"
uv tool install --upgrade scons
uv tool install --upgrade pre-commit

if ! command_exists rustup; then
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | \
    sh -s -- -y --profile minimal --default-toolchain stable --no-modify-path
fi
export PATH="$HOME/.cargo/bin:$PATH"
rustup toolchain install stable --profile minimal
rustup default stable
rustup component add rustfmt clippy rust-analyzer rust-src --toolchain stable

printf '\nConfiguração global do host concluída.\n'
printf 'Para instalar dependências do projeto e sidecars, execute scons install.\n'
printf 'Para compilar o projeto, execute scons build.\n'
