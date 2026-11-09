"""Run the pinned pnpm shim on POSIX and Windows development environments."""

from __future__ import annotations

import errno
import os
import shutil
import subprocess  # nosec B404
import sys


def main(argv: list[str]) -> int:
    """Run pnpm, retrying a shebang-less pnpm 11 shim through its host shell."""
    executable = shutil.which("pnpm")
    if executable is None:
        print("pnpm was not found; install the version pinned by packageManager.")
        return 127

    command = [executable, *argv]
    try:
        result = subprocess.run(  # noqa: S603  # nosec B603
            command,
            check=False,
            shell=os.name == "nt",  # nosec B602 - arguments are fixed repository tooling commands
        )
    except OSError as error:
        if error.errno != errno.ENOEXEC or os.name == "nt":
            raise
        shell = shutil.which("sh") or "/bin/sh"
        result = subprocess.run(  # noqa: S603  # nosec B603
            [shell, executable, *argv],
            check=False,
        )
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
