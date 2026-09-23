from pathlib import Path

from tbench.warm_docker import built_services, image_cache, warm_tag


def test_tags_name_the_task_the_role_and_the_content():
    assert (
        warm_tag("cargo-flight-dispatch", "environment", "b2cf375ef4adfe315f6caae0")
        == "tbench-warm/cargo-flight-dispatch:environment-b2cf375ef4adfe315f6c"
    )
    assert (
        warm_tag("Terminal Bench/X", "tests", "abc", service="api")
        == "tbench-warm/terminal-bench-x-api:tests-abc"
    )
    # The main service keeps the task's own repository name.
    assert warm_tag("t", "environment", "1", service="main") == "tbench-warm/t:environment-1"


def test_built_services_are_the_compose_builds_other_than_main(tmp_path: Path):
    compose = tmp_path / "docker-compose.yaml"
    compose.write_text(
        "services:\n"
        "  main:\n    build: {context: .}\n"
        "  api:\n    build:\n      context: ./api\n"
        "  kafka:\n    image: apache/kafka-native:4.3.1\n"
    )
    assert built_services(compose) == ["api"]
    assert built_services(tmp_path / "missing.yaml") == []


def test_warm_needs_every_built_image_kept():
    tags = {"main": "m", "api": "a"}
    assert image_cache(None, tags, False, lambda tag: True) == "warm"
    assert image_cache(None, tags, False, lambda tag: tag == "m") == "cold"
    assert image_cache(None, tags, True, lambda tag: True) == "cold"
    assert image_cache("alexgshaw/fix-git:1", tags, False, lambda tag: True) == "task-image"
