import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('default_features', ROOT / 'scripts/test_default_features.py')
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class DefaultManifestTests(unittest.TestCase):
    def setUp(self):
        self.manifest = (ROOT / 'Cargo.toml').read_text()

    def test_default_projection_preserves_other_manifest_fields(self):
        original = MODULE.tomllib.loads(self.manifest)
        projected = MODULE.tomllib.loads(MODULE.default_manifest(self.manifest))
        del original['dependencies']['meteo-types']
        original['features']['meteo-types'] = []
        self.assertEqual(projected, original)

    def test_enabled_default_cannot_omit_dependency(self):
        with self.assertRaises(AssertionError):
            MODULE.default_manifest(self.manifest.replace('default = []', 'default = ["meteo-types"]'))

    def test_transitive_feature_cannot_omit_dependency(self):
        with self.assertRaises(AssertionError):
            MODULE.default_manifest(self.manifest.replace('ts = ["dep:ts-rs"]', 'ts = ["dep:ts-rs", "meteo-types"]'))
