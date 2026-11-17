#!/usr/bin/env bash
# Instala o modelo Laya System 1 por laya-serve ou llama.cpp.
# O app de notificações em laya.aay.sh é um produto distinto.
set -Eeuo pipefail

OS=$(uname -s)
case "$OS" in
  Linux) ;;
  MINGW*|MSYS*|CYGWIN*|Windows_NT)
    printf 'Este instalador só prossegue em Linux; Windows detectado.\n' >&2
    exit 1
    ;;
  Darwin)
    printf 'Este instalador só prossegue em Linux; macOS detectado.\n' >&2
    exit 1
    ;;
  *) printf 'Sistema operacional não suportado: %s.\n' "$OS" >&2; exit 1 ;;
esac

if [[ ! -r /etc/os-release ]]; then
  printf 'Não foi possível identificar a distribuição Linux.\n' >&2
  exit 1
fi
# shellcheck disable=SC1091
. /etc/os-release
case "${ID:-}:${ID_LIKE:-}" in
  pop:*|ubuntu:*|debian:*|*:debian*) DISTRO_FAMILY=debian ;;
  fedora:*|*:fedora*) DISTRO_FAMILY=fedora ;;
  arch:*|*:arch*) DISTRO_FAMILY=arch ;;
  *) printf 'Distribuição não mapeada: %s. Nenhum pacote de sistema será alterado.\n' "${PRETTY_NAME:-unknown}" >&2; exit 1 ;;
esac
printf 'Distribuição detectada: %s (%s).\n' "${PRETTY_NAME:-Linux}" "$DISTRO_FAMILY"
(( EUID != 0 )) || { printf 'Execute como usuário normal; não é necessário root/sudo.\n' >&2; exit 1; }

MODE=install
ENGINE=
while (($#)); do
  case "$1" in
    --mode) (($# >= 2)) || { printf 'Falta o valor de --mode.\n' >&2; exit 2; }; ENGINE=$2; shift 2 ;;
    --check) MODE=check; shift ;;
    -h|--help) MODE=help; shift ;;
    *) printf 'Argumento desconhecido: %s\n' "$1" >&2; exit 2 ;;
  esac
done
case "$ENGINE" in
  llamacpp) ENGINE=llama.cpp ;;
  ""|laya-server|llama.cpp) ;;
  *) printf 'Modo inválido: %s. Use laya-server ou llama.cpp.\n' "$ENGINE" >&2; exit 2 ;;
esac
if [[ "$MODE" == help ]]; then
  cat <<'EOF'
Uso: ./laya-install.sh [--mode laya-server|llama.cpp] [--check]

Sem --mode, apresenta um menu interativo:
  laya-server  Laya oficial com Transformers/PyTorch CPU e API protegida local
  llama.cpp    usa o binário oficial llama e carrega ggml-org/Laya-GGUF:Q8_0 do Hugging Face

--check verifica o modo escolhido sem alterar arquivos.
EOF
  exit 0
fi
if [[ -z "$ENGINE" ]]; then
  [[ -t 0 ]] || { printf 'Em execução não interativa, informe --mode laya-server ou --mode llama.cpp.\n' >&2; exit 2; }
  printf 'Escolha como executar o Laya:\n  1) Servidor oficial Laya (Transformers/PyTorch CPU)\n  2) llama.cpp otimizado com ggml-org/Laya-GGUF:Q8_0 (GPU quando disponível)\n'
  read -r -p 'Opção [1/2]: ' choice
  case "$choice" in 1) ENGINE=laya-server ;; 2) ENGINE=llama.cpp ;; *) printf 'Opção inválida.\n' >&2; exit 2 ;; esac
fi

for tool in curl python3 systemctl; do
  command -v "$tool" >/dev/null 2>&1 || { printf 'Dependência ausente: %s.\n' "$tool" >&2; exit 1; }
done
systemctl --user show-environment >/dev/null 2>&1 || { printf 'systemd de usuário não está disponível nesta sessão.\n' >&2; exit 1; }

