#!/usr/bin/env python3
"""Fail-closed checks for a specific oa-api Cloud Run rollout.

The caller obtains these snapshots using authenticated gcloud commands and
obtains health from the intended HTTPS load-balancer route, with redirects and
caching disabled. This offline checker does not authenticate those inputs or
run a deployment. It rejects any failed deploy command and requires snapshots
from before and after the health request; callers must abort on a failure.
"""
import argparse
import json
import re
import sys
import uuid
from pathlib import Path

REPOSITORY = 'us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/oa-api'
IMAGE = re.compile(re.escape(REPOSITORY) + r'@sha256:[0-9a-f]{64}\Z')
COMMIT = re.compile(r'[0-9a-f]{40}\Z')
SERVICE_NAMES = {'oa-api', 'oa-api-staging'}
NAMESPACE = '157437760789'
LOCATION = 'us-central1'
API_VERSION = 'serving.knative.dev/v1'
MAX_JSON = 1024 * 1024

class Invalid(ValueError):
    pass

def need(condition, message):
    if not condition:
        raise Invalid(message)

def unique_object(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, 'duplicate JSON key')
        result[key] = value
    return result

def read_json(path):
    with Path(path).open('rb') as source:
        raw = source.read(MAX_JSON + 1)
    need(len(raw) <= MAX_JSON, 'JSON input exceeds limit')
    return json.loads(raw, object_pairs_hook=unique_object,
                      parse_constant=lambda _: (_ for _ in ()).throw(Invalid('invalid JSON number')))

def number(value, label):
    need(type(value) is int and value >= 0, label + ' is invalid')
    return value

def obj(value, label):
    need(type(value) is dict, label + ' is invalid')
    return value

def name(value, label):
    need(type(value) is str and 0 < len(value) <= 128 and re.fullmatch(r'[a-z0-9][a-z0-9-]*', value) is not None, label + ' is invalid')
    return value

def ready(conditions, label):
    need(type(conditions) is list, label + ' conditions are missing')
    need(all(type(c) is dict and type(c.get('type')) is str and type(c.get('status')) is str for c in conditions), label + ' condition is malformed')
    values = [c for c in conditions if c['type'] == 'Ready']
    need(len(values) == 1 and values[0].get('status') == 'True', label + ' is not ready')

def metadata(doc, service):
    item = obj(doc, 'service')
    need(item.get('apiVersion') == API_VERSION and item.get('kind') == 'Service', 'wrong service API version or kind')
    meta = obj(item.get('metadata'), 'service metadata')
    labels = obj(meta.get('labels'), 'service labels')
    need(meta.get('namespace') == NAMESPACE and labels.get('cloud.googleapis.com/location') == LOCATION, 'wrong service namespace or region')
    need(type(meta.get('uid')) is str and 0 < len(meta['uid']) <= 128, 'missing service UID')
    status = obj(item.get('status'), 'service status')
    need(meta.get('name') == service, 'wrong service')
    generation = number(meta.get('generation'), 'service generation')
    return item, meta, status, generation

def current_service(doc, service, image):
    item, _, status, generation = metadata(doc, service)
    need(number(status.get('observedGeneration'), 'observed service generation') == generation, 'service generation is stale')
    ready(status.get('conditions'), 'service')
    revision = name(status.get('latestReadyRevisionName'), 'ready revision')
    need(status.get('latestCreatedRevisionName') == revision, 'created revision is not ready')
    spec = obj(item.get('spec'), 'service spec')
    template = obj(spec.get('template'), 'service template')
    template_spec = obj(template.get('spec'), 'service template spec')
    containers = template_spec.get('containers')
    need(type(containers) is list and len(containers) == 1 and type(containers[0]) is dict, 'expected one service container')
    need(containers[0].get('image') == image, 'service image does not match digest')
    traffic = status.get('traffic')
    need(type(traffic) is list and bool(traffic), 'traffic is missing')
    positive = []
    for target in traffic:
        target = obj(target, 'traffic entry')
        need('percent' in target and type(target.get('revisionName')) is str, 'traffic target is malformed')
        name(target['revisionName'], 'traffic revision')
        percent = number(target['percent'], 'traffic percentage')
        need(percent <= 100, 'traffic percentage is invalid')
        if percent:
            positive.append((target.get('revisionName'), percent))
    need(positive == [(revision, 100)], 'traffic does not exclusively target ready revision')
    return generation, revision

