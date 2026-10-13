#!/usr/bin/env bash
# Push scripts + a job file and run it detached on worker 0. usage: startfile.sh NAME ZONE JOBFILE
NAME=$1 ZONE=$2 JOB=$3; HERE=$(cd "$(dirname "$0")" && pwd)
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-/Users/christopherdavid/work/.secrets/gcloud-sa-config}
"$HERE/vm.sh" push "$NAME" "$ZONE" >/dev/null 2>&1
b=$(basename "$JOB")
gcloud compute tpus tpu-vm scp "$JOB" "$NAME":~/"$b" --zone "$ZONE" --project openagentsgemini --quiet >/dev/null 2>&1
"$HERE/vm.sh" run "$NAME" "$ZONE" "(nohup bash ~/vmsync.sh $NAME >/dev/null 2>&1 &); nohup bash ~/$b > ~/job-$b-\$(date +%s).log 2>&1 < /dev/null & echo started $b"
