//! Tankhor web delivery layer (bilingual HTML shell, design tokens, and `/api/v1` transport).

pub mod api;
pub mod console;
pub mod design;
pub mod i18n;
pub mod middleware;
pub mod security;
pub mod shell;

/// CSS custom properties generated at build time from `crates/web/design/tokens.toml`.
pub const GENERATED_TOKENS_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/tokens.css"));

#[cfg(test)]
mod tests {
    use super::GENERATED_TOKENS_CSS;

    #[test]
    fn generated_css_contains_all_tokens() {
        for expected in [
            "--tk-color-background:",
            "--tk-color-surface:",
            "--tk-color-surface-muted:",
            "--tk-color-text-primary:",
            "--tk-color-text-secondary:",
            "--tk-color-brand-velvet:",
            "--tk-color-curtain-base:",
            "--tk-color-sky:",
            "--tk-color-action:",
            "--tk-color-on-action:",
            "--tk-color-border:",
            "--tk-color-border-control:",
            "--tk-color-on-curtain:",
            "--tk-color-success:",
            "--tk-color-warning:",
            "--tk-color-danger:",
            "--tk-font-sans:",
            "--tk-type-hero-desktop-size:",
            "--tk-type-body-size:",
            "--tk-space-4:",
            "--tk-space-64:",
            "--tk-radius-control:",
            "--tk-radius-panel:",
            "--tk-target-primary-button:",
            "--tk-target-touch-min:",
            "--tk-motion-control-feedback: 180ms;",
            "--tk-motion-curtain-continuity: 500ms;",
            ":root[data-theme=\"dark\"]",
        ] {
            assert!(
                GENERATED_TOKENS_CSS.contains(expected),
                "GENERATED_TOKENS_CSS missing expected fragment `{expected}`"
            );
        }
    }
}
