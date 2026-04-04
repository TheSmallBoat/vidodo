from __future__ import annotations

import json
import sys
from pathlib import Path

try:
    from jsonschema import Draft202012Validator
    from referencing import Registry, Resource
except ImportError as error:  # pragma: no cover - exercised in user environments
    print(
        "missing dependency: jsonschema. install it with `python3 -m pip install jsonschema`",
        file=sys.stderr,
    )
    raise SystemExit(2) from error


REPO_ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = REPO_ROOT / "tests" / "schema"
SCHEMA_ROOT = REPO_ROOT / "schemas"


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def build_schema_registry() -> Registry:
    registry = Registry()
    for schema_path in sorted(SCHEMA_ROOT.rglob("*.json")):
        registry = registry.with_resource(
            schema_path.resolve().as_uri(),
            Resource.from_contents(load_json(schema_path)),
        )
    return registry


SCHEMA_REGISTRY = build_schema_registry()


def fixture_to_schema_path(fixture_path: Path) -> Path:
    relative = fixture_path.relative_to(FIXTURE_ROOT)
    category = relative.parts[0]
    filename = relative.name

    if ".valid." in filename:
        stem = filename.split(".valid.", 1)[0]
    elif ".invalid." in filename:
        stem = filename.split(".invalid.", 1)[0]
    else:
        raise ValueError(f"unsupported fixture naming convention: {fixture_path}")

    schema_path = SCHEMA_ROOT / category / f"{stem}.v0.json"
    if not schema_path.exists():
        raise FileNotFoundError(f"schema not found for fixture {fixture_path}: {schema_path}")
    return schema_path


def validate_fixture(fixture_path: Path) -> list[str]:
    schema_path = fixture_to_schema_path(fixture_path)
    schema = load_json(schema_path)
    instance = load_json(fixture_path)
    validator = Draft202012Validator(
        schema,
        registry=SCHEMA_REGISTRY,
        _resolver=SCHEMA_REGISTRY.resolver(schema_path.resolve().as_uri()),
    )
    errors = sorted(validator.iter_errors(instance), key=lambda item: item.json_path)
    expects_valid = ".valid." in fixture_path.name

    if expects_valid and errors:
        return [
            f"expected valid fixture to pass: {fixture_path.relative_to(REPO_ROOT)}: {error.message}"
            for error in errors
        ]

    if not expects_valid and not errors:
        return [f"expected invalid fixture to fail: {fixture_path.relative_to(REPO_ROOT)}"]

    return []


def collect_fixtures(arguments: list[str]) -> list[Path]:
    if arguments:
        return [Path(argument).resolve() for argument in arguments]
    return sorted(FIXTURE_ROOT.rglob("*.json"))


def check_stability_annotations() -> list[str]:
    """Verify that bare object fields in IR schemas carry x-stability annotations."""
    failures: list[str] = []
    ir_dir = SCHEMA_ROOT / "ir"
    if not ir_dir.is_dir():
        return failures

    for schema_path in sorted(ir_dir.glob("*.json")):
        schema = load_json(schema_path)
        _check_bare_objects(
            schema,
            schema_path.relative_to(REPO_ROOT),
            "$",
            failures,
        )
    return failures


def _check_bare_objects(
    node: object,
    file_label: Path,
    path: str,
    failures: list[str],
) -> None:
    if not isinstance(node, dict):
        return
    if node.get("type") == "object" and "properties" not in node:
        if "x-stability" not in node:
            failures.append(
                f"{file_label} {path}: bare object missing x-stability annotation"
            )
    for key, value in node.items():
        if key.startswith("$"):
            continue
        child_path = f"{path}.{key}" if path else key
        if isinstance(value, dict):
            _check_bare_objects(value, file_label, child_path, failures)
        elif isinstance(value, list):
            for index, item in enumerate(value):
                _check_bare_objects(
                    item, file_label, f"{child_path}[{index}]", failures
                )


def main(arguments: list[str]) -> int:
    fixtures = collect_fixtures(arguments)
    if not fixtures:
        print("no schema fixtures found", file=sys.stderr)
        return 1

    failures: list[str] = []
    for fixture in fixtures:
        failures.extend(validate_fixture(fixture))

    stability_failures = check_stability_annotations()
    failures.extend(stability_failures)

    if failures:
        print("schema validation failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print(f"validated {len(fixtures)} schema fixtures")
    if stability_failures == []:
        print("x-stability annotations: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))


def test_check_bare_objects_detects_missing_stability():
    """Unit test: bare object without x-stability is flagged."""
    schema_with_annotation = {
        "type": "object",
        "properties": {
            "open_field": {"type": "object", "x-stability": "experimental", "minProperties": 0}
        },
    }
    failures_ok: list[str] = []
    _check_bare_objects(schema_with_annotation, Path("test.json"), "$", failures_ok)
    assert failures_ok == [], f"expected no failures but got {failures_ok}"

    schema_without_annotation = {
        "type": "object",
        "properties": {"open_field": {"type": "object"}},
    }
    failures_bad: list[str] = []
    _check_bare_objects(schema_without_annotation, Path("test.json"), "$", failures_bad)
    assert len(failures_bad) == 1, f"expected 1 failure but got {failures_bad}"
    assert "x-stability" in failures_bad[0]