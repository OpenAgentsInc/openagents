#!/usr/bin/env bash
# Render the owned configuration without reaching a cluster or loading secrets.
set -euo pipefail
space_source=${1:?Provide the pinned SPACE checkout.}
space_output=${2:?Provide an output directory under scratch.}
space_helm=${SPACE_HELM:?Provide the verified Helm 3.19.0 binary.}
space_python=${SPACE_INFRA_PYTHON:-python3}
space_config=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
[[ $(git -C "$space_source" rev-parse HEAD) == de87b436c35dac056564bdd5b2a113c9046e189b ]]
[[ $("$space_helm" version --template '{{.Version}}') == v3.19.0 ]]
"$space_python" -c 'import yaml; assert yaml.__version__ == "6.0.3"'
mkdir -p "$space_output"
space_chart=$space_source/infra/space/cluster/chart
"$space_helm" template space "$space_chart" -n space --kube-version=1.37.1 \
  -f "$space_config/values.json" >"$space_output/worker-source.yaml" 2>"$space_output/worker-render.log"
"$space_helm" template space-builder "$space_chart" -n space --kube-version=1.37.1 \
  -f "$space_chart/values-builder.yaml" -f "$space_config/values.json" \
  -f "$space_config/values-builder-guards.json" >"$space_output/builder-source.yaml" 2>"$space_output/builder-render.log"
"$space_helm" template space "$space_chart" -n space --kube-version=1.37.1 \
  -f "$space_config/values.json" --set space-api-local.migrate.enabled=true \
  --show-only charts/space-api-local/templates/migrate-job.yaml \
  >"$space_output/migrate-source.yaml" 2>"$space_output/migrate-render.log"
"$space_python" "$space_config/overlay.py" "$space_output/worker-source.yaml" "$space_output/worker.yaml" --metadata-policy
"$space_python" "$space_config/overlay.py" "$space_output/builder-source.yaml" "$space_output/builder.yaml"
"$space_python" "$space_config/overlay.py" "$space_output/migrate-source.yaml" "$space_output/migrate.yaml"
"$space_python" - "$space_output" <<'PY'
import json,sys,yaml
from pathlib import Path
root=Path(sys.argv[1]); seen=set(); workloads=0
for filename in ('worker.yaml','builder.yaml','migrate.yaml'):
    for doc in yaml.safe_load_all((root/filename).read_text()):
        key=(doc['kind'],doc['metadata'].get('namespace'),doc['metadata']['name'])
        assert key not in seen, 'A resource is owned by two renders.'
        seen.add(key)
        if doc['kind']=='HorizontalPodAutoscaler': assert doc['spec']['minReplicas']==doc['spec']['maxReplicas']==1
        if doc['kind'] not in ('Deployment','DaemonSet','Job'): continue
        workloads+=1
        if doc['kind']=='Deployment': assert doc['spec'].get('replicas',1)==1
        pod=doc['spec']['template']['spec']
        assert pod['imagePullSecrets']==[{'name':'space-image-reader'}]
        for container in pod.get('containers',[])+pod.get('initContainers',[]):
            env={e['name']:e for e in container.get('env',[])}
            if 'POSTGRES_CONNECTION_STRING' not in env: continue
            assert any(m['mountPath']=='/var/run/space-postgres-ca' and m['readOnly'] for m in container['volumeMounts'])
            reference=env['POSTGRES_CONNECTION_STRING']['valueFrom']['secretKeyRef']['name']
            if doc['kind']=='Job':
                assert reference=='space-postgres-bootstrap' and 'PGPASSWORD' in env
            else:
                assert reference=='space-postgres' and 'PGPASSWORD' not in env
            assert env['POSTGRES_POOL_MAX_CONNECTIONS']['value']=='4'
            assert 'space-dev-' not in str(container)
print(json.dumps({'resources':len(seen),'workloads':workloads,'duplicate_owners':0,'bootstrap_password_outside_migration':False}))
PY
