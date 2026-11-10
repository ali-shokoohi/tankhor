use kernel::locale::Locale;
use web::i18n::{ConsoleCatalog, I18nErrorKind};

#[test]
fn foundation_copy_has_fa_en_parity_and_context() {
    let catalog = ConsoleCatalog::load_authoritative()
        .expect("authoritative console Fluent bundles must load with FA/EN parity");

    // Required foundation keys across base shell, theme, language, validation, accessible names,
    // and transport limits must exist in BOTH locales with non-empty localized strings.
    let required_keys = [
        "brand-name",
        "console-title",
        "console-foundation-heading",
        "console-foundation-status",
        "skip-to-main",
        "nav-primary-label",
        "preferences-form-label",
        "language-label",
        "language-fa",
        "language-en",
        "theme-label",
        "theme-light",
        "theme-dark",
        "theme-system",
        "preferences-apply",
        "validation-required",
        "validation-invalid-value",
        "error-csrf-invalid",
        "error-body-too-large",
        "error-service-busy",
    ];

    for key in required_keys {
        let fa = catalog
            .format(Locale::Fa, key, &[])
            .unwrap_or_else(|e| panic!("FA key `{key}` failed: {e}"));
        let en = catalog
            .format(Locale::En, key, &[])
            .unwrap_or_else(|e| panic!("EN key `{key}` failed: {e}"));
        assert!(!fa.trim().is_empty(), "FA key `{key}` must not be empty");
        assert!(!en.trim().is_empty(), "EN key `{key}` must not be empty");

        // Every message must carry `-surface` and `-situation` context attributes in both locales.
        let fa_meta = catalog
            .metadata(Locale::Fa, key)
            .unwrap_or_else(|| panic!("missing FA metadata for `{key}`"));
        let en_meta = catalog
            .metadata(Locale::En, key)
            .unwrap_or_else(|| panic!("missing EN metadata for `{key}`"));
        assert_eq!(fa_meta.surface(), "console");
        assert_eq!(en_meta.surface(), "console");
        assert!(!fa_meta.situation().is_empty());
        assert_eq!(fa_meta.situation(), en_meta.situation());
    }

    // Brand naming guardrail: Tankhor / تن‌خور (with ZWNJ), never Taneh / تنه, and no emoji in
    // validation/error/security messages.
    let brand_fa = catalog
        .format(Locale::Fa, "brand-name", &[])
        .expect("brand_fa");
    let brand_en = catalog
        .format(Locale::En, "brand-name", &[])
        .expect("brand_en");
    assert_eq!(brand_fa, "تن\u{200c}خور");
    assert_eq!(brand_en, "Tankhor");

    // Parameterized validation message works in both locales.
    let fa_field_err = catalog
        .format(Locale::Fa, "validation-field-error", &[("field", "تلفن")])
        .expect("fa parameterized");
    let en_field_err = catalog
        .format(Locale::En, "validation-field-error", &[("field", "Phone")])
        .expect("en parameterized");
    assert!(fa_field_err.contains("تلفن"));
    assert!(en_field_err.contains("Phone"));

    // Missing key MUST return an explicit error (never silent English fallback for FA!).
    let missing_err = catalog
        .format(Locale::Fa, "nonexistent-key", &[])
        .expect_err("missing key must fail explicitly");
    assert_eq!(missing_err.kind(), I18nErrorKind::MissingMessage);

    // Negative parity check 1: key present in EN but missing in FA must fail catalog construction
    // (no silent English fallback).
    let missing_fa = ConsoleCatalog::from_sources(
        "only-shared = مقدار\n    .surface = console\n    .situation = shell\n",
        "only-shared = Value\n    .surface = console\n    .situation = shell\nen-only = Extra\n    .surface = console\n    .situation = shell\n",
    )
    .expect_err("EN-only key must be rejected");
    assert_eq!(missing_fa.kind(), I18nErrorKind::KeyParityMismatch);

    // Negative parity check 2: mismatched placeable variables between FA and EN must fail.
    let mismatched_vars = ConsoleCatalog::from_sources(
        "msg = مقدار { $count }\n    .surface = console\n    .situation = validation\n",
        "msg = Value { $total }\n    .surface = console\n    .situation = validation\n",
    )
    .expect_err("mismatched placeables must be rejected");
    assert_eq!(
        mismatched_vars.kind(),
        I18nErrorKind::VariableParityMismatch
    );

    // Negative context check 3: missing `.surface` or `.situation` attribute must fail.
    let missing_context = ConsoleCatalog::from_sources("msg = مقدار\n", "msg = Value\n")
        .expect_err("missing surface/situation metadata must be rejected");
    assert_eq!(
        missing_context.kind(),
        I18nErrorKind::MissingContextMetadata
    );
}
