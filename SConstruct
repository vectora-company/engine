"""SCons entry point for the Rust engine workspace."""

import os
import shutil
import subprocess


ROOT = Dir(".").abspath


def _run(*command: str) -> None:
    """Run a tool from the repository root and fail with its exit status."""
    executable = shutil.which(command[0])
    if executable is None:
        raise SystemExit(
            f"Required tool '{command[0]}' was not found. "
            "Install the prerequisites described in CONTRIBUTING.md."
        )
    result = subprocess.run(
        [executable, *command[1:]],
        cwd=ROOT,
        stdin=subprocess.DEVNULL,
        check=False,
    )
    if result.returncode:
        raise SystemExit(result.returncode)


def _install(target, source, env):
    if shutil.which("rustup") is None:
        raise SystemExit(
            "Install Rust with rustup first; rust-toolchain.toml selects "
            "stable, rustfmt, and clippy."
        )
    _run("rustup", "component", "add", "rustfmt", "clippy")
    _run("cargo", "fetch", "--locked")
    _run("uv", "sync", "--locked", "--all-groups")


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


def _tests(target, source, env):
    _run("cargo", "test", "--workspace", "--all-features", "--locked")
    _run("uv", "run", "pytest")


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
  scons install    prepara rustfmt/clippy, dependências Cargo e tooling Python

Qualidade
  scons lint       Rust (rustfmt + Clippy) e Python (Ruff + ty + Bandit)
  scons tests      Cargo e pytest (tooling e bindings Python)
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
