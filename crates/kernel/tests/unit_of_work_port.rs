use kernel::error::{DomainError, ErrorCategory, ErrorCode};
use kernel::ports::unit_of_work::{MockUnitOfWork, StoreId, TransactionMode, TxHandle, UnitOfWork};

#[derive(Debug)]
struct SampleBackendTx {
    value: u32,
}

#[derive(Debug)]
struct OtherBackendTx;

fn assert_uow_automock_above_async_trait() {
    let trait_source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/ports/unit_of_work.rs"
    ))
    .expect("unit_of_work port file must exist");
    let automock_pos = trait_source
        .find("cfg_attr(any(test, feature = \"mocks\"), mockall::automock)")
        .expect("automock attribute must be present");
    let async_trait_pos = trait_source
        .find("async_trait::async_trait")
        .expect("async_trait attribute must be present");
    assert!(
        automock_pos < async_trait_pos,
        "mockall::automock must appear before async_trait::async_trait"
    );
}

async fn verify_uow_mock_lifecycle() {
    let mock_store = StoreId::new();
    let mut mock_uow = MockUnitOfWork::new();
    mock_uow
        .expect_begin()
        .withf(|mode| *mode == TransactionMode::ReadWrite)
        .times(1)
        .returning(move |mode| {
            Ok(TxHandle::new(
                mock_store,
                mode,
                SampleBackendTx { value: 9 },
            ))
        });
    mock_uow.expect_commit().times(1).returning(move |mut tx| {
        let _ = tx.take::<SampleBackendTx>(mock_store)?;
        Ok(())
    });

    let uow: &dyn UnitOfWork = &mock_uow;
    let tx = uow
        .begin(TransactionMode::ReadWrite)
        .await
        .expect("begin returns TxHandle");
    uow.commit(tx).await.expect("consuming commit succeeds");

    let mut mock_unknown = MockUnitOfWork::new();
    mock_unknown.expect_begin().times(1).returning(move |mode| {
        Ok(TxHandle::new(
            mock_store,
            mode,
            SampleBackendTx { value: 0 },
        ))
    });
    mock_unknown
        .expect_commit()
        .times(1)
        .returning(|_| Err(DomainError::unknown_commit(ErrorCode::CommitOutcomeUnknown)));
    let tx2 = mock_unknown
        .begin(TransactionMode::ReadWrite)
        .await
        .expect("begin 2");
    let unknown_err = mock_unknown
        .commit(tx2)
        .await
        .expect_err("unknown commit outcome distinct from rollback");
    assert_eq!(unknown_err.category(), ErrorCategory::UnknownCommit);
    assert_eq!(unknown_err.code(), ErrorCode::CommitOutcomeUnknown);
}

#[tokio::test]
async fn uow_port_has_consuming_transaction_lifecycle() {
    let store_a = StoreId::new();
    let store_b = StoreId::new();
    assert_ne!(store_a, store_b);

    let mut handle = TxHandle::new(
        store_a,
        TransactionMode::ReadWrite,
        SampleBackendTx { value: 7 },
    );
    assert_eq!(handle.store_id(), store_a);
    assert_eq!(handle.mode(), TransactionMode::ReadWrite);
    assert!(!handle.is_finished());

    let wrong_store = handle
        .downcast_mut::<SampleBackendTx>(store_b)
        .expect_err("wrong store must return typed error");
    assert_eq!(wrong_store.category(), ErrorCategory::InvalidHandle);
    assert_eq!(wrong_store.code(), ErrorCode::ForeignTransactionHandle);

    let wrong_backend = handle
        .downcast_mut::<OtherBackendTx>(store_a)
        .expect_err("wrong backend type must return typed error");
    assert_eq!(wrong_backend.category(), ErrorCategory::InvalidHandle);
    assert_eq!(wrong_backend.code(), ErrorCode::ForeignTransactionHandle);

    {
        let tx = handle
            .downcast_mut::<SampleBackendTx>(store_a)
            .expect("matching store and backend");
        tx.value = 42;
    }

    let taken = handle
        .take::<SampleBackendTx>(store_a)
        .expect("take extracts payload once");
    assert_eq!(taken.value, 42);
    assert!(handle.is_finished());

    let finished_err = handle
        .downcast_mut::<SampleBackendTx>(store_a)
        .expect_err("finished handle must return typed error");
    assert_eq!(finished_err.category(), ErrorCategory::InvalidHandle);
    assert_eq!(finished_err.code(), ErrorCode::FinishedTransactionHandle);

    let ro_handle = TxHandle::new(
        store_a,
        TransactionMode::ReadOnly,
        SampleBackendTx { value: 1 },
    );
    let ro_err = ro_handle
        .require_writable()
        .expect_err("read-only handle must reject write requirement");
    assert_eq!(ro_err.category(), ErrorCategory::InvalidHandle);
    assert_eq!(ro_err.code(), ErrorCode::ReadOnlyTransaction);

    verify_uow_mock_lifecycle().await;
    assert_uow_automock_above_async_trait();
}
