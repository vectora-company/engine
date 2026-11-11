"""Testes de utils/checks.py, um bloco por subcomando."""

from __future__ import annotations

from pathlib import Path

import checks
import pytest
from checks import (
    RemovedPath,
    _find_references,
    _find_violations,
    _staged_files,
    _staged_removed_paths,
    no_plan_main,
)

# ── no-plan ──────────────────────────────────────────────────────────────────


pytestmark = pytest.mark.usefixtures("tmp_path")


def _write(tmp_path: Path, name: str, content: str) -> str:
    """Executa a operação write."""
    path = tmp_path / name
    path.write_text(content, encoding="utf-8")
    return str(path)


class TestPlanoDeteccao:
    def test_bloqueia_sprint_numerado(self, tmp_path: Path) -> None:
        """Verifica o comportamento de bloqueia sprint numerado."""
        path = _write(tmp_path, "a.py", "# Sprint 3: ajusta cache\nx = 1\n")
        assert _find_violations(path) == [(1, "# Sprint 3: ajusta cache")]

    def test_bloqueia_sprint_com_letra(self, tmp_path: Path) -> None:
        """Verifica o comportamento de bloqueia sprint com letra."""
        path = _write(tmp_path, "a.py", "# Sprint E: enxuga o build\nx = 1\n")
        assert len(_find_violations(path)) == 1

    def test_erro_borda_sprint_com_letra_minuscula_nao_bloqueia(
        self, tmp_path: Path
    ) -> None:
        """Verifica o comportamento de erro borda sprint com letra minuscula nao bloqueia."""
        path = _write(tmp_path, "a.py", "# sprint e atualiza o cliente\nx = 1\n")
        assert _find_violations(path) == []

    def test_bloqueia_bloco_com_letra_maiuscula(self, tmp_path: Path) -> None:
        """Verifica o comportamento de bloqueia bloco com letra maiuscula."""
        path = _write(tmp_path, "a.ts", "// Bloco T10.4 concluÃ­do\nconst x = 1;\n")
        assert len(_find_violations(path)) == 1

    def test_bloqueia_fase_com_sufixo(self, tmp_path: Path) -> None:
        """Verifica o comportamento de bloqueia fase com sufixo."""
        path = _write(tmp_path, "a.py", "# fase-2a do rollout\nx = 1\n")
        assert len(_find_violations(path)) == 1

    def test_erro_borda_bloco_minusculo_nao_bloqueia(self, tmp_path: Path) -> None:
        """Verifica o comportamento de erro borda bloco minusculo nao bloqueia."""
        path = _write(tmp_path, "a.py", "# separa em um bloco de cÃ³digo sÃ³\nx = 1\n")
        assert _find_violations(path) == []


class TestFerramentaIaDeteccao:
    def test_bloqueia_coderabbit_case_insensitive(self, tmp_path: Path) -> None:
        """Verifica o comportamento de bloqueia coderabbit case insensitive."""
        path = _write(tmp_path, "a.tsx", "// Addressed by CodeRabbit\nconst x = 1;\n")
        assert len(_find_violations(path)) == 1

    def test_bloqueia_claude_code(self, tmp_path: Path) -> None:
        """Verifica o comportamento de bloqueia claude code."""
        path = _write(tmp_path, "a.py", "# corrigido via Claude Code\nx = 1\n")
        assert len(_find_violations(path)) == 1

    def test_bloqueia_copilot(self, tmp_path: Path) -> None:
        """Verifica o comportamento de bloqueia copilot."""
        path = _write(tmp_path, "a.js", "// sugestÃ£o do copilot\nconst x = 1;\n")
        assert len(_find_violations(path)) == 1

    def test_erro_borda_comentario_dentro_de_interpolacao_de_template_bloqueia(
        self, tmp_path: Path
    ) -> None:
        """Verifica o comportamento de erro borda comentario dentro de interpolacao de template bloqueia."""
        path = _write(tmp_path, "a.ts", "const value = `${/* Claude */ 1}`;\n")
        assert len(_find_violations(path)) == 1

    def test_erro_borda_nome_de_variavel_claude_nao_bloqueia_sozinho(
        self, tmp_path: Path
    ) -> None:
        """Verifica o comportamento de erro borda nome de variavel claude nao bloqueia sozinho."""
        path = _write(
            tmp_path, "a.py", "# nenhuma referÃªncia de ferramenta aqui\nx = 1\n"
        )
        assert _find_violations(path) == []

    def test_erro_borda_identificador_claude_em_codigo_real_nao_bloqueia(
        self, tmp_path: Path
    ) -> None:
        """Verifica o comportamento de erro borda identificador claude em codigo real nao bloqueia."""
        path = _write(tmp_path, "a.py", "claude = object()\n")
        assert _find_violations(path) == []

    def test_erro_borda_string_com_id_de_modelo_claude_nao_bloqueia(
        self, tmp_path: Path
    ) -> None:
        """Verifica o comportamento de erro borda string com id de modelo claude nao bloqueia."""
        path = _write(tmp_path, "a.py", 'model = "claude-3-5-sonnet-latest"\n')
        assert _find_violations(path) == []

    def test_erro_borda_referencia_a_claude_md_nao_bloqueia(
        self, tmp_path: Path
    ) -> None:
        """Verifica o comportamento de erro borda referencia a claude md nao bloqueia."""
        path = _write(tmp_path, "a.py", "# regra vem de CLAUDE.md Â§1\nx = 1\n")
        assert _find_violations(path) == []


