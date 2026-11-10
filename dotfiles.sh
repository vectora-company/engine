#!/usr/bin/env bash
# Prepara a toolchain Linux necessária para desenvolver a Vectora Engine.
set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
TS_DIR="$ROOT_DIR/bindings/typescript"
PYTHON_MINOR=3.13
NODE_MAJOR=24
MODE=install
cd "$ROOT_DIR"

if (( $# > 1 )); then
  printf 'Use ./dotfiles.sh [--check|--toolchain-only]\n' >&2
  exit 2
fi

case "${1:-}" in
  "") ;;
  --check) MODE=check ;;
  --toolchain-only) MODE=toolchain ;;
  -h|--help)
    printf '%s\n' 'Uso: ./dotfiles.sh [--check|--toolchain-only]' \
      '  padrão             instala toolchain Linux e executa scons install' \
      '  --check            somente verifica dependências, sem instalar' \
      '  --toolchain-only   instala dependências sem executar o build do projeto'
    exit 0
    ;;
  *) printf 'Opção desconhecida: %s\n' "$1" >&2; exit 2 ;;
esac

if [[ "$(uname -s)" != Linux ]]; then
  printf 'Este bootstrap só configura Linux (sistema detectado: %s).\n' "$(uname -s)" >&2
  exit 1
fi
if [[ ! -r /etc/os-release ]]; then
  printf 'Não foi possível identificar a distribuição: /etc/os-release ausente.\n' >&2
  exit 1
fi
# shellcheck disable=SC1091
. /etc/os-release
DISTRO=${ID:-unknown}
if [[ -e /run/ostree-booted ]]; then
  printf '%s\n' 'Sistema imutável baseado em ostree detectado.' \
    'Não instalo pacotes no host; use um toolbox/container e rode scons install nele.' >&2
  exit 1
fi
if (( EUID == 0 )) && [[ "$MODE" != check ]]; then
  printf 'Execute como usuário normal; o script solicita sudo ou doas quando necessário.\n' >&2
  exit 1
fi

case "$DISTRO" in
  ubuntu|pop|debian|linuxmint|elementary|zorin|kali|raspbian) PACKAGE_MANAGER=apt ;;
  fedora|fedora-asahi-remix|rhel|centos|rocky|almalinux) PACKAGE_MANAGER=dnf ;;
  arch|omarchy|manjaro|endeavouros) PACKAGE_MANAGER=pacman ;;
  alpine) PACKAGE_MANAGER=apk ;;
  lfs|lebasix) PACKAGE_MANAGER=linuxbrew ;;
  *)
    case " ${ID_LIKE:-} " in
      *" debian "*|*" ubuntu "*) PACKAGE_MANAGER=apt ;;
      *" fedora "*|*" rhel "*) PACKAGE_MANAGER=dnf ;;
      *" arch "*) PACKAGE_MANAGER=pacman ;;
      *)
        printf 'Distribuição não mapeada: ID=%s ID_LIKE=%s.\n' "$DISTRO" "${ID_LIKE:-ausente}" >&2
        exit 1
        ;;
    esac
    ;;
esac

export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
export PYENV_ROOT="${PYENV_ROOT:-$HOME/.pyenv}"
export PNPM_HOME="${PNPM_HOME:-${XDG_DATA_HOME:-$HOME/.local/share}/pnpm}"
export PATH="$HOME/.local/bin:$PYENV_ROOT/bin:$PYENV_ROOT/shims:$HOME/.cargo/bin:$PNPM_HOME:$PATH"
for node_bin in "${NVM_DIR}/versions/node/v${NODE_MAJOR}"*/bin; do
  if [[ -d "$node_bin" ]]; then PATH="$node_bin:$PATH"; break; fi
done
export PATH

command_exists() { command -v "$1" >/dev/null 2>&1; }
has_uv_tool() {
  local installed_tools
  installed_tools=$(uv tool list 2>/dev/null | awk '{print $1}' || true)
  grep -Fxq "$1" <<< "$installed_tools"
}
has_protoc() {
  local executable=${PROTOC:-protoc}
  command -v "$executable" >/dev/null 2>&1 && "$executable" --version >/dev/null 2>&1
}

EXPECTED_PNPM=$(sed -n 's/.*"packageManager": "pnpm@\([^"]*\)".*/\1/p' "$TS_DIR/package.json")
if [[ -z "$EXPECTED_PNPM" ]]; then
  printf 'Não encontrei packageManager em %s/package.json.\n' "$TS_DIR" >&2
  exit 1
fi

printf 'Vectora Engine Linux bootstrap — distro=%s (%s), modo=%s\n' "$DISTRO" "$PACKAGE_MANAGER" "$MODE"
PENDING=0
check_command() {
  local name=$1
  if command_exists "$name"; then
    printf '  [ok]       %s: %s\n' "$name" "$(command -v "$name")"
  else
    printf '  [pendente] %s\n' "$name"
    ((PENDING+=1))
  fi
}

printf '\nFerramentas do projeto:\n'
for tool in git curl gcc make cmake pkg-config rustup cargo rustc rustfmt pyenv uv node pnpm scons pre-commit python; do
  check_command "$tool"