VECTOR_HOME=${VECTORA_HOME:-"$HOME/.vectora-dev"}
DATA_HOME=${XDG_DATA_HOME:-"$HOME/.local/share"}
CONFIG_HOME=${XDG_CONFIG_HOME:-"$HOME/.config"}
UNIT_DIR="$CONFIG_HOME/systemd/user"
LAYA_ROOT="$DATA_HOME/vectora/laya"
LAYA_PORT=18000
LLAMA_PORT=18080
LAYA_KEY=
LLAMA_ROOT="$VECTOR_HOME/tools/llama.cpp"
LLAMA_MODELS="$DATA_HOME/vectora/models/huggingface/ggml-org/Laya-GGUF"
MODEL_ID=multilingual
if [[ "$ENGINE" == llama.cpp ]]; then
  MODEL_ID=ggml-org/Laya-GGUF:Q8_0
fi

if [[ "$MODE" == check ]]; then
  SERVICE=vectora-laya.service
  if [[ "$ENGINE" == llama.cpp ]]; then SERVICE=vectora-laya-llama.service; fi
  systemctl --user is-active --quiet "$SERVICE" || { printf 'Serviço inativo: %s\n' "$SERVICE" >&2; exit 1; }
  PORT=$LAYA_PORT
  [[ "$ENGINE" != llama.cpp ]] || PORT=$LLAMA_PORT
  HEALTH_ARGS=()
  if [[ "$ENGINE" == laya-server ]]; then
    KEY=$(sed -n 's/^LAYA_API_KEY=//p' "$CONFIG_HOME/vectora/laya.env")
    HEALTH_ARGS=(-H "Authorization: Bearer $KEY")
  else
    KEY=$(sed -n 's/^LLAMA_ARG_API_KEY=//p' "$CONFIG_HOME/vectora/laya-llama.env")
    HEALTH_ARGS=(-H "Authorization: Bearer $KEY")
  fi
  curl --silent --fail --max-time 5 "${HEALTH_ARGS[@]}" "http://127.0.0.1:$PORT/health" >/dev/null || { printf 'Health-check falhou em 127.0.0.1:%s.\n' "$PORT" >&2; exit 1; }
  grep -q "SYSTEMONE_LAYA_MODEL=$MODEL_ID" "$VECTOR_HOME/.env" || { printf 'Modelo não configurado no Vectora.\n' >&2; exit 1; }
  if [[ "$ENGINE" == llama.cpp ]]; then
    LLAMA_EXE="$LLAMA_ROOT/active/llama"
    [[ -x "$LLAMA_EXE" ]] || { printf 'Runtime llama não encontrado: %s\n' "$LLAMA_EXE" >&2; exit 1; }
  fi
  printf '[ok] modo %s ativo e configurado no Vectora.\n' "$ENGINE"
  exit 0
fi

mkdir -p "$UNIT_DIR" "$VECTOR_HOME"
chmod 700 "$UNIT_DIR" "$VECTOR_HOME"
if [[ "$ENGINE" == laya-server ]]; then
  command -v uv >/dev/null 2>&1 || { printf 'Dependência ausente: uv.\n' >&2; exit 1; }
  LAYA_VENV="$LAYA_ROOT/.venv"
  mkdir -p "$LAYA_ROOT"
  if [[ ! -x "$LAYA_VENV/bin/python" ]]; then uv venv --python 3.12 "$LAYA_VENV"; fi
  uv pip install --python "$LAYA_VENV/bin/python" \
    --index-url https://download.pytorch.org/whl/cpu \
    --extra-index-url https://pypi.org/simple \
    --index-strategy first-index \
    'torch==2.15.0+cpu'
  uv pip install --python "$LAYA_VENV/bin/python" \
    --index-url https://pypi.org/simple \
    'laya[serve]==0.4.34'
  LAYA_ENV="$CONFIG_HOME/vectora/laya.env"
  mkdir -p "$(dirname "$LAYA_ENV")"
  if [[ ! -s "$LAYA_ENV" ]]; then
    umask 077
    LAYA_KEY=$(python3 -c 'import secrets; print(secrets.token_hex(32))')
    cat > "$LAYA_ENV" <<EOF
