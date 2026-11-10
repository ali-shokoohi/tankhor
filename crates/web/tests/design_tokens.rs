use web::design::{ContrastErrorKind, DesignTokens, ThemeMode};

#[test]
fn visual_identity_values_have_one_source_and_checked_contrast() {
    let tokens = DesignTokens::load_authoritative().expect("authoritative tokens.toml must parse");
    tokens
        .validate_all()
        .expect("authoritative tokens.toml must pass all WCAG 2.2 AA checks");

    // Mutate a normal-text pair below 4.5:1 (e.g. near-unrounded boundary ~4.46:1) -> must fail
    // because unrounded contrast ratios are enforced!
    let mut low_normal = tokens.clone();
    low_normal
        .set_theme_color(ThemeMode::Light, "textSecondary", "#777777")
        .expect("set_theme_color");
    let err = low_normal
        .validate_all()
        .expect_err("normal text below 4.5:1 must fail");
    assert!(matches!(err.kind(), ContrastErrorKind::TextContrastTooLow));

    // Specifically test an unrounded borderline ratio (e.g., #777777 on #FFFFFF is ~4.478:1,
    // which rounds to 4.5 at 1 decimal place, MUST still fail unrounded >= 4.5!).
    let ratio_borderline = web::design::contrast_ratio_hex("#777777", "#FFFFFF").expect("hex");
    assert!(
        ratio_borderline > 4.45 && ratio_borderline < 4.5,
        "expected borderline ratio in (4.45, 4.5), got {ratio_borderline}"
    );

    // Mutate a large-text pair below 3.0:1 -> must fail.
    let mut low_large = tokens.clone();
    low_large
        .set_theme_color(ThemeMode::Light, "textPrimary", "#969696")
        .expect("set_theme_color");
    let large_err = low_large
        .validate_all()
        .expect_err("large text below 3.0:1 must fail");
    assert!(matches!(
        large_err.kind(),
        ContrastErrorKind::TextContrastTooLow
    ));

    // Mutate `borderControl` below 3.0:1 against surface -> must fail control/focus boundary check.
    let mut low_control = tokens.clone();
    low_control
        .set_theme_color(ThemeMode::Light, "borderControl", "#9A9A9A")
        .expect("set_theme_color");
    let ctrl_err = low_control
        .validate_all()
        .expect_err("control boundary below 3.0:1 must fail");
    assert!(matches!(
        ctrl_err.kind(),
        ContrastErrorKind::ControlContrastTooLow
    ));

    // Using decorative `border` as a control boundary must be rejected explicitly.
    let mut decorative_boundary = tokens.clone();
    decorative_boundary.add_control_boundary("illegal_decorative_control", "border", "surface");
    let dec_err = decorative_boundary
        .validate_all()
        .expect_err("decorative `border` must never be sole control boundary");
    assert!(matches!(
        dec_err.kind(),
        ContrastErrorKind::DecorativeTokenUsedAsControlBoundary
    ));
}

#[test]
fn foundation_brand_and_contrast_follow_approved_baseline() {
    let tokens = DesignTokens::load_authoritative().expect("authoritative tokens.toml must parse");

    // Brand naming: Tankhor / تن‌خور (with ZWNJ \u{200c}), never Taneh / تنه.
    assert_eq!(tokens.brand_fa(), "تن\u{200c}خور");
    assert_eq!(tokens.brand_en(), "Tankhor");
    assert!(!tokens.brand_fa().contains("تنه"));
    assert!(!tokens.brand_en().to_ascii_lowercase().contains("taneh"));

    // Font family uses Vazirmatn first, never generic SF Pro replacement.
    assert_eq!(
        tokens.font_stack().first().map(String::as_str),
        Some("Vazirmatn")
    );
    assert!(
        !tokens
            .font_stack()
            .iter()
            .any(|f| f.to_ascii_lowercase().contains("sf pro")),
        "SF Pro replacement is prohibited"
    );

    // Prohibitions: white small text (`surface`) on light `sky` must be rejected if declared as a text pair.
    let mut white_on_sky = tokens.clone();
    white_on_sky.add_text_pair(
        "illegal_white_on_sky",
        "surface",
        "sky",
        web::design::TextSizeRole::Normal,
    );
    let sky_err = white_on_sky
        .validate_all()
        .expect_err("white small text on light sky must be rejected");
    assert!(matches!(
        sky_err.kind(),
        ContrastErrorKind::ProhibitedPairUsed | ContrastErrorKind::TextContrastTooLow
    ));
}
