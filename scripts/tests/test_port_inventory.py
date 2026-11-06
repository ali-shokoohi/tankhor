"""T030: source-backed port/contract/mock inventory and real/fake feature graphs."""

from pathlib import Path
import tempfile
import unittest

from traceability_fixture import PORTS, fixture
from test_traceability import CHECKER, load_checker


class PortInventoryTest(unittest.TestCase):
    def test_every_port_has_fake_real_contract_and_service_mock_evidence(self):
        self.assertTrue(CHECKER.is_file(), "actual-symbol inventory checker is missing")
        checker = load_checker()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            feature = fixture(root, complete=True)
            self.assertEqual(checker.check(root, feature, "final"), [])
            for port in PORTS:
                snake = port.lower()
                paths = [f"crates/context/src/ports/{snake}.rs",
                         f"crates/testkit/src/contracts/{snake}.rs",
                         f"crates/testkit/src/fakes/{snake}.rs",
                         f"crates/testkit/tests/{snake}_fake.rs",
                         f"crates/infra-system/src/{snake}.rs",
                         f"crates/infra-system/tests/{snake}.rs",
                         f"crates/testkit/tests/mock_{snake}.rs"]
                for relative in paths:
                    with self.subTest(port=port, missing=relative):
                        path = root / relative
                        original = path.read_text()
                        path.write_text("// filename and labels are not implementation evidence\n")
                        self.assertTrue(checker.check(root, feature, "final"), relative)
                        path.write_text(original)
            for port in ("IdGenerator", "AuditRepository"):
                path = root / f"crates/testkit/tests/mock_{port.lower()}.rs"
                original = path.read_text()
                for replacement in (original.replace("times(1)", "times(0)"),
                                    original.replace("expect_act()", "expect_unrelated_query()"),
                                    original.replace("service(&port).await;", "// service(&port).await;")):
                    path.write_text(replacement)
                    self.assertTrue(checker.check(root, feature, "final"), "zero-call/query-only evidence")
                path.write_text(original)
                service = root / f"crates/context/src/application/{port.lower()}.rs"
                original_service = service.read_text()
                service.write_text(f'pub async fn service(port: &impl {port}) {{ list_settings().await; }}\n')
                self.assertTrue(checker.check(root, feature, "final"),
                                "read-only query cannot stand in for ID generation or audit append")
                service.write_text(original_service)
            sms = root / "crates/infra-system/tests/smssender.rs"
            original = sms.read_text()
            sms.write_text(original.splitlines()[0])
            self.assertTrue(checker.check(root, feature, "final"), "both SMS adapters need runners")
            sms.write_text(original)
            manifest = root / "crates/app/Cargo.toml"
            original = manifest.read_text()
            manifest.write_text(original.replace("default=[]", 'default=["dev-fakes"]'))
            self.assertTrue(checker.check(root, feature, "final"), "real graph cannot include fakes")
            manifest.write_text(original.replace('"testkit/fakes"', '"testkit/missing"'))
            self.assertTrue(checker.check(root, feature, "final"), "fake graph must resolve with fakes")
            manifest.write_text(original)
            self.assertEqual(checker.check(root, feature, "final"), [])


if __name__ == "__main__":
    unittest.main()
