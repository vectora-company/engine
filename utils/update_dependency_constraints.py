"""Raise direct Python dependency lower bounds to versions in ``uv.lock``."""

from __future__ import annotations

import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEPENDENCY_LINE = re.compile(r"^(\s*)(['\"])(.*)\2(,?\s*)$")
ARRAY_START = re.compile(r"^\s*([A-Za-z0-9_-]+)\s*=\s*\[\s*(?:#.*)?$")
SECTION = re.compile(r"^\s*\[([^]]+)]\s*(?:#.*)?$")
PACKAGE_NAME = re.compile(r"\s*([A-Za-z0-9][A-Za-z0-9._-]*)(?:\[[^]]+])?")
LOWER_BOUND = re.compile(r">=\s*[^,;\s]+")


def _normalize(name: str) -> str:
    return re.sub(r"[-_.]+", "-", name).lower()


def _locked_versions(lock: Path) -> dict[str, str]:
    lock_data = tomllib.loads(lock.read_text(encoding="utf-8"))
    versions: dict[str, str] = {}
    for package in lock_data["package"]:
        name = _normalize(package["name"])
        version = package["version"]
        previous = versions.get(name)
        if previous is None or _version_key(version) > _version_key(previous):
            versions[name] = version
    return versions


def _version_key(version: str) -> tuple[int, ...]:
    return tuple(int(part) for part in re.findall(r"\d+", version))


def _raise_lower_bound(requirement: str, versions: dict[str, str]) -> str:
    base, separator, marker = requirement.partition(";")
    name = PACKAGE_NAME.match(base)
    if not name or " @ " in base:
        return requirement
    locked = versions.get(_normalize(name.group(1)))
    if locked is None:
        raise ValueError(f"Dependência ausente em uv.lock: {requirement}")

    lower = LOWER_BOUND.search(base)
    if lower:
        if lower.group().replace(" ", "") == f">={locked}":
            return requirement
        updated = LOWER_BOUND.sub(f">={locked}", base, count=1)
    else:
        updated = f"{base[: name.end()].rstrip()} >={locked}{base[name.end() :]}"
    return f"{updated}{separator}{marker}" if separator else updated


def _dependency_array(section: str, key: str) -> bool:
    return (
        (section == "project" and key == "dependencies")
        or section == "project.optional-dependencies"
        or (section == "dependency-groups" and key != "include-group")
        or (section == "build-system" and key == "requires")
    )


def _update_file(path: Path, versions: dict[str, str]) -> bool:
    lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    section = ""
    array_key: str | None = None
    changed = False

    for index, line in enumerate(lines):
        section_match = SECTION.match(line)
        if section_match:
            section = section_match.group(1)
            array_key = None
            continue

        if array_key is None:
            array_start = ARRAY_START.match(line)
            if array_start and _dependency_array(section, array_start.group(1)):
                array_key = array_start.group(1)
            else:
                continue

        if line.lstrip().startswith("]"):
            array_key = None
            continue
        match = DEPENDENCY_LINE.match(line.rstrip("\n"))
        if not match:
            continue
        updated = _raise_lower_bound(match.group(3), versions)
        if updated != match.group(3):
            lines[index] = (
                f"{match.group(1)}{match.group(2)}{updated}"
                f"{match.group(2)}{match.group(4)}\n"
            )
            changed = True

    if changed:
        path.write_text("".join(lines), encoding="utf-8")
    return changed


def update_constraints(
    project: Path | None = None,
    lock: Path | None = None,
    build_project: Path | None = None,
) -> bool:
    """Update direct dependency minima in the root and Python build manifests."""
    root_project = project or ROOT / "pyproject.toml"
    lock_path = lock or ROOT / "uv.lock"
    build_manifest = build_project or ROOT / "bindings/python/pyproject.toml"
    versions = _locked_versions(lock_path)
    changed = _update_file(root_project, versions)
    if build_manifest.is_file():
        changed = _update_file(build_manifest, versions) or changed
    return changed


if __name__ == "__main__":
    status = "atualizado" if update_constraints() else "já atualizado"
    print(f"Manifests Python {status}")
