"""T069: FR-038/FR-UX-20/AC-ADM-047 CSS generation from tokens.toml and stray visual literal scanner."""

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCANNER = ROOT / "scripts/check-design-values.py"


class GeneratedTokensTest(unittest.TestCase):
    def test_token_css_is_generated_and_no_stray_hex_is_accepted(self):
        # 1. Scanner script and crates/web build script must exist.
        self.assertTrue(SCANNER.is_file(), "scripts/check-design-values.py is missing")
        self.assertTrue((ROOT / "crates/web/build.rs").is_file(), "crates/web/build.rs is missing")

        # 2. Scanner must pass on the clean repository.
        clean_scan = subprocess.run(
            [sys.executable, str(SCANNER), "--root", str(ROOT)],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(
            clean_scan.returncode,
            0,
            f"check-design-values.py failed on clean repo:\n{clean_scan.stdout}\n{clean_scan.stderr}",
        )

        # 3. Inject stray literals into disposable template, CSS, and brand SVG fixtures -> must fail!
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp_root = Path(tmpdir)
            for rel, bad_content in (
                ("crates/web/templates/console/bad.html", '<div style="color: #F8FAFC">x</div>\n'),
                ("crates/web/static/bad.css", ".card { background: #185B7A; }\n"),
                ("crates/web/static/rgb.css", ".card { color: rgb(24, 91, 122); }\n"),
                (
                    "crates/web/static/brand/bad.svg",
                    '<svg viewBox="0 0 24 24"><rect width="24" height="24" fill="#123456"/></svg>\n',
                ),
            ):
                fixture = tmp_root / rel
                fixture.parent.mkdir(parents=True, exist_ok=True)
                # Ensure tokens.toml exists in the disposable root so single-source check knows it.
                tokens_copy = tmp_root / "crates/web/design/tokens.toml"
                tokens_copy.parent.mkdir(parents=True, exist_ok=True)
                tokens_copy.write_text(
                    (ROOT / "crates/web/design/tokens.toml").read_text(encoding="utf-8"),
                    encoding="utf-8",
                )
                fixture.write_text(bad_content, encoding="utf-8")

                rejected = subprocess.run(
                    [sys.executable, str(SCANNER), "--root", str(tmp_root)],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertNotEqual(
                    rejected.returncode,
                    0,
                    f"scanner accepted stray literal in {rel}: {bad_content!r}",
                )
                fixture.unlink()

            # A brand SVG whose fill colours come exclusively from tokens.toml (supplied artwork) is preserved.
            valid_svg = tmp_root / "crates/web/static/brand/mark.svg"
            valid_svg.parent.mkdir(parents=True, exist_ok=True)
            valid_svg.write_text(
                '<svg viewBox="0 0 24 24"><title>Tankhor</title><desc>Mark</desc><path fill="#4A2F27" d="M0 0h24v24H0z"/></svg>\n',
                encoding="utf-8",
            )
            valid_svg_scan = subprocess.run(
                [sys.executable, str(SCANNER), "--root", str(tmp_root)],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(
                valid_svg_scan.returncode,
                0,
                f"scanner rejected approved token colour in supplied brand SVG:\n{valid_svg_scan.stdout}",
            )

        # 4. Verify `crates/web` compiles and generates CSS custom properties in OUT_DIR from tokens.toml.
        web_check = subprocess.run(
            ["cargo", "test", "--offline", "-p", "web", "--lib", "generated_css_contains_all_tokens"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(
            web_check.returncode,
            0,
            f"web generated CSS check failed:\n{web_check.stdout}\n{web_check.stderr}",
        )


if __name__ == "__main__":
    unittest.main()
