"""Run unchanged default source without a private optional sibling checkout.

Cargo discovers path dependencies even when disabled. Project only the manifest,
after verifying the omitted dependency is unreachable from default features.
"""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib


def default_manifest(text):
    original = tomllib.loads(text)
    assert original['features']['default'] == [], 'default features must remain empty'
    assert original['features']['meteo-types'] == ['dep:meteo-types']
    assert original['dependencies']['meteo-types'] == {'path': '../meteo-types', 'optional': True}
    for feature, values in original['features'].items():
        if feature != 'meteo-types':
            assert not any('meteo-types' in value for value in values)
    lines = []
    for line in text.splitlines():
        if line.startswith('meteo-types = {'):
            continue
        if line.startswith('meteo-types = ['):
            line = 'meteo-types = []'
        lines.append(line)
    projected = '\n'.join(lines) + '\n'
    expected = tomllib.loads(text)
    del expected['dependencies']['meteo-types']
    expected['features']['meteo-types'] = []
    assert tomllib.loads(projected) == expected, 'unexpected manifest change'
    return projected


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--offline', action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    manifest = default_manifest((root / 'Cargo.toml').read_text())
    # Remove only our temporary project; source and data remain unchanged.
    with tempfile.TemporaryDirectory(prefix='forecast-score-default-') as directory:
        project = Path(directory)
        (project / 'Cargo.toml').write_text(manifest)
        shutil.copyfile(root / 'Cargo.lock', project / 'Cargo.lock')
        for name in ['src', 'tests', 'benches']:
            (project / name).symlink_to(root / name, target_is_directory=True)
        options = ['--manifest-path', str(project / 'Cargo.toml')]
        if args.offline:
            options.append('--offline')
        environment = dict(os.environ, CARGO_TARGET_DIR=str(root / 'target' / 'ci-default'), CARGO_BUILD_JOBS='3')
        for command in [['cargo', 'test'], ['cargo', 'bench', '--no-run', '--bench', 'reference_scoring']]:
            result = subprocess.run(command + options, check=False, env=environment)
            if result.returncode:
                return result.returncode
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
