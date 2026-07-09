import unittest

from openagents_bench.gcp_batch import build_batch_job, machine_type_for_task_class


class GcpBatchTests(unittest.TestCase):
    def test_machine_type_mapping(self):
        self.assertEqual(machine_type_for_task_class("small_terminal"), "e2-standard-4")
        self.assertEqual(machine_type_for_task_class("normal_coding"), "e2-standard-8")
        self.assertEqual(machine_type_for_task_class("large_repo"), "n2-standard-16")
        self.assertEqual(machine_type_for_task_class("memory_heavy"), "n2-highmem-16")

    def test_batch_job_uses_bounded_gcs_envelope(self):
        job = build_batch_job(
            image_uri="us-central1-docker.pkg.dev/project/oa-benchmark-runners/py-bench-runner:dev",
            run_id="run_123",
            task_run_id="taskrun_456",
            task_spec_gcs="gs://specs/runs/run_123/tasks/taskrun_456.json",
            artifact_prefix="gs://artifacts/runs/run_123/tasks/taskrun_456/",
            agent="fake-agent",
            model="fake-model",
            service_account="bench-runner@project.iam.gserviceaccount.com",
            machine_type="e2-standard-8",
            max_run_duration="7200s",
            task_event_topic="benchmark-task-events-dev",
        )

        task_spec = job["taskGroups"][0]["taskSpec"]
        command = task_spec["runnables"][0]["container"]["commands"][1]
        variables = task_spec["environment"]["variables"]

        self.assertIn("--task-spec-gcs \"$TASK_SPEC_GCS\"", command)
        self.assertIn("--artifact-prefix \"$ARTIFACT_PREFIX\"", command)
        self.assertNotIn("rm -rf", command)
        self.assertEqual(variables["TASK_SPEC_GCS"], "gs://specs/runs/run_123/tasks/taskrun_456.json")
        self.assertEqual(variables["ARTIFACT_PREFIX"], "gs://artifacts/runs/run_123/tasks/taskrun_456/")
        self.assertEqual(job["allocationPolicy"]["instances"][0]["policy"]["machineType"], "e2-standard-8")
        self.assertEqual(job["allocationPolicy"]["serviceAccount"]["email"], "bench-runner@project.iam.gserviceaccount.com")
        self.assertEqual(job["logsPolicy"]["destination"], "CLOUD_LOGGING")


if __name__ == "__main__":
    unittest.main()
