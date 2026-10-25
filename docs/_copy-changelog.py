"""
Copy ../CHANGELOG.md into the site as _changelog.md before each render.

Quarto runs this as a pre-render script from the docs/ folder. The file's
own "# Changelog" title and intro line are dropped because changelog.qmd
supplies its own. Release headings are shifted down one level so they sit
under the page title.
"""
from pathlib import Path

source = Path(__file__).resolve().parent.parent / "CHANGELOG.md"
target = Path(__file__).resolve().parent / "_changelog.md"

lines = source.read_text(encoding="utf-8").splitlines()
start = next((i for i, line in enumerate(lines) if line.startswith("## ")), 0)
body = "\n".join(lines[start:]) + "\n"
target.write_text(body, encoding="utf-8")
