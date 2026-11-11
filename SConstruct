"""SCons entry point for the Rust engine workspace."""

import os
import platform
import shutil
import subprocess
import sys


ROOT = Dir(".").abspath


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


def _activate_user_tool_paths() -> None:
    """Expose tools installed by the Linux bootstrap to this SCons process."""
    home = os.path.expanduser("~")
    paths = [
        os.path.join(home, ".local", "bin"),
        os.path.join(home, ".cargo", "bin"),
        os.path.join(
            os.environ.get("PYENV_ROOT", os.path.join(home, ".pyenv")), "bin"
        ),
        os.path.join(
            os.environ.get("PYENV_ROOT", os.path.join(home, ".pyenv")), "shims"
        ),
        os.environ.get(
            "PNPM_HOME", os.path.join(home, ".local", "share", "pnpm")
        ),
    ]
    node_root = os.path.join(
        os.environ.get("NVM_DIR", os.path.join(home, ".nvm")), "versions", "node"
    )
    if os.path.isdir(node_root):
        paths.extend(
            os.path.join(node_root, entry, "bin")
            for entry in os.listdir(node_root)
            if entry.startswith("v24")
        )
    current = os.environ.get("PATH", "")
    os.environ["PATH"] = os.pathsep.join(
        [path for path in paths if os.path.isdir(path)] + [current]
    )


def _install(target, source, env):
    if platform.system() == "Linux" and not os.environ.get("VECTORA_DOTFILES_TOOLCHAIN_DONE"):
        _run("bash", os.path.join(ROOT, "dotfiles.sh"), "--toolchain-only")
        _activate_user_tool_paths()
    if shutil.which("rustup") is None:
        raise SystemExit(
            "Install Rust with rustup first; rust-toolchain.toml selects "
            "stable, rustfmt, and clippy."
        )
    if not os.environ.get("PROTOC") and shutil.which("protoc") is None:
        guidance = (
            "Run ./dotfiles.sh --toolchain-only, then retry scons install."
            if sys.platform.startswith("linux")
            else "Install the Protocol Buffers compiler and retry scons install."
        )
        raise SystemExit(
            "Protocol Buffers compiler 'protoc' is required by the LanceDB "
            f"dependency. {guidance}"
        )
    _run("rustup", "component", "add", "rustfmt", "clippy")
    _run("cargo", "fetch", "--locked")
    _run("uv", "sync", "--locked", "--all-groups")
    _pnpm(
        "ci",
        cwd=os.path.join(ROOT, "bindings", "typescript"),
    )


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


def _help(target, source, env):
    print(
        """
Vectora Engine — alvos SCons (rodar da raiz do repositório)

Setup
  scons install    prepara Rust, tooling Python e bindings nativas TypeScript/Python

Qualidade
  scons lint       Rust (rustfmt + Clippy) e Python (Ruff + ty + Bandit)
  scons tests      Cargo, pytest e testes do pacote Node nativo
  scons ci         install + lint + tests

Manutenção
  scons update     atualiza Cargo e uv.lock; revise os lockfiles
  scons clean      remove target/ e logs do SCons
""".strip()
    )


def _ci(target, source, env):
    _install(target, source, env)
    _lint(target, source, env)
    _tests(target, source, env)


env = Environment(ENV=os.environ)
env.Command("_help", [], _help)
env.Command("_install", [], _install)
env.Command("_lint", [], _lint)
env.Command("_tests", [], _tests)
env.Command("_ci", [], _ci)
env.Command("_update", [], _update)
env.Command("_clean", [], _clean)

Default("_help")
for alias, target in {
    "install": "_install",
    "lint": "_lint",
    "tests": "_tests",
    "ci": "_ci",
    "update": "_update",
    "clean": "_clean",
}.items():
    Alias(alias, target)
