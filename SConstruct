"""SCons entry point for the Rust engine workspace."""

import glob
import io
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import urllib.request
import zipfile


ROOT = Dir(".").abspath
NATS_VERSION = "2.10.22"
DEV_SERVICE_IMAGES = (
    "docker.io/library/postgres:17-alpine",
    "docker.io/qdrant/qdrant:v1.15.5",
    "docker.io/library/redis:7.4-alpine",
)


def _project_python_args() -> list[str]:
    """Select the pyenv-installed interpreter through uv when available."""
    configured = os.environ.get("UV_PYTHON")
    if configured:
        return ["--python", configured]
    if not sys.platform.startswith("linux"):
        return []
    pyenv_root = os.environ.get("PYENV_ROOT", os.path.expanduser("~/.pyenv"))
    candidates = glob.glob(
        os.path.join(pyenv_root, "versions", "3.13.*", "bin", "python")
    )
    if not candidates:
        return []
    latest = max(
        candidates,
        key=lambda path: tuple(
            int(part)
            for part in os.path.basename(
                os.path.dirname(os.path.dirname(path))
            ).split(".")
        ),
    )
    return ["--python", latest]


def _run(*command: str, cwd: str = ROOT) -> None:
    """Run a tool from the repository root and fail with its exit status."""
    executable = shutil.which(command[0])
    if executable is None:
        raise SystemExit(
            f"Required tool '{command[0]}' was not found. "
            "Install the prerequisites described in CONTRIBUTING.md."
        )
    result = subprocess.run(
        [executable, *command[1:]],
        cwd=cwd,
        stdin=subprocess.DEVNULL,
        check=False,
    )
    if result.returncode:
        raise SystemExit(result.returncode)


def _pnpm(*arguments: str, cwd: str = ROOT) -> None:
    """Run pnpm through the portable shim used by pre-commit."""
    _run(
        sys.executable,
        os.path.join(ROOT, "utils", "run_pnpm.py"),
        *arguments,
        cwd=cwd,
    )


def _install(target, source, env):
    if shutil.which("rustup") is None:
        raise SystemExit(
            "Install Rust with rustup first; rust-toolchain.toml selects "
            "stable, rustfmt, and clippy."
        )
    if not os.environ.get("PROTOC") and shutil.which("protoc") is None:
        guidance = (
            "Run ./dotfiles.sh on Linux, then retry scons install."
            if sys.platform.startswith("linux")
            else "Install the Protocol Buffers compiler and retry scons install."
        )
        raise SystemExit(
            "Protocol Buffers compiler 'protoc' is required by the LanceDB "
            f"dependency. {guidance}"
        )
    _run("rustup", "component", "add", "rustfmt", "clippy")
    _run("cargo", "fetch", "--locked")
    _run("uv", "sync", "--locked", "--all-groups", *_project_python_args())
    if shutil.which("podman") is None:
        raise SystemExit(
            "Podman is required to install the development service images. "
            "Install Podman, then rerun scons install."
        )
    _pnpm(
        "ci",
        cwd=os.path.join(ROOT, "bindings", "typescript"),
    )
    _install_nats_sidecar()
    for image in DEV_SERVICE_IMAGES:
        _run("podman", "pull", image)
    go_manifest = os.path.join(ROOT, "bindings", "go", "go.mod")
    if os.path.isfile(go_manifest):
        if shutil.which("go") is None:
            raise SystemExit(
                "Go bindings exist, but Go is not installed. Install Go and retry."
            )
        _run("go", "mod", "download", cwd=os.path.dirname(go_manifest))


def _install_nats_sidecar() -> None:
    """Install the pinned NATS server binary used by local engine development."""
    system = platform.system().lower()
    platform_name = {"linux": "linux", "darwin": "darwin", "windows": "windows"}.get(
        system
    )
    if platform_name is None:
        raise SystemExit(f"NATS sidecar does not support this OS: {platform.system()}")
    machine = platform.machine().lower()
    architecture = {
        "x86_64": "amd64",
        "amd64": "amd64",
        "arm64": "arm64",
        "aarch64": "arm64",
    }.get(machine)
    if architecture is None:
        raise SystemExit(f"NATS sidecar does not support this architecture: {machine}")
    stem = f"nats-server-v{NATS_VERSION}-{platform_name}-{architecture}"
    archive_extension = "zip" if platform_name == "windows" else "tar.gz"
    archive_url = (
        "https://github.com/nats-io/nats-server/releases/download/"
        f"v{NATS_VERSION}/{stem}.{archive_extension}"
    )
    binary_name = "nats-server.exe" if platform_name == "windows" else "nats-server"
    member_name = f"{stem}/{binary_name}"
    destination_dir = os.path.join(ROOT, "resources")
    destination = os.path.join(destination_dir, binary_name)
    version_marker = os.path.join(destination_dir, ".nats-server-version")
    if os.path.isfile(destination) and os.path.isfile(version_marker):
        with open(version_marker, encoding="utf-8") as marker:
            if marker.read().strip() == NATS_VERSION:
                print(f">> NATS sidecar já instalado: {destination}")
                return

    print(f">> baixando NATS sidecar: {archive_url}")
    with urllib.request.urlopen(archive_url, timeout=60) as response:
        archive = response.read()
    if archive_extension == "zip":
        with zipfile.ZipFile(io.BytesIO(archive)) as bundle:
            payload = bundle.read(member_name)
    else:
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as bundle:
            member = bundle.extractfile(member_name)
            if member is None:
                raise RuntimeError(f"NATS archive is missing {member_name}")
            payload = member.read()

    os.makedirs(destination_dir, exist_ok=True)
    temporary = f"{destination}.download"
    try:
        with open(temporary, "wb") as output:
            output.write(payload)
        os.replace(temporary, destination)
    finally:
        if os.path.exists(temporary):
            os.remove(temporary)
    if platform_name != "windows":
        os.chmod(destination, 0o755)
    with open(version_marker, "w", encoding="utf-8") as marker:
        marker.write(f"{NATS_VERSION}\n")
    print(f">> NATS sidecar instalado: {destination}")


