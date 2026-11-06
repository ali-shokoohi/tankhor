"""T026: FR-024/FR-CON-37 developer-stack health, secrets and role separation."""

import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
COMPOSE = ROOT / "deploy/compose.yml"


class ComposeTest(unittest.TestCase):
    def test_compose_keeps_migration_and_runtime_roles_separate(self):
        self.assertTrue(COMPOSE.is_file(), "developer Compose stack is missing")
        with tempfile.TemporaryDirectory(prefix="tankhor-compose-config-") as directory:
            environment = dict(os.environ)
            for secret in ("POSTGRES_ADMIN_PASSWORD", "POSTGRES_MIGRATION_PASSWORD",
                           "POSTGRES_RUNTIME_PASSWORD", "POSTGRES_PREPARATION_PASSWORD",
                           "MINIO_ROOT_USER", "MINIO_ROOT_PASSWORD"):
                path = Path(directory) / secret
                path.write_text("synthetic-config-test-value")
                path.chmod(0o600)
                environment[secret + "_FILE"] = str(path)
            result = subprocess.run(
                ["docker", "compose", "-f", str(COMPOSE), "config", "--format", "json"],
                env=environment, capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            config = json.loads(result.stdout)
        self.assertEqual(config["name"], "tankhor-dev")
        self.assertEqual(set(config["services"]), {"postgres", "minio"})
        self.assertTrue(config["services"]["postgres"]["image"].startswith("postgres:16@sha256:"))
        for name, service in config["services"].items():
            with self.subTest(service=name):
                self.assertRegex(service["image"], r"@sha256:[a-f0-9]{64}$")
                self.assertTrue(service["healthcheck"]["test"])
                self.assertGreater(service["healthcheck"]["retries"], 0)
                self.assertFalse(service.get("privileged", False))
                self.assertFalse(any(key.startswith("io.tankhor.fixture")
                                     for key in service.get("labels", {})))
                self.assertTrue(all(port["host_ip"] == "127.0.0.1" for port in service["ports"]))
                for key, value in service.get("environment", {}).items():
                    if re.search("PASSWORD|ROOT_USER", key):
                        self.assertTrue(key.endswith("_FILE"), "credentials must be secret references")
                        self.assertTrue(value.startswith("/run/secrets/"))
        postgres = config["services"]["postgres"]
        self.assertNotEqual(postgres["environment"]["POSTGRES_USER"], "tankhor_runtime")
        secrets_used = {entry["source"] for entry in postgres["secrets"]}
        self.assertTrue({"postgres_admin", "postgres_migration", "postgres_runtime", "postgres_preparation"}
                        .issubset(secrets_used))
        initializer = ROOT / "deploy/init-postgres.sh"
        self.assertTrue(initializer.is_file(), "distinct developer roles need real initialization")
        source = initializer.read_text()
        for role in ("tankhor_migration", "tankhor_runtime", "tankhor_preparation"):
            self.assertIn(role, source)
        self.assertIn("NOSUPERUSER NOCREATEDB NOCREATEROLE", source)
        self.assertIn("OWNER TO tankhor_migration", source)
        self.assertNotIn("GRANT tankhor_migration TO tankhor_runtime", source)
        self.assertNotIn("synthetic-config-test-value", COMPOSE.read_text())


if __name__ == "__main__":
    unittest.main()