LAYA_HOST=127.0.0.1
LAYA_PORT=$LAYA_PORT
LAYA_DEVICE=cpu
LAYA_PRELOAD=0
LAYA_MODELS=english,multilingual,typed-decisions
LAYA_MAX_LOADED=3
LAYA_THREADS=4
LAYA_AUTO_TASK=1
LAYA_DEFAULT_MODEL=multilingual
LAYA_API_KEY=$LAYA_KEY
USE_TF=0
HF_HOME=$LAYA_ROOT/huggingface
EOF
  else
    LAYA_KEY=$(sed -n 's/^LAYA_API_KEY=//p' "$LAYA_ENV")
  fi
  if [[ -z "$LAYA_KEY" ]]; then
    LAYA_KEY=$(python3 -c 'import secrets; print(secrets.token_hex(32))')
    LAYA_ENV_TMP="$LAYA_ENV.tmp.$$"
    { grep -v '^LAYA_API_KEY=' "$LAYA_ENV" || true; printf 'LAYA_API_KEY=%s\n' "$LAYA_KEY"; } > "$LAYA_ENV_TMP"
    chmod 600 "$LAYA_ENV_TMP"
    mv -f "$LAYA_ENV_TMP" "$LAYA_ENV"
  fi
  chmod 600 "$LAYA_ENV"
  cat > "$UNIT_DIR/vectora-laya.service" <<EOF
[Unit]
Description=Servidor local Laya para Vectora
After=network.target

[Service]
Type=simple
EnvironmentFile=$LAYA_ENV
ExecStart=$LAYA_VENV/bin/laya-serve
Restart=on-failure
RestartSec=5
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=$LAYA_ROOT

[Install]
WantedBy=default.target
EOF
  SERVICE=vectora-laya.service
  BASE_URL="http://127.0.0.1:$LAYA_PORT"
else
  command -v curl >/dev/null 2>&1 || { printf 'Dependência ausente: curl.\n' >&2; exit 1; }
  mkdir -p "$LLAMA_ROOT/versions" "$LLAMA_MODELS/cache"
  ARCH=$(uname -m)
  case "$ARCH" in x86_64|amd64|aarch64|arm64) ;; *) printf 'Arquitetura sem runtime oficial: %s.\n' "$ARCH" >&2; exit 1 ;; esac
  TMP_HOME=$(mktemp -d "$LLAMA_ROOT/.install-home.XXXXXX")
  trap 'rm -rf -- "$TMP_HOME"' EXIT
  curl --fail --location --silent --show-error --retry 5 --retry-all-errors --connect-timeout 20 --max-time 90 \
    https://llama.app/install.sh -o "$TMP_HOME/install.sh"
  [[ -s "$TMP_HOME/install.sh" ]] || { printf 'O instalador oficial do llama.cpp veio vazio.\n' >&2; exit 1; }
  HOME="$TMP_HOME" SKIP_INSTALL=1 sh "$TMP_HOME/install.sh"
  LLAMA_STAGED="$TMP_HOME/.llama-app/llama"
  [[ -x "$LLAMA_STAGED" ]] || { printf 'O instalador oficial não produziu o binário llama.\n' >&2; exit 1; }
  LLAMA_VERSION_OUTPUT=$("$LLAMA_STAGED" version 2>&1)
  LLAMA_SHA=$(sha256sum "$LLAMA_STAGED" | cut -d ' ' -f1)
  RUNTIME_ID="$(printf '%s' "$LLAMA_VERSION_OUTPUT" | head -n1 | tr -cs '[:alnum:]._- ' '_' | cut -c1-48)-${LLAMA_SHA:0:12}"
  VERSION_DIR="$LLAMA_ROOT/versions/$RUNTIME_ID"
  mkdir -p "$VERSION_DIR"
  install -m 755 "$LLAMA_STAGED" "$VERSION_DIR/llama"
  LLAMA_EXE="$VERSION_DIR/llama"
  "$LLAMA_EXE" version >/dev/null 2>&1 || { printf 'O binário oficial llama não executou.\n' >&2; exit 1; }
  ln -sfn "versions/$RUNTIME_ID" "$LLAMA_ROOT/.active.tmp"
  mv -Tf "$LLAMA_ROOT/.active.tmp" "$LLAMA_ROOT/active"
  python3 - "$LLAMA_ROOT" "$RUNTIME_ID" "$LLAMA_VERSION_OUTPUT" "$LLAMA_SHA" <<'PY'
import json, pathlib, sys
from datetime import datetime, timezone
root = pathlib.Path(sys.argv[1])
rid, version, digest = sys.argv[2:]
manifest = root / "runtime-manifest.json"
try:
    runtimes = json.loads(manifest.read_text()).get("runtimes", [])
except (OSError, json.JSONDecodeError):
    runtimes = []
runtimes = [item for item in runtimes if not isinstance(item, dict) or item.get("id") != rid]
runtimes.append({"id": rid, "version": version.splitlines()[0], "sha256": digest,
                 "source": "https://llama.app/install.sh", "directory": str(root / "versions" / rid),
                 "installed_at": datetime.now(timezone.utc).isoformat()})
