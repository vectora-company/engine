"""Tests for raising Python requirement floors from the resolved uv lock."""

from pathlib import Path

import pytest
from update_dependency_constraints import update_constraints


def test_updates_dependency_groups_and_pep517_while_preserving_constraints(
    tmp_path: Path,
):
    project = tmp_path / "pyproject.toml"
    project.write_text(
        "[dependency-groups]\n"
        "dev = [\n"
        '  "pytest>=8.0,<10",\n'
        "  \"httpx[http2]>=0.26; python_version >= '3.13'\",\n"
        "]\n",
        encoding="utf-8",
    )
    build_project = tmp_path / "binding-pyproject.toml"
    build_project.write_text(
        '[build-system]\nrequires = [\n  "maturin>=1.0,<2",\n]\n',
        encoding="utf-8",
    )
    lock = tmp_path / "uv.lock"
    lock.write_text(
        "version = 1\n"
        '[[package]]\nname = "pytest"\nversion = "9.2.0"\n'
        '[[package]]\nname = "httpx"\nversion = "0.28.1"\n'
        '[[package]]\nname = "maturin"\nversion = "1.16.0"\n',
        encoding="utf-8",
    )

    assert update_constraints(project, lock, build_project)
    assert '"pytest>=9.2.0,<10"' in project.read_text(encoding="utf-8")
    assert "\"httpx[http2]>=0.28.1; python_version >= '3.13'\"" in project.read_text(
        encoding="utf-8"
    )
    assert '"maturin>=1.16.0,<2"' in build_project.read_text(encoding="utf-8")
    assert not update_constraints(project, lock, build_project)


def test_adds_a_lower_bound_to_an_unbounded_requirement(tmp_path: Path):
    project = tmp_path / "pyproject.toml"
    project.write_text(
        '[dependency-groups]\ndev = [\n  "pytest",\n]\n', encoding="utf-8"
    )
    lock = tmp_path / "uv.lock"
    lock.write_text(
        'version = 1\n[[package]]\nname = "pytest"\nversion = "9.2.0"\n',
        encoding="utf-8",
    )

    assert update_constraints(project, lock, tmp_path / "missing.toml")
    assert '"pytest >=9.2.0"' in project.read_text(encoding="utf-8")


def test_fails_if_a_direct_requirement_is_missing_from_lock(tmp_path: Path):
    project = tmp_path / "pyproject.toml"
    project.write_text(
        '[dependency-groups]\ndev = [\n  "pytest>=8",\n]\n', encoding="utf-8"
    )
    lock = tmp_path / "uv.lock"
    lock.write_text("version = 1\npackage = []\n", encoding="utf-8")

    with pytest.raises(ValueError, match=r"Dependência ausente em uv\.lock"):
        update_constraints(project, lock, tmp_path / "missing.toml")