def verify(pre, before, revision_doc, after, health, *, service, image, commit, release, deploy_exit, http_status):
    need(service in SERVICE_NAMES, 'unsupported service')
    need(type(image) is str and IMAGE.fullmatch(image) is not None, 'image must be the immutable API digest')
    need(type(commit) is str and COMMIT.fullmatch(commit) is not None, 'build commit must be a full lowercase SHA-1')
    try:
        parsed = uuid.UUID(release)
    except (TypeError, AttributeError, ValueError):
        raise Invalid('release ID must be a canonical UUIDv4') from None
    need(str(parsed) == release and parsed.version == 4 and parsed.variant == uuid.RFC_4122, 'release ID must be a canonical UUIDv4')
    need(type(deploy_exit) is int and deploy_exit == 0, 'deploy command failed')
    need(type(http_status) is int and http_status == 200, 'health HTTP status is not 200')
    _, pre_meta, pre_status, pre_generation = metadata(pre, service)
    need(number(pre_status.get('observedGeneration'), 'previous observed generation') == pre_generation, 'previous service generation is stale')
    ready(pre_status.get('conditions'), 'previous service')
    previous = name(pre_status.get('latestReadyRevisionName'), 'previous ready revision')
    need(pre_status.get('latestCreatedRevisionName') == previous, 'previous created revision is not ready')
    generation, revision = current_service(before, service, image)
    need(generation > pre_generation and revision != previous, 'deployment did not produce a new revision')
    need(current_service(after, service, image) == (generation, revision), 'service changed during health check')
    need(before['metadata']['uid'] == pre_meta['uid'] == after['metadata']['uid'], 'service UID changed during deployment')
    r = obj(revision_doc, 'revision')
    need(r.get('apiVersion') == API_VERSION and r.get('kind') == 'Revision', 'wrong revision API version or kind')
    meta = obj(r.get('metadata'), 'revision metadata')
    status = obj(r.get('status'), 'revision status')
    need(meta.get('name') == revision, 'wrong revision')
    labels = obj(meta.get('labels'), 'revision labels')
    need(labels.get('serving.knative.dev/service') == service, 'revision belongs to another service')
    need(meta.get('namespace') == NAMESPACE and labels.get('cloud.googleapis.com/location') == LOCATION, 'wrong revision namespace or region')
    need(number(status.get('observedGeneration'), 'observed revision generation') == number(meta.get('generation'), 'revision generation'), 'revision generation is stale')
    ready(status.get('conditions'), 'revision')
    need(status.get('imageDigest') == image, 'resolved revision image does not match digest')
    spec = obj(r.get('spec'), 'revision spec')
    containers = spec.get('containers')
    need(type(containers) is list and len(containers) == 1 and type(containers[0]) is dict, 'expected one revision container')
    need(containers[0].get('image') == image, 'revision spec image does not match digest')
    h = obj(health, 'health')
    need(h.get('ok') is True, 'health probe failed')
    need(h.get('build') == commit and h.get('release') == release, 'health build or release does not match')
    need(h.get('service') == service and h.get('revision') == revision, 'health service or revision does not match')
    return revision

def build_image(build, tag):
    need(type(tag) is str and re.fullmatch(re.escape(REPOSITORY) + r':[a-z0-9][a-z0-9-]{0,127}', tag) is not None, 'invalid build tag')
    build = obj(build, 'build')
    need(build.get('status') == 'SUCCESS', 'build did not succeed')
    results = obj(build.get('results'), 'build results')
    images = results.get('images')
    need(type(images) is list and len(images) == 1, 'build must produce exactly one image')
    image = obj(images[0], 'built image')
    need(image.get('name') == tag, 'built tag does not match request')
    digest = image.get('digest')
    need(type(digest) is str and re.fullmatch(r'sha256:[0-9a-f]{64}', digest) is not None, 'build digest is invalid')
    return REPOSITORY + '@' + digest

def main():
    if len(sys.argv) == 5 and sys.argv[1] == 'ready-revision':
        try:
            service, image = sys.argv[3], sys.argv[4]
            need(service in SERVICE_NAMES, 'unsupported service')
            need(IMAGE.fullmatch(image) is not None, 'image must be the immutable API digest')
            print(current_service(read_json(sys.argv[2]), service, image)[1])
        except (Invalid, OSError, ValueError, UnicodeError, RecursionError) as error:
            print('service verification failed:', error, file=sys.stderr)
            return 1
        return 0
    if len(sys.argv) == 4 and sys.argv[1] == 'build-image':
        try:
            print(build_image(read_json(sys.argv[2]), sys.argv[3]))
        except (Invalid, OSError, ValueError, UnicodeError, RecursionError) as error:
            print('build verification failed:', error, file=sys.stderr)
            return 1
        return 0
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ('pre', 'before', 'revision', 'after', 'health', 'service', 'image', 'commit', 'release'):
        parser.add_argument('--' + option, required=True)
    parser.add_argument('--deploy-exit', type=int, required=True)
    parser.add_argument('--http-status', type=int, required=True)
    args = parser.parse_args()
    if args.deploy_exit != 0:
        print('deployment verification failed: deploy command failed', file=sys.stderr)
        return 1
    if args.http_status != 200:
        print('deployment verification failed: health HTTP status is not 200', file=sys.stderr)
        return 1
    try:
        revision = verify(*(read_json(getattr(args, key)) for key in ('pre', 'before', 'revision', 'after', 'health')),
                          service=args.service, image=args.image, commit=args.commit, release=args.release,
                          deploy_exit=args.deploy_exit, http_status=args.http_status)
    except (Invalid, OSError, ValueError, UnicodeError, RecursionError) as error:
        print('deployment verification failed:', error, file=sys.stderr)
        return 1
    print('deployment verified:', revision)
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