done
if command_exists cargo && cargo clippy --version >/dev/null 2>&1; then
  printf '  [ok]       cargo clippy\n'
else
  printf '  [pendente] cargo clippy\n'
  ((PENDING+=1))
fi

NODE_VERSION=$(node --version 2>/dev/null || true)
if [[ "$NODE_VERSION" == v${NODE_MAJOR}.* ]]; then
  printf '  [ok]       Node.js: %s\n' "$NODE_VERSION"
else
  printf '  [pendente] Node.js %s (detectado: %s)\n' "$NODE_MAJOR" "${NODE_VERSION:-ausente}"
  ((PENDING+=1))
fi
PNPM_VERSION=$(pnpm --version 2>/dev/null || true)
if [[ "$PNPM_VERSION" == "$EXPECTED_PNPM" ]]; then
  printf '  [ok]       pnpm: %s\n' "$PNPM_VERSION"
else
  printf '  [pendente] pnpm %s (detectado: %s)\n' "$EXPECTED_PNPM" "${PNPM_VERSION:-ausente}"
  ((PENDING+=1))
fi
PYENV_VERSIONS=$(pyenv versions --bare 2>/dev/null || true)
if command_exists pyenv && grep -Eq '^3\.13\.[0-9]+$' <<< "$PYENV_VERSIONS"; then
  printf '  [ok]       Python %s.x instalado no pyenv\n' "$PYTHON_MINOR"
else
  printf '  [pendente] Python %s.x instalado no pyenv\n' "$PYTHON_MINOR"
  ((PENDING+=1))
fi
PYTHON_ACTUAL=$(python --version 2>/dev/null | awk '{print $2}' || true)
if [[ "$PYTHON_ACTUAL" == "$PYTHON_MINOR".* ]]; then
  printf '  [ok]       Python: %s\n' "$PYTHON_ACTUAL"
else
  printf '  [pendente] Python %s.x (detectado: %s)\n' "$PYTHON_MINOR" "${PYTHON_ACTUAL:-ausente}"
  ((PENDING+=1))
fi
for tool in scons pre-commit; do
  if has_uv_tool "$tool"; then printf '  [ok]       uv tool: %s\n' "$tool"
  else printf '  [pendente] uv tool: %s\n' "$tool"; ((PENDING+=1)); fi
done
if [[ -s "$NVM_DIR/nvm.sh" ]]; then
  printf '  [ok]       nvm: %s\n' "$NVM_DIR/nvm.sh"
else
  printf '  [pendente] nvm (controlador de Node.js)\n'
  ((PENDING+=1))
fi
if has_protoc; then
  printf '  [ok]       Protocol Buffers compiler: %s (%s)\n' \
    "${PROTOC:-$(command -v protoc)}" "$(${PROTOC:-protoc} --version)"
else
  printf '  [pendente] Protocol Buffers compiler (protoc)\n'
  ((PENDING+=1))
fi
if command_exists rustup && rustup component list --installed 2>/dev/null | grep -q '^rust-analyzer-'; then
  printf '  [ok]       rust-analyzer\n'
else
  printf '  [pendente] rust-analyzer (componente Rust)\n'
  ((PENDING+=1))
fi

if [[ "$MODE" == check ]]; then
  if (( PENDING )); then printf '\n%d dependência(s) pendente(s).\n' "$PENDING"; exit 1; fi
  printf '\nToolchain Linux da Engine instalada.\n'
  exit 0
fi

