//! T011: FR-024/FR-CON-37 fixture ownership and nonproduction endpoint isolation.

use testkit::fixtures::ownership::{Endpoint, FixtureScope, OwnershipError};

#[test]
fn fixture_ownership_rejects_foreign_resources() {
    let scope = FixtureScope::new("repository-a", "run-a").expect("valid fixture scope");
    let endpoint = Endpoint::new("127.0.0.1", 49152).expect("dynamic loopback endpoint");
    let labels = scope.postgres_labels();
    assert_eq!(scope.validate(&labels, endpoint, endpoint), Ok(()));

    for key in labels.keys() {
        let mut missing = labels.clone();
        missing.remove(key);
        assert_eq!(
            scope.validate(&missing, endpoint, endpoint),
            Err(OwnershipError::MissingLabel),
            "every ownership label is mandatory"
        );
        let mut foreign = labels.clone();
        foreign.insert(key.clone(), "foreign-or-production".to_owned());
        assert_eq!(
            scope.validate(&foreign, endpoint, endpoint),
            Err(OwnershipError::MismatchedLabel),
            "every ownership label must match exactly"
        );
    }

    for foreign in [
        FixtureScope::new("repository-ab", "run-a").unwrap(),
        FixtureScope::new("repository-a", "run-ab").unwrap(),
    ] {
        assert_eq!(
            scope.validate(&foreign.postgres_labels(), endpoint, endpoint),
            Err(OwnershipError::MismatchedLabel)
        );
    }

    let mut developer = std::collections::BTreeMap::new();
    developer.insert(
        "com.docker.compose.project".to_owned(),
        "tankhor".to_owned(),
    );
    assert_eq!(
        scope.validate(&developer, endpoint, endpoint),
        Err(OwnershipError::MissingLabel)
    );
    let mut with_metadata = labels.clone();
    with_metadata.insert("org.testcontainers".to_owned(), "true".to_owned());
    assert_eq!(scope.validate(&with_metadata, endpoint, endpoint), Ok(()));

    for host in [
        "0.0.0.0",
        "::",
        "192.168.1.2",
        "203.0.113.5",
        "database.production",
        "localhost.production",
        "127.0.0.1.production",
        "localhost",
        "[::1]",
    ] {
        assert_eq!(Endpoint::new(host, 49152), Err(OwnershipError::NonLoopback));
    }
    assert_eq!(
        Endpoint::new("127.0.0.1", 0),
        Err(OwnershipError::InvalidPort)
    );
    let ipv6 = Endpoint::new("::1", 49152).unwrap();
    assert_eq!(scope.validate(&labels, ipv6, ipv6), Ok(()));
    let another = Endpoint::new("127.0.0.1", 49153).unwrap();
    assert_eq!(
        scope.validate(&labels, another, endpoint),
        Err(OwnershipError::WrongEndpoint)
    );
    assert_eq!(
        scope.validate(&labels, ipv6, endpoint),
        Err(OwnershipError::WrongEndpoint)
    );

    for (project, run) in [
        ("", "run"),
        ("repo", ""),
        ("repo\nother", "run"),
        ("../repo", "run"),
        ("repo", "run=other"),
    ] {
        assert_eq!(
            FixtureScope::new(project, run),
            Err(OwnershipError::InvalidScope)
        );
    }
}
