import argparse
import json
from typing import Dict, List, Optional


Json = Dict[str, object]


def machine_type_for_task_class(task_class: str) -> str:
    mapping = {
        "small_terminal": "e2-standard-4",
        "normal_coding": "e2-standard-8",
        "large_repo": "n2-standard-16",
        "memory_heavy": "n2-highmem-16",
    }
    if task_class not in mapping:
        raise ValueError("unknown task class: %s" % task_class)
    return mapping[task_class]


def build_batch_job(
    image_uri: str,
    run_id: str,
    task_run_id: str,
    task_spec_gcs: str,
    artifact_prefix: str,
    agent: str,
    model: str,
    service_account: str,
    machine_type: str,
    max_run_duration: str,
    task_event_topic: Optional[str] = None,
    retry_count: int = 0,
) -> Json:
    command = " ".join(
        [
            "python3 -m openagents_bench.run_task",
            "--run-id \"$RUN_ID\"",
            "--task-run-id \"$TASK_RUN_ID\"",
            "--task-spec-gcs \"$TASK_SPEC_GCS\"",
            "--artifact-prefix \"$ARTIFACT_PREFIX\"",
            "--agent \"$AGENT\"",
            "--model \"$MODEL\"",
        ]
    )
    variables = {
        "RUN_ID": run_id,
        "TASK_RUN_ID": task_run_id,
        "TASK_SPEC_GCS": task_spec_gcs,
        "ARTIFACT_PREFIX": artifact_prefix,
        "AGENT": agent,
        "MODEL": model,
    }
    if task_event_topic:
        variables["TASK_EVENT_TOPIC"] = task_event_topic

    return {
        "taskGroups": [
            {
                "taskSpec": {
                    "runnables": [
                        {
                            "container": {
                                "imageUri": image_uri,
                                "entrypoint": "/bin/bash",
                                "commands": ["-lc", command],
                            }
                        }
                    ],
                    "environment": {
                        "variables": variables,
                    },
                    "maxRunDuration": max_run_duration,
                    "maxRetryCount": retry_count,
                },
                "taskCount": 1,
                "parallelism": 1,
            }
        ],
        "allocationPolicy": {
            "serviceAccount": {
                "email": service_account,
            },
            "instances": [
                {
                    "policy": {
                        "machineType": machine_type,
                    }
                }
            ],
        },
        "logsPolicy": {
            "destination": "CLOUD_LOGGING",
        },
        "labels": {
            "openagents-component": "benchmark-cloud",
            "openagents-run": run_id.lower().replace("_", "-")[:63],
        },
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Emit a Google Cloud Batch job config for one benchmark task")
    parser.add_argument("--image-uri", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--task-run-id", required=True)
    parser.add_argument("--task-spec-gcs", required=True)
    parser.add_argument("--artifact-prefix", required=True)
    parser.add_argument("--agent", required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--service-account", required=True)
    parser.add_argument("--machine-type")
    parser.add_argument("--task-class", default="normal_coding")
    parser.add_argument("--max-run-duration", default="7200s")
    parser.add_argument("--task-event-topic")
    parser.add_argument("--retry-count", type=int, default=0)
    return parser


def main(argv: Optional[List[str]] = None) -> int:
    args = build_parser().parse_args(argv)
    machine_type = args.machine_type or machine_type_for_task_class(args.task_class)
    print(
        json.dumps(
            build_batch_job(
                image_uri=args.image_uri,
                run_id=args.run_id,
                task_run_id=args.task_run_id,
                task_spec_gcs=args.task_spec_gcs,
                artifact_prefix=args.artifact_prefix,
                agent=args.agent,
                model=args.model,
                service_account=args.service_account,
                machine_type=machine_type,
                max_run_duration=args.max_run_duration,
                task_event_topic=args.task_event_topic,
                retry_count=args.retry_count,
            ),
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
