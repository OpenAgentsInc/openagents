#!/usr/bin/env bash
# Schedule one deletion of this research VM; no reusable account key is issued.
set -euo pipefail
action=${1:?use create TIMESTAMP, reschedule TIMESTAMP, or cancel}
project=openagentsgemini
zone=us-central1-a
location=us-central1
vm=oa-layertree-signet-tdx
instance_id=6613632645341139193
queue=layertree-signet-kill
task=delete-$instance_id
account=oa-layertree-kill@$project.iam.gserviceaccount.com
role=layertreeResearchDelete
case $action in
  reschedule)
    # The observer calls this only after recording a completed clean soak.
    # Create the replacement first, then cancel the old bounded task.
    python3 - "${2:?provide the qualified UTC deletion timestamp}" <<'PY'
from datetime import datetime, timezone
import json
import subprocess
import sys

project = 'openagentsgemini'
zone = 'us-central1-a'
vm = 'oa-layertree-signet-tdx'
instance = '6613632645341139193'
account = f'oa-layertree-kill@{project}.iam.gserviceaccount.com'
queue = ['--project', project, '--location', 'us-central1', '--queue', 'layertree-signet-kill']
timestamp = datetime.fromisoformat(sys.argv[1].replace('Z', '+00:00'))
assert timestamp.tzinfo is not None
url = f'https://compute.googleapis.com/compute/v1/projects/{project}/zones/{zone}/instances/{vm}'
name = f'delete-{instance}-clean24'


def gcloud(*args):
    return subprocess.run(['gcloud', *args], capture_output=True, text=True, timeout=60)


def describe(task):
    result = gcloud('tasks', 'describe', task, *queue, '--format=json')
    if result.returncode:
        if 'NOT_FOUND' in result.stderr:
            return None
        raise RuntimeError(result.stderr[-500:])
    return json.loads(result.stdout)


result = gcloud('compute', 'instances', 'describe', vm, '--project', project,
                '--zone', zone, '--format=value(id)')
assert result.returncode == 0 and result.stdout.strip() == instance
task = describe(name)
if task is None:
    seconds = (timestamp - datetime.now(timezone.utc)).total_seconds()
    assert 60 < seconds <= 9 * 86400
    result = gcloud('tasks', 'create-http-task', name, *queue, '--url=' + url,
                    '--method=DELETE', '--schedule-time=' + timestamp.isoformat(),
                    '--oauth-service-account-email=' + account,
                    '--oauth-token-scope=https://www.googleapis.com/auth/cloud-platform', '--quiet')
    if result.returncode:
        raise RuntimeError(result.stderr[-500:])
    task = describe(name)
assert task is not None
assert datetime.fromisoformat(task['scheduleTime'].replace('Z', '+00:00')) == timestamp
request = task['httpRequest']
assert request['url'] == url and request['httpMethod'] == 'DELETE'
assert request['oauthToken']['serviceAccountEmail'] == account
old = f'delete-{instance}'
if describe(old) is not None:
    result = gcloud('tasks', 'delete', old, *queue, '--quiet')
    if result.returncode:
        raise RuntimeError(result.stderr[-500:])
print(json.dumps(task))
PY
    ;;
  create)
    timestamp=${2:?provide the planned UTC deletion timestamp}
    python3 - "$timestamp" <<'PY'
from datetime import datetime, timezone
import sys
when=datetime.fromisoformat(sys.argv[1].replace('Z','+00:00'))
seconds=(when-datetime.now(timezone.utc)).total_seconds()
assert 7*86400 < seconds <= 9*86400
PY
    id=$(gcloud compute instances describe "$vm" --project "$project" --zone "$zone" --format='value(id)')
    [[ $id == "$instance_id" ]] || { echo 'This is not the planned signet VM.' >&2; exit 2; }
    gcloud services enable cloudtasks.googleapis.com --project "$project" --quiet
    if ! gcloud iam service-accounts describe "$account" --project "$project" >/dev/null 2>&1; then
      gcloud iam service-accounts create oa-layertree-kill --project "$project" --display-name='Bounded signet VM deletion' --quiet
    fi
    if ! gcloud iam roles describe "$role" --project "$project" >/dev/null 2>&1; then
      gcloud iam roles create "$role" --project "$project" --title='Delete one research VM' --permissions=compute.instances.delete --stage=GA --quiet
    fi
    permissions=$(gcloud iam roles describe "$role" --project "$project" --format='value(includedPermissions)')
    [[ $permissions == compute.instances.delete ]] || { echo 'The deletion role has unexpected permissions.' >&2; exit 2; }
    # This binding belongs to this instance, rather than the project. A new
    # instance with the same name does not inherit it after deletion.
    gcloud compute instances add-iam-policy-binding "$vm" --project "$project" --zone "$zone" --member="serviceAccount:$account" --role="projects/$project/roles/$role" --quiet
    gcloud beta services identity create --project "$project" --service cloudtasks.googleapis.com --quiet
    gcloud iam service-accounts add-iam-policy-binding "$account" --project "$project" --member=serviceAccount:service-157437760789@gcp-sa-cloudtasks.iam.gserviceaccount.com --role=roles/iam.serviceAccountTokenCreator --quiet
    if ! gcloud tasks queues describe "$queue" --project "$project" --location "$location" >/dev/null 2>&1; then
      gcloud tasks queues create "$queue" --project "$project" --location "$location" --max-attempts=1 --max-concurrent-dispatches=1 --max-dispatches-per-second=1 --quiet
    fi
    gcloud tasks create-http-task "$task" --project "$project" --location "$location" --queue "$queue" \
      --url="https://compute.googleapis.com/compute/v1/projects/$project/zones/$zone/instances/$vm" \
      --method=DELETE --schedule-time="$timestamp" \
      --oauth-service-account-email="$account" --oauth-token-scope=https://www.googleapis.com/auth/cloud-platform --quiet
    gcloud tasks describe "$task" --project "$project" --location "$location" --queue "$queue" --format=json
    ;;
  cancel) gcloud tasks delete "$task" --project "$project" --location "$location" --queue "$queue" --quiet ;;
  *) echo 'Use create TIMESTAMP, reschedule TIMESTAMP, or cancel.' >&2; exit 2 ;;
esac