class TestTrackerDeteccao:
    def test_nao_bloqueia_cor_hexadecimal_css(self, tmp_path: Path) -> None:
        """Verifica o comportamento de nao bloqueia cor hexadecimal css."""
        path = _write(tmp_path, "a.py", "# background-color: #000000\nx = 1\n")
        assert _find_violations(path) == []

    def test_nao_bloqueia_cor_hexadecimal_css_com_alpha(self, tmp_path: Path) -> None:
        """Verifica o comportamento de nao bloqueia cor hexadecimal css com alpha."""
        path = _write(tmp_path, "a.py", "# background-color: #00000080\nx = 1\n")
        assert _find_violations(path) == []

    def test_nao_bloqueia_cores_css_curta(self, tmp_path: Path) -> None:
        """Não confunde #000 e #0000 com referências de issue."""
        path = _write(tmp_path, "a.py", "# color: #000; color: #0000\n")
        assert _find_violations(path) == []

    def test_bloqueia_issue_com_numero(self, tmp_path: Path) -> None:
        """Verifica o comportamento de bloqueia issue com numero."""
        path = _write(tmp_path, "a.py", "# fix para issue #318\nx = 1\n")
        assert len(_find_violations(path)) == 1

    def test_bloqueia_numero_de_issue_direto(self, tmp_path: Path) -> None:
        """Verifica o comportamento de bloqueia numero de issue direto."""
        path = _write(tmp_path, "a.ts", "// resolvido em #384\nconst x = 1;\n")
        assert len(_find_violations(path)) == 1
        path2 = _write(tmp_path, "b.py", "# ver #1 para contexto\nx = 1\n")
        assert len(_find_violations(path2)) == 1

    def test_bloqueia_pr_com_numero(self, tmp_path: Path) -> None:
        """Verifica o comportamento de bloqueia pr com numero."""
        path = _write(tmp_path, "a.tsx", "// PR 383 adicionou isso\nconst x = 1;\n")
        assert len(_find_violations(path)) == 1

    def test_bloqueia_pull_request(self, tmp_path: Path) -> None:
        """Verifica o comportamento de bloqueia pull request."""
        path = _write(tmp_path, "a.py", "# ver pull request #12\nx = 1\n")
        assert len(_find_violations(path)) == 1

    def test_bloqueia_pull_request_sem_numero(self, tmp_path: Path) -> None:
        """Verifica o comportamento de bloqueia pull request sem numero."""
        path = _write(tmp_path, "a.py", "# acompanhamento no pull request\nx = 1\n")
        assert len(_find_violations(path)) == 1

    def test_bloqueia_urls_github_de_issue_e_pull(self, tmp_path: Path) -> None:
        """Verifica o comportamento de bloqueia urls github de issue e pull."""
        issue = _write(
            tmp_path,
            "issue.py",
            "# https://github.com/vectora-ltda/vectora/issues/367\nx = 1\n",
        )
        pull = _write(
            tmp_path,
            "pull.py",
            "# https://github.com/vectora-ltda/vectora/pull/385\nx = 1\n",
        )
        assert len(_find_violations(issue)) == 1
        assert len(_find_violations(pull)) == 1

    def test_aceita_gha_e_github_actions_mas_bloqueia_issue(
        self, tmp_path: Path
    ) -> None:
        """GHA e GitHub Actions descrevem o ambiente de execução e são
        permitidos; a referência a uma issue na mesma linha continua barrada."""
        path1 = _write(tmp_path, "b.py", "# rodado no GHA\nx = 1\n")
        assert _find_violations(path1) == []
        path2 = _write(tmp_path, "c.py", "# fluxo do github actions\nx = 1\n")
        assert _find_violations(path2) == []
        path3 = _write(tmp_path, "d.py", "# .env.gha.example lista os secrets\nx = 1\n")
        assert _find_violations(path3) == []
        path4 = _write(tmp_path, "e.py", "# rodado no GHA, ver issue #12\nx = 1\n")
        assert len(_find_violations(path4)) == 1


