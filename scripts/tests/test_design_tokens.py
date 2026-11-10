"""T067: FR-028/FR-038/FR-UX-07/FR-UX-20/AC-ADM-047 authoritative design token source validation."""

from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]
TOKENS_PATH = ROOT / "crates/web/design/tokens.toml"

EXPECTED_LIGHT_COLORS = {
    "background": "#F8FAFC",
    "surface": "#FFFFFF",
    "surfaceMuted": "#EEF4F8",
    "textPrimary": "#18232D",
    "textSecondary": "#506273",
    "brandVelvet": "#4A2F27",
    "curtainBase": "#4A2F27",
    "sky": "#BDE4F7",
    "action": "#185B7A",
    "onAction": "#FFFFFF",
    "border": "#CCD6DF",
    "borderControl": "#185B7A",
    "onCurtain": "#F1F5F9",
    "success": "#146C43",
    "warning": "#8A4B08",
    "danger": "#B42318",
}

EXPECTED_DARK_COLORS = {
    "background": "#101820",
    "surface": "#18232D",
    "surfaceMuted": "#22323F",
    "textPrimary": "#F1F5F9",
    "textSecondary": "#BCCAD6",
    "brandVelvet": "#D6B5A8",
    "curtainBase": "#4A2F27",
    "sky": "#274E66",
    "action": "#A7DBF3",
    "onAction": "#102B3B",
    "border": "#506273",
    "borderControl": "#A7DBF3",
    "onCurtain": "#F1F5F9",
    "success": "#80D8A6",
    "warning": "#F3C278",
    "danger": "#FFB4AB",
}


class DesignTokensTest(unittest.TestCase):
    def test_token_set_matches_authoritative_semantic_roles(self):
        self.assertTrue(TOKENS_PATH.is_file(), "crates/web/design/tokens.toml is missing")
        tokens = tomllib.loads(TOKENS_PATH.read_text(encoding="utf-8"))

        # 1. Exact 16 semantic colour roles in both light and dark themes (§12.2).
        colors = tokens["colors"]
        self.assertEqual(set(colors["light"].keys()), set(EXPECTED_LIGHT_COLORS.keys()))
        self.assertEqual(set(colors["dark"].keys()), set(EXPECTED_DARK_COLORS.keys()))
        self.assertEqual(colors["light"], EXPECTED_LIGHT_COLORS)
        self.assertEqual(colors["dark"], EXPECTED_DARK_COLORS)
        for theme_map in (colors["light"], colors["dark"]):
            for hex_val in theme_map.values():
                self.assertRegex(hex_val, r"^#[0-9A-F]{6}$")

        # 2. Typography family and §12.3 scales with proposed sizes explicitly labeled proposed.
        typography = tokens["typography"]
        self.assertEqual(
            typography["font_stack"],
            ["Vazirmatn", "Arial", "Tahoma", "sans-serif"],
        )
        scale = typography["scale"]
        expected_type_roles = {
            "hero_desktop": (48, 600, 1.4),
            "hero_mobile": (32, 600, 1.4),
            "section_large": (28, 600, 1.5),
            "section_small": (24, 600, 1.5),
            "body": (16, 400, 1.8),
            "control": (16, 500, 1.6),
            "caption": (14, 400, 1.7),
        }
        self.assertEqual(set(scale.keys()), set(expected_type_roles.keys()))
        for role, (size_px, weight, line_height) in expected_type_roles.items():
            entry = scale[role]
            self.assertEqual(entry["size_px"], size_px)
            self.assertEqual(entry["weight"], weight)
            self.assertAlmostEqual(entry["line_height"], line_height)
            self.assertEqual(entry["status"], "proposed")

        # 3. Spacing, radii, targets, icons, and breakpoints (§12.1 / §12.3).
        self.assertEqual(tokens["spacing"]["scale_px"], [4, 8, 12, 16, 24, 32, 48, 64])
        self.assertEqual(tokens["radii"]["control_px"], 8)
        self.assertEqual(tokens["radii"]["panel_px"], 16)
        self.assertEqual(tokens["targets"]["primary_button_height_px"], 48)
        self.assertEqual(tokens["targets"]["touch_target_min_px"], 44)
        self.assertEqual(tokens["icons"]["canvas_px"], 24)
        self.assertAlmostEqual(tokens["icons"]["stroke_min_px"], 1.5)
        self.assertAlmostEqual(tokens["icons"]["stroke_max_px"], 2.0)

        # 4. Motion values (§12.4) kept labeled proposed.
        motion = tokens["motion"]
        self.assertEqual(motion["control_feedback"]["duration_ms"], 180)
        self.assertEqual(motion["control_feedback"]["status"], "proposed")
        self.assertEqual(motion["curtain_continuity"]["duration_ms"], 500)
        self.assertEqual(motion["curtain_continuity"]["status"], "proposed")

        # 5. Declared contrast pairs, large-text roles, and control/decorative boundary rules.
        contrast = tokens["contrast"]
        self.assertGreaterEqual(len(contrast["text_pairs"]), 8)
        for pair in contrast["text_pairs"]:
            self.assertIn(pair["foreground"], EXPECTED_LIGHT_COLORS)
            self.assertIn(pair["background"], EXPECTED_LIGHT_COLORS)
            self.assertIn(pair["size_role"], ("normal", "large"))

        boundaries = contrast["control_boundaries"]
        self.assertGreaterEqual(len(boundaries), 2)
        for boundary in boundaries:
            self.assertEqual(boundary["token"], "borderControl")
            self.assertIn(boundary["against"], EXPECTED_LIGHT_COLORS)

        self.assertIn("border", contrast["decorative_only_tokens"])
        self.assertNotIn("border", {b["token"] for b in boundaries})


if __name__ == "__main__":
    unittest.main()