if command_exists sudo; then SUDO=(sudo)
elif command_exists doas; then SUDO=(doas)
else SUDO=()
fi
run_privileged() {
  if ((${#SUDO[@]} == 0)); then
    printf 'Instale sudo ou doas para adicionar pacotes de sistema.\n' >&2
    return 1
  fi
  "${SUDO[@]}" "$@"
}

install_packages() {
  local packages=() package
  case "$PACKAGE_MANAGER" in
    apt) command_exists apt-get && command_exists dpkg-query ;;
    dnf) command_exists dnf && command_exists rpm ;;
    pacman) command_exists pacman ;;
    apk) command_exists apk ;;
    linuxbrew) : ;;
  esac || {
    printf 'O gerenciador de pacotes esperado (%s) não está disponível nesta distro.\n' "$PACKAGE_MANAGER" >&2
    return 1
  }
  case "$PACKAGE_MANAGER" in
    apt)
      local -a wanted=(build-essential ca-certificates curl git python3 pkg-config cmake protobuf-compiler \
        libssl-dev zlib1g-dev libbz2-dev libreadline-dev libsqlite3-dev libncursesw5-dev xz-utils \
        tk-dev libxml2-dev libxmlsec1-dev libffi-dev liblzma-dev libgdbm-dev)
      for package in "${wanted[@]}"; do
        dpkg-query -W -f='${db:Status-Abbrev}' "$package" 2>/dev/null | grep -q '^ii' || packages+=("$package")
      done
      if ((${#packages[@]})); then
        run_privileged apt-get update
        run_privileged env DEBIAN_FRONTEND=noninteractive apt-get install -y "${packages[@]}"
      fi
      ;;
    dnf)
      local -a wanted=(gcc gcc-c++ make ca-certificates curl git python3 pkgconf-pkg-config cmake protobuf-compiler \
        openssl-devel zlib-devel bzip2-devel readline-devel sqlite-devel ncurses-devel xz-devel tk-devel \
        libffi-devel libxml2-devel xmlsec1-devel gdbm-devel libuuid-devel)
      for package in "${wanted[@]}"; do rpm -q "$package" >/dev/null 2>&1 || packages+=("$package"); done
      if ((${#packages[@]})); then run_privileged dnf install -y "${packages[@]}"; fi
      ;;
    pacman)
      local -a wanted=(base-devel ca-certificates curl git python pkgconf cmake protobuf openssl zlib bzip2 \
        readline sqlite ncurses xz tk libxml2 xmlsec libffi gdbm util-linux)
      for package in "${wanted[@]}"; do pacman -Q "$package" >/dev/null 2>&1 || packages+=("$package"); done
      if ((${#packages[@]})); then run_privileged pacman -S --needed --noconfirm "${packages[@]}"; fi
      ;;
    apk)
      local -a wanted=(bash build-base ca-certificates curl git python3 pkgconf cmake protobuf-dev linux-headers \
        openssl-dev zlib-dev bzip2-dev readline-dev sqlite-dev ncurses-dev xz-dev tk-dev libxml2-dev xmlsec-dev \
        libffi-dev gdbm-dev util-linux-dev)
      for package in "${wanted[@]}"; do apk info -e "$package" >/dev/null 2>&1 || packages+=("$package"); done
      if ((${#packages[@]})); then run_privileged apk add "${packages[@]}"; fi
      ;;
    linuxbrew)
      if ! command_exists brew && [[ -x /home/linuxbrew/.linuxbrew/bin/brew ]]; then
        eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"
      fi
      if ! command_exists brew; then
        NONINTERACTIVE=1 /bin/bash -c "\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
        eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"
      fi
      local -a wanted=(gcc git curl pkgconf cmake protobuf openssl readline sqlite xz tcl-tk)
      for package in "${wanted[@]}"; do brew list --versions "$package" >/dev/null 2>&1 || packages+=("$package"); done
      if ((${#packages[@]})); then brew install "${packages[@]}"; fi
      ;;
  esac
}

install_packages
if ! has_protoc; then
  printf '%s\n' 'A instalação do pacote protobuf não disponibilizou um protoc executável no PATH.' >&2
  exit 1
fi

if [[ ! -s "$NVM_DIR/nvm.sh" ]]; then
  mkdir -p "$NVM_DIR"
  curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
fi
set +u
# shellcheck disable=SC1090
# shellcheck disable=SC1091
. "$NVM_DIR/nvm.sh"
INSTALLED_NODE_MAJOR=$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || printf 0)
if [[ "$INSTALLED_NODE_MAJOR" != "$NODE_MAJOR" ]]; then nvm install "$NODE_MAJOR"; fi
nvm alias default "$NODE_MAJOR" >/dev/null
nvm use "$NODE_MAJOR" >/dev/null
set -u

export PNPM_HOME
export PATH="$PNPM_HOME:$PATH"
if [[ "$(pnpm --version 2>/dev/null || true)" != "$EXPECTED_PNPM" ]]; then
  npm install --global "pnpm@$EXPECTED_PNPM"
fi

if ! command_exists pyenv; then git clone https://github.com/pyenv/pyenv.git "$PYENV_ROOT"; fi
export PATH="$PYENV_ROOT/bin:$PYENV_ROOT/shims:$PATH"
eval "$(pyenv init - bash)"
PYENV_VERSIONS=$(pyenv versions --bare)
PYTHON_PATCH=$(grep -E '^3\.13\.[0-9]+$' <<< "$PYENV_VERSIONS" | tail -n 1 || true)
if [[ -z "$PYTHON_PATCH" ]]; then
  PYTHON_PATCH=$(pyenv install --list | awk '$1 ~ /^3\.13\.[0-9]+$/ { version=$1 } END { print version }')
  [[ -n "$PYTHON_PATCH" ]] || { printf 'Não encontrei uma versão estável de Python %s.x no pyenv.\n' "$PYTHON_MINOR" >&2; exit 1; }
  pyenv install "$PYTHON_PATCH"
fi
pyenv global "$PYTHON_PATCH"

if ! command_exists uv; then curl -LsSf https://astral.sh/uv/install.sh | sh; fi
export PATH="$HOME/.local/bin:$PATH"
uv python install "$PYTHON_PATCH"
uv tool install --upgrade scons
uv tool install --upgrade pre-commit

if ! command_exists rustup; then
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable --no-modify-path
fi
export PATH="$HOME/.cargo/bin:$PATH"
rustup toolchain install stable --profile minimal
rustup default stable
rustup component add rustfmt clippy rust-analyzer --toolchain stable

if [[ "$MODE" == toolchain ]]; then exit 0; fi
export VECTORA_DOTFILES_TOOLCHAIN_DONE=1
"$ROOT_DIR/dotfiles.sh" --check
cd "$ROOT_DIR"
scons install
