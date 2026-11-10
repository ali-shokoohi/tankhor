use std::str::FromStr;

use kernel::locale::{Direction, Locale, Theme};

#[test]
fn locale_direction_is_derived() {
    assert_eq!(Locale::Fa.as_tag(), "fa");
    assert_eq!(Locale::En.as_tag(), "en");
    assert_eq!(Locale::Fa.direction(), Direction::Rtl);
    assert_eq!(Locale::En.direction(), Direction::Ltr);
    assert_eq!(Direction::Rtl.as_str(), "rtl");
    assert_eq!(Direction::Ltr.as_str(), "ltr");

    assert_eq!(Locale::from_str("fa").expect("fa parses"), Locale::Fa);
    assert_eq!(Locale::from_str("en").expect("en parses"), Locale::En);
    assert!(Locale::from_str("de").is_err());
    assert!(Locale::from_str("").is_err());

    assert_eq!(Theme::Light.as_str(), "light");
    assert_eq!(Theme::Dark.as_str(), "dark");
    assert_eq!(Theme::System.as_str(), "system");
    assert_eq!(
        Theme::from_str("light").expect("light parses"),
        Theme::Light
    );
    assert_eq!(Theme::from_str("dark").expect("dark parses"), Theme::Dark);
    assert_eq!(
        Theme::from_str("system").expect("system parses"),
        Theme::System
    );
    assert!(Theme::from_str("solarized").is_err());

    let fa_json = serde_json::to_string(&Locale::Fa).expect("locale serializes");
    assert_eq!(fa_json, "\"fa\"");
    let decoded_locale: Locale = serde_json::from_str(&fa_json).expect("locale deserializes");
    assert_eq!(decoded_locale.direction(), Direction::Rtl);

    let system_json = serde_json::to_string(&Theme::System).expect("theme serializes");
    assert_eq!(system_json, "\"system\"");
    let decoded_theme: Theme = serde_json::from_str(&system_json).expect("theme deserializes");
    assert_eq!(decoded_theme, Theme::System);
}
