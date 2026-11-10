"""SCons entry point for the Rust engine workspace."""

import os
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


def _install(target, source, env):
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
    _run(
        "uv",
        "run",
        "--project",
        ROOT,
        "maturin",
        "develop",
        cwd=os.path.join(ROOT, "bindings", "python"),
    )
    _pnpm(
        "install",
        "--frozen-lockfile",
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
