use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use kernel::locale::{Locale, Theme};
use scraper::{Html, Selector};
use tower::ServiceExt;
use web::i18n::ConsoleCatalog;
use web::shell::{CONSOLE_STYLESHEET, FieldViewModel, ShellPreferences, ShellView, shell_router};

fn assert_logical_css_only() {
    assert!(
        CONSOLE_STYLESHEET.contains("padding-inline")
            && CONSOLE_STYLESHEET.contains("padding-block")
            && CONSOLE_STYLESHEET.contains("font-family: \"Vazirmatn\"")
    );
    for forbidden_physical in [
        "margin-left:",
        "margin-right:",
        "padding-left:",
        "padding-right:",
        "border-left:",
        "border-right:",
        "text-align: left",
        "text-align: right",
        "float: left",
        "float: right",
    ] {
        assert!(
            !CONSOLE_STYLESHEET.contains(forbidden_physical),
            "console.css must use CSS logical properties instead of `{forbidden_physical}`"
        );
    }
}

async fn assert_shell_router_serves_local_assets() {
    let app = shell_router(ConsoleCatalog::load_authoritative().unwrap());
    for (path, expected_ct_prefix) in [
        ("/console?lang=fa&theme=dark", "text/html"),
        ("/console?lang=en&theme=light", "text/html"),
        ("/static/tokens.css", "text/css"),
        ("/static/console.css", "text/css"),
        ("/static/vendor/vazirmatn.woff2", "font/woff2"),
        ("/static/vendor/OFL.txt", "text/plain"),
        ("/static/vendor/htmx.min.js", "application/javascript"),
        ("/static/vendor/htmx-LICENSE.txt", "text/plain"),
    ] {
        let req = Request::builder()
            .method("GET")
            .uri(path)
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "GET {path} must return 200");
        let ct = resp
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            ct.starts_with(expected_ct_prefix),
            "GET {path} content-type `{ct}` must start with `{expected_ct_prefix}`"
        );
        let bytes = to_bytes(resp.into_body(), 2 * 1024 * 1024).await.unwrap();
        assert!(!bytes.is_empty(), "GET {path} body must not be empty");
    }
}

fn assert_rendered_shell_case(
    catalog: &ConsoleCatalog,
    locale: Locale,
    theme: Theme,
    expected_lang: &str,
    expected_dir: &str,
    expected_theme: &str,
    expected_brand: &str,
) {
    let root_selector = Selector::parse("html").unwrap();
    let h1_sel = Selector::parse("h1").unwrap();
    let skip_sel = Selector::parse("a[href=\"#main-content\"]").unwrap();
    let main_sel = Selector::parse("main#main-content").unwrap();
    let nav_sel = Selector::parse("nav[aria-label]").unwrap();
    let label_sel = Selector::parse("label").unwrap();
    let script_sel = Selector::parse("script[src]").unwrap();
    let link_sel = Selector::parse("link[rel=\"stylesheet\"]").unwrap();
    let img_sel = Selector::parse("img, svg").unwrap();

    let prefs = ShellPreferences::new(locale, theme);
    let sample_field = FieldViewModel {
        id: "sample-note".to_owned(),
        name: "sample_note".to_owned(),
        label: catalog.format(locale, "language-label", &[]).unwrap(),
        value: String::new(),
        error: Some(catalog.format(locale, "validation-required", &[]).unwrap()),
        required: true,
    };
    let view = ShellView::foundation(catalog, prefs, "csrf-test-token", Some(sample_field))
        .expect("render foundation shell");
    let rendered = view.render_html().expect("Askama HTML render");
    let doc = Html::parse_document(&rendered);

    let root_node = doc.select(&root_selector).next().expect("<html> element");
    assert_eq!(root_node.value().attr("lang"), Some(expected_lang));
    assert_eq!(root_node.value().attr("dir"), Some(expected_dir));
    assert_eq!(root_node.value().attr("data-theme"), Some(expected_theme));

    let h1_nodes: Vec<_> = doc.select(&h1_sel).collect();
    assert_eq!(
        h1_nodes.len(),
        1,
        "every full page must have exactly one h1 ({locale:?}/{theme:?})"
    );
    let h1_text: String = h1_nodes[0].text().collect();
    assert!(
        h1_text.contains(expected_brand),
        "h1 `{h1_text}` must reference localized brand `{expected_brand}`"
    );

    assert!(doc.select(&skip_sel).next().is_some(), "missing skip link");
    assert!(doc.select(&main_sel).next().is_some(), "missing main");
    assert!(doc.select(&nav_sel).next().is_some(), "missing named nav");
    assert_eq!(doc.select(&img_sel).count(), 0, "no invented console logo");

    let labels: Vec<_> = doc.select(&label_sel).collect();
    assert!(labels.len() >= 3);
    for label in &labels {
        let target_id = label.value().attr("for").expect("explicit `for`");
        let target_sel = Selector::parse(&format!("#{target_id}")).unwrap();
        assert!(doc.select(&target_sel).next().is_some());
    }

    let err_input = doc
        .select(&Selector::parse("#sample-note").unwrap())
        .next()
        .expect("field partial control");
    assert_eq!(err_input.value().attr("aria-invalid"), Some("true"));
    let err_id = err_input
        .value()
        .attr("aria-describedby")
        .expect("aria-describedby");
    assert!(
        doc.select(&Selector::parse(&format!("#{err_id}")).unwrap())
            .next()
            .is_some()
    );

    let scripts: Vec<_> = doc.select(&script_sel).collect();
    assert!(!scripts.is_empty());
    for script in scripts {
        assert!(script.value().attr("src").unwrap().starts_with('/'));
    }
    for link in doc.select(&link_sel) {
        assert!(link.value().attr("href").unwrap().starts_with('/'));
    }
}

#[tokio::test]
async fn base_shell_has_locale_theme_and_accessible_structure() {
    let catalog = ConsoleCatalog::load_authoritative().expect("catalog");
    let cases = [
        (
            Locale::Fa,
            Theme::Light,
            "fa",
            "rtl",
            "light",
            "تن\u{200c}خور",
        ),
        (
            Locale::Fa,
            Theme::Dark,
            "fa",
            "rtl",
            "dark",
            "تن\u{200c}خور",
        ),
        (
            Locale::Fa,
            Theme::System,
            "fa",
            "rtl",
            "system",
            "تن\u{200c}خور",
        ),
        (Locale::En, Theme::Light, "en", "ltr", "light", "Tankhor"),
        (Locale::En, Theme::Dark, "en", "ltr", "dark", "Tankhor"),
        (Locale::En, Theme::System, "en", "ltr", "system", "Tankhor"),
    ];
    for (locale, theme, expected_lang, expected_dir, expected_theme, expected_brand) in cases {
        assert_rendered_shell_case(
            &catalog,
            locale,
            theme,
            expected_lang,
            expected_dir,
            expected_theme,
            expected_brand,
        );
    }
    assert_logical_css_only();
    assert_shell_router_serves_local_assets().await;
}
