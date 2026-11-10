#!/usr/bin/env python3
"""Verify that crates/web/design/tokens.toml is the single source of visual literals."""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys
import tomllib

HEX_COLOR_RE = re.compile(r"#(?:[0-9A-Fa-f]{3}|[0-9A-Fa-f]{4}|[0-9A-Fa-f]{6}|[0-9A-Fa-f]{8})\b")
COLOR_FN_RE = re.compile(r"\b(?:rgb|rgba|hsl|hsla|oklch|oklab|lab|lch)\s*\(", re.IGNORECASE)


def load_approved_hex_colors(tokens_path: Path) -> set[str]:
    if not tokens_path.is_file():
        raise SystemExit(f"missing authoritative token file: {tokens_path}")
    data = tomllib.loads(tokens_path.read_text(encoding="utf-8"))
    colors = data.get("colors", {})
    approved: set[str] = set()
    for theme_name in ("light", "dark"):
        theme = colors.get(theme_name, {})
        for value in theme.values():
            if isinstance(value, str) and HEX_COLOR_RE.fullmatch(value):
                approved.add(value.upper())
    if not approved:
        raise SystemExit(f"no approved colours loaded from {tokens_path}")
    return approved


def check_text_for_forbidden_literals(
    rel_path: Path,
    text: str,
    *,
    allow_approved_hex: set[str] | None = None,
) -> list[str]:
    errors: list[str] = []
    for match in HEX_COLOR_RE.finditer(text):
        token = match.group(0).upper()
        if allow_approved_hex is not None and token in allow_approved_hex:
            continue
        errors.append(f"{rel_path}: forbidden hex colour literal `{match.group(0)}`")
    for match in COLOR_FN_RE.finditer(text):
        errors.append(f"{rel_path}: forbidden colour function `{match.group(0)}`")
    return errors


def scan_root(root: Path) -> list[str]:
    tokens_path = root / "crates/web/design/tokens.toml"
    approved_hex = load_approved_hex_colors(tokens_path)
    errors: list[str] = []

    web_root = root / "crates/web"
    if not web_root.is_dir():
        return errors

    for path in sorted(web_root.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(root)
        parts = rel.parts

        # Canonical token file is the sole source of truth.
        if rel == Path("crates/web/design/tokens.toml"):
            continue
        # Third-party vendored files (fonts, htmx, licenses) are not authored visual sources.
        if "vendor" in parts:
            continue
        # Test files may construct negative contrast/mutation inputs.
        if "tests" in parts:
            continue

        suffix = path.suffix.lower()
        if suffix not in {".html", ".css", ".svg", ".rs"}:
            continue

        text = path.read_text(encoding="utf-8")

        # Supplied brand artwork under crates/web/static/brand/*.svg may use only approved token colours.
        if "brand" in parts and suffix == ".svg":
            errors.extend(
                check_text_for_forbidden_literals(
                    rel,
                    text,
                    allow_approved_hex=approved_hex,
                )
            )
            continue

        # All authored templates, stylesheets, SVGs, and Rust sources in crates/web must use CSS variables / tokens.toml.
        errors.extend(check_text_for_forbidden_literals(rel, text))

    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()

    errors = scan_root(args.root.resolve())
    if errors:
        for err in errors:
            print(err, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