class TestMain:
    def test_erro_borda_arquivo_limpo_retorna_zero(self, tmp_path: Path) -> None:
        """Verifica o comportamento de erro borda arquivo limpo retorna zero."""
        path = _write(tmp_path, "a.py", "def soma(a, b):\n    return a + b\n")
        assert no_plan_main([path]) == 0

    def test_erro_borda_arquivo_com_violacao_retorna_um(self, tmp_path: Path) -> None:
        """Verifica o comportamento de erro borda arquivo com violacao retorna um."""
        path = _write(tmp_path, "a.py", "# Sprint 1\nx = 1\n")
        assert no_plan_main([path]) == 1

    def test_erro_borda_arquivo_inexistente_nao_lanca_excecao(self) -> None:
        """Verifica o comportamento de erro borda arquivo inexistente nao lanca excecao."""
        assert no_plan_main(["C:/caminho/que/nao/existe.py"]) == 0

    def test_erro_borda_multiplos_arquivos_um_so_com_violacao(
        self, tmp_path: Path
    ) -> None:
        """Verifica o comportamento de erro borda multiplos arquivos um so com violacao."""
        limpo = _write(tmp_path, "limpo.py", "x = 1\n")
        sujo = _write(tmp_path, "sujo.py", "# gerado com ChatGPT\nx = 1\n")
        assert no_plan_main([limpo, sujo]) == 1


# ── removed-paths ────────────────────────────────────────────────────────────


def test_parses_deletion_and_rename_records() -> None:
    raw = b"D\0documents/old.md\0R100\0docs/old.md\0docs/new.md\0"
    assert _staged_removed_paths(raw) == [
        RemovedPath("documents/old.md"),
        RemovedPath("docs/old.md", "docs/new.md"),
    ]


def test_reports_reference_to_deleted_path() -> None:
    files = {
        "README.md": "See documents/old.md for details.\n",
        "documents/old.md": "Historical content.\n",
    }
    assert _find_references(files, [RemovedPath("documents/old.md")]) == [
        ("README.md", 1, "documents/old.md", "See documents/old.md for details.")
    ]


def test_ignores_deleted_file_and_clean_snapshot() -> None:
    files = {"documents/old.md": "documents/old.md\n", "README.md": "Nothing here.\n"}
    assert _find_references(files, [RemovedPath("documents/old.md")]) == []


def test_reports_backslash_reference_to_renamed_path() -> None:
    files = {"config.json": '{"source": "docs\\old.md"}\n'}
    assert _find_references(files, [RemovedPath("docs/old.md", "docs/new.md")]) == [
        ("config.json", 1, "docs/old.md", '{"source": "docs\\old.md"}')
    ]


def test_ignores_staged_submodule_gitlink(monkeypatch) -> None:
    calls: list[tuple[str, ...]] = []

    def fake_git_output(*args: str) -> bytes:
        calls.append(args)
        if args == ("ls-files", "-s", "-z"):
            return b"".join(
                (
                    b"160000 abc123 0\tvendor/submodule\0",
                    b"100644 def456 0\tREADME.md\0",
                )
            )
        assert args == ("show", ":README.md")
        return b"README content\n"

    monkeypatch.setattr("checks._git_output", fake_git_output)

    assert _staged_files() == {"README.md": "README content\n"}
    assert ("show", ":vendor/submodule") not in calls
