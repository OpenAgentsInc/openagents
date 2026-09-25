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


def test_stops_get_the_short_grace_and_nothing_else_does():
    from tbench.warm_docker import with_stop_timeout

    down = ["down", "--rmi", "local", "--volumes", "--remove-orphans"]
    assert with_stop_timeout(down, 1) == ["down", "--timeout", "1", *down[1:]]
    assert with_stop_timeout(["stop", "main"], 1) == ["stop", "--timeout", "1", "main"]
    assert with_stop_timeout(["up", "--detach", "--wait"], 1) == ["up", "--detach", "--wait"]
    assert with_stop_timeout(["build"], 1) == ["build"]
    # Harbor's own commands are unchanged without a grace.
    assert with_stop_timeout(down, None) == down
    # A command that names its own timeout keeps it.
    assert with_stop_timeout(["down", "-t", "30"], 1) == ["down", "-t", "30"]
    assert with_stop_timeout(["stop", "--timeout=5"], 1) == ["stop", "--timeout=5"]


def test_only_the_kept_class_shortens_the_stop():
    from tbench.warm_docker import TimedDockerEnvironment, WarmDockerEnvironment

    assert TimedDockerEnvironment.STOP_TIMEOUT_SEC is None
    assert WarmDockerEnvironment.STOP_TIMEOUT_SEC == 1
    assert issubclass(WarmDockerEnvironment, TimedDockerEnvironment)


def test_a_tasks_images_include_its_kept_ones():
    from tbench.host import images_of_task

    references = [
        "payments-pipeline-fix__abc__env-main:latest",
        "tbench-warm/payments-pipeline-fix:environment-1",
        "tbench-warm/payments-pipeline-fix-seeder:environment-1",
        "tbench-warm/payments-pipeline-fix:tests-2",
        "tbench-warm/payments:environment-3",
        "payments__x__env-main:latest",
        "ubuntu:24.04",
    ]
    assert images_of_task("payments-pipeline-fix", references) == [
        "payments-pipeline-fix__abc__env-main:latest",
        "tbench-warm/payments-pipeline-fix-seeder:environment-1",
        "tbench-warm/payments-pipeline-fix:environment-1",
        "tbench-warm/payments-pipeline-fix:tests-2",
    ]