temporary = manifest.with_suffix(".json.tmp")
temporary.write_text(json.dumps({"runtimes": runtimes}, indent=2) + "\n")
temporary.replace(manifest)
(root / "active-runtime").write_text(rid + "\n")
PY
  LAYOUT="$LLAMA_MODELS/cache"
  LAYA_KEY=$(python3 -c 'import secrets; print(secrets.token_hex(32))')
  LAYA_ENV="$CONFIG_HOME/vectora/laya-llama.env"
  mkdir -p "$(dirname "$LAYA_ENV")"
  umask 077
  cat > "$LAYA_ENV" <<EOF
LLAMA_ARG_API_KEY=$LAYA_KEY
HF_HOME=$LAYOUT/huggingface
LLAMA_CACHE=$LAYOUT
EOF
  chmod 600 "$LAYA_ENV"
  cat > "$UNIT_DIR/vectora-laya-llama.service" <<EOF
[Unit]
Description=Laya GGUF local via llama.cpp para Vectora
After=network.target

[Service]
Type=simple
EnvironmentFile=$LAYA_ENV
ExecStart=$LLAMA_EXE serve -hf $MODEL_ID --alias $MODEL_ID --host 127.0.0.1 --port $LLAMA_PORT --threads 4 --ctx-size 8192 --ubatch-size 8192 --no-webui
Restart=on-failure
RestartSec=5
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=$LAYOUT $LLAMA_ROOT $LLAMA_MODELS

[Install]
WantedBy=default.target
EOF
  SERVICE=vectora-laya-llama.service
  BASE_URL="http://127.0.0.1:$LLAMA_PORT"
fi

systemctl --user daemon-reload
systemctl --user enable "$SERVICE"
systemctl --user restart "$SERVICE"

printf 'Aguardando health-check do Laya em %s…\n' "$BASE_URL"
ready=0
for _ in $(seq 1 180); do
  HEALTH_ARGS=()
  if [[ -n "$LAYA_KEY" ]]; then HEALTH_ARGS=(-H "Authorization: Bearer $LAYA_KEY"); fi
  if curl --silent --fail --max-time 3 "${HEALTH_ARGS[@]}" "$BASE_URL/health" >/dev/null; then ready=1; break; fi
  sleep 2
done
if (( ! ready )); then
  systemctl --user --no-pager --full status "$SERVICE" || true
  printf 'Laya não ficou saudável. Consulte: journalctl --user -u %s -f\n' "$SERVICE" >&2
  exit 1
fi

# Commit a configuração no consumidor apenas depois que o endpoint responde.
VECTORA_ENV="$VECTOR_HOME/.env"
python3 - "$VECTORA_ENV" "$BASE_URL" "$MODEL_ID" "$LAYA_KEY" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1]); base, model, key = sys.argv[2:]
updates = {
    "SYSTEMONE_PROVIDER": "laya-local",
    "SYSTEMONE_LAYA_BASE_URL": base,
    "SYSTEMONE_LAYA_MODEL": model,
    "SYSTEMONE_LAYA_API_KEY": key,
}
lines = path.read_text(encoding="utf-8").splitlines() if path.exists() else []
for name, value in updates.items():
    lines = [line for line in lines if not line.startswith(name + "=")]
    if value: lines.append(f"{name}={value}")
path.parent.mkdir(parents=True, exist_ok=True)
temporary = path.with_suffix(path.suffix + ".tmp")
temporary.write_text("\n".join(lines) + "\n", encoding="utf-8")
temporary.chmod(0o600)
temporary.replace(path)
PY

if [[ "$ENGINE" == llama.cpp ]]; then
  systemctl --user disable --now vectora-laya.service >/dev/null 2>&1 || true
else
  systemctl --user disable --now vectora-laya-llama.service >/dev/null 2>&1 || true
fi
printf '[ok] Laya ativo via %s em %s.\n' "$ENGINE" "$BASE_URL"
printf '[ok] Vectora configurado com o endpoint local e modelo %s. A ativação do SystemOne permanece sob seu controle.\n' "$MODEL_ID"
printf 'Reinicie o Vectora para carregar a configuração persistida.\n'
printf 'Logs: journalctl --user -u %s -f\n' "$SERVICE"