def _build(target, source, env):
    """Compile Rust and each language binding, reusing persistent tool caches."""
    os.environ.setdefault("CARGO_TARGET_DIR", os.path.join(ROOT, "target"))
    os.environ.setdefault("CARGO_INCREMENTAL", "1")
    _run("cargo", "build", "--workspace", "--all-targets", "--all-features", "--locked")
    _run(
        "uv",
        "run",
        "--project",
        ROOT,
        "maturin",
        "build",
        "--locked",
        "--out",
        os.path.join(ROOT, "target", "wheels"),
        cwd=os.path.join(ROOT, "bindings", "python"),
    )
    _pnpm("build", cwd=os.path.join(ROOT, "bindings", "typescript"))

    go_manifest = os.path.join(ROOT, "bindings", "go", "go.mod")
    if os.path.isfile(go_manifest):
        if shutil.which("go") is None:
            raise SystemExit(
                "Go bindings exist, but Go is not installed. Install Go and retry."
            )
        _run("go", "build", "-mod=readonly", "./...", cwd=os.path.dirname(go_manifest))


def _lint(target, source, env):
    _run("cargo", "fmt", "--all", "--", "--check")
    _run(
        "cargo",
        "clippy",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--locked",
        "--",
        "-D",
        "warnings",
    )
    _run(
        "cargo",
        "doc",
        "--workspace",
        "--all-features",
        "--no-deps",
        "--locked",
    )
    _run("uv", "run", "ruff", "check", "utils", "bindings/python")
    _run("uv", "run", "ruff", "format", "--check", "utils", "bindings/python")
    _run("uv", "run", "ty", "check", "utils", "bindings/python")
    _run("uv", "run", "bandit", "-s", "B101", "-r", "utils", "bindings/python")
    _pnpm(
        "exec",
        "oxlint",
        "--deny-warnings",
        "src",
        "test",
        cwd=os.path.join(ROOT, "bindings", "typescript"),
    )
    _pnpm("typecheck", cwd=os.path.join(ROOT, "bindings", "typescript"))


def _tests(target, source, env):
    _run("cargo", "test", "--workspace", "--all-features", "--locked")
    _run("uv", "run", "pytest")
    _pnpm("build", cwd=os.path.join(ROOT, "bindings", "typescript"))
    _pnpm("test", cwd=os.path.join(ROOT, "bindings", "typescript"))


def _update(target, source, env):
    _run("cargo", "update")
    _run("uv", "lock", "--upgrade")


def _clean(target, source, env):
    _run("cargo", "clean")
    log_dir = os.path.join(ROOT, ".scons-logs")
    if os.path.isdir(log_dir):
        shutil.rmtree(log_dir)
    for path in (
        os.path.join(ROOT, "bindings", "typescript", "dist"),
        os.path.join(ROOT, "bindings", "typescript", ".cache"),
    ):
        if os.path.isdir(path):
            shutil.rmtree(path)
    native_dir = os.path.join(ROOT, "bindings", "typescript", "native")
    for name in ("index.js", "index.d.ts"):
        generated_file = os.path.join(native_dir, name)
        if os.path.isfile(generated_file):
            os.remove(generated_file)
    for name in os.listdir(native_dir):
        if name.endswith(".node"):
            os.remove(os.path.join(native_dir, name))


def _help(target, source, env):
    print(
        """
Vectora Engine — alvos SCons (rodar da raiz do repositório)

Setup
  scons install    instala dependências e sidecars de desenvolvimento
  scons build      compila Rust e bindings Python/TypeScript (e Go quando existir)

Qualidade
  scons lint       Rust (rustfmt + Clippy) e Python (Ruff + ty + Bandit)
  scons tests      Cargo, pytest e testes do pacote Node nativo
  scons ci         install + build + lint + tests

Manutenção
  scons update     atualiza Cargo e uv.lock; revise os lockfiles
  scons clean      remove target/ e logs do SCons
""".strip()
    )


def _ci(target, source, env):
    _install(target, source, env)
    _build(target, source, env)
    _lint(target, source, env)
    _tests(target, source, env)


env = Environment(ENV=os.environ)
env.Command("_help", [], _help)
env.Command("_install", [], _install)
env.Command("_build", [], _build)
env.Command("_lint", [], _lint)
env.Command("_tests", [], _tests)
env.Command("_ci", [], _ci)
env.Command("_update", [], _update)
env.Command("_clean", [], _clean)

Default("_help")
for alias, target in {
    "install": "_install",
    "build": "_build",
    "lint": "_lint",
    "tests": "_tests",
    "ci": "_ci",
    "update": "_update",
    "clean": "_clean",
}.items():
    Alias(alias, target)
