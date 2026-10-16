#!/usr/bin/env python3
"""Check GCS's S3 protocol from an owned private VM with synthetic bytes."""
import argparse
import base64
from datetime import datetime, timezone
import hashlib
import hmac
import http.client
import json
from pathlib import Path
from urllib.parse import quote
import uuid
import xml.etree.ElementTree as ET


class Store:
    def __init__(self, credential, bucket):
        self.credential = credential
        self.bucket = bucket

    def request(self, method, key="", query=None, body=b"", extra=None, statuses=(200,)):
        stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        day = stamp[:8]
        scope = f"{day}/us-central1/s3/aws4_request"
        path = "/" + quote(self.bucket + "/" + key, safe="/-_.~")
        query = "&".join(quote(k, safe="-_.~") + "=" + quote(str(v), safe="-_.~") for k, v in sorted((query or {}).items()))
        digest = hashlib.sha256(body).hexdigest()
        headers = {"host": "storage.googleapis.com", "x-amz-date": stamp, "x-amz-content-sha256": digest, **(extra or {})}
        if method == "PUT" and "x-amz-copy-source" not in headers:
            headers["content-md5"] = base64.b64encode(hashlib.md5(body).digest()).decode()
        names = ";".join(sorted(headers))
        canonical = "\n".join([method, path, query, "".join(k + ":" + headers[k].strip() + "\n" for k in sorted(headers)), names, digest])
        value = "AWS4-HMAC-SHA256\n" + stamp + "\n" + scope + "\n" + hashlib.sha256(canonical.encode()).hexdigest()
        signing = ("AWS4" + self.credential["secret_access_key"]).encode()
        for part in (day, "us-central1", "s3", "aws4_request"):
            signing = hmac.digest(signing, part.encode(), "sha256")
        signature = hmac.new(signing, value.encode(), "sha256").hexdigest()
        headers["authorization"] = f'AWS4-HMAC-SHA256 Credential={self.credential["access_key_id"]}/{scope}, SignedHeaders={names}, Signature={signature}'
        connection = http.client.HTTPSConnection("storage.googleapis.com", timeout=30)
        try:
            connection.request(method, path + ("?" + query if query else ""), body=body, headers=headers)
            response = connection.getresponse()
            data = response.read(16777217)
            if len(data) > 16777216 or response.status not in statuses:
                # Never print authorization headers or provider response bodies.
                raise RuntimeError(f"The synthetic {method} operation returned HTTP {response.status}.")
            return {k.lower(): v for k, v in response.getheaders()}, data, response.status
        finally:
            connection.close()


def probe(store, receipt):
    prefix = "qualification/11814/" + str(uuid.uuid4()) + "/"
    receipt.update({"bucket": store.bucket, "prefix": prefix})
    keys, uploads = [], []
    events = []
    try:
        tiny = prefix + "tiny"
        payload = b"OpenAgents synthetic storage qualification; no customer content."
        keys.append(tiny)
        store.request("PUT", tiny, body=payload)
        headers, _, _ = store.request("HEAD", tiny)
        assert int(headers["content-length"]) == len(payload)
        assert store.request("GET", tiny)[1] == payload
        events.append("single put/head/get")

        large = prefix + "multipart"
        keys.append(large)
        body = bytes(range(256)) * (7 * 1024 * 1024 // 256)
        _, result, _ = store.request("POST", large, {"uploads": ""})
        upload = ET.fromstring(result).findtext("{*}UploadId")
        assert upload
        uploads.append((large, upload))
        parts = []
        for number, chunk in enumerate((body[:6 * 1024 * 1024], body[6 * 1024 * 1024:]), 1):
            headers, _, _ = store.request("PUT", large, {"partNumber": number, "uploadId": upload}, body=chunk)
            part = ET.Element("Part")
            ET.SubElement(part, "PartNumber").text = str(number)
            ET.SubElement(part, "ETag").text = headers["etag"]
            parts.append(part)
        complete = ET.Element("CompleteMultipartUpload")
        complete.extend(parts)
        store.request("POST", large, {"uploadId": upload}, body=ET.tostring(complete))
        uploads.remove((large, upload))
        assert store.request("GET", large)[1] == body
        assert store.request("GET", large, extra={"range": "bytes=4096-8191"}, statuses=(206,))[1] == body[4096:8192]
        events.append("multipart upload/complete/full and ranged get")

        copied = prefix + "copy"
        keys.append(copied)
        store.request("PUT", copied, extra={"x-amz-copy-source": "/" + store.bucket + "/" + large})
        assert store.request("GET", copied)[1] == body
        events.append("server-side copy")

        abandoned = prefix + "aborted"
        _, result, _ = store.request("POST", abandoned, {"uploads": ""})
        upload = ET.fromstring(result).findtext("{*}UploadId")
        assert upload
        uploads.append((abandoned, upload))
        store.request("DELETE", abandoned, {"uploadId": upload}, statuses=(204,))
        uploads.remove((abandoned, upload))
        events.append("multipart abort")

        listed = ET.fromstring(store.request("GET", query={"list-type": 2, "prefix": prefix})[1])
        assert {row.text for row in listed.findall("{*}Contents/{*}Key")} == set(keys)
        events.append("prefix listing")
        receipt.update({"bucket": store.bucket, "prefix": prefix, "checks": events,
                        "payload_bytes": len(body), "payload_sha256": hashlib.sha256(body).hexdigest()})
    finally:
        # A timed-out initiate request may have succeeded remotely. Discover
        # only this run's random prefix before removing unfinished uploads.
        pending = ET.fromstring(store.request("GET", query={"uploads": "", "prefix": prefix})[1])
        for row in pending.findall("{*}Upload"):
            key, upload = row.findtext("{*}Key"), row.findtext("{*}UploadId")
            assert key and key.startswith(prefix) and upload
            if (key, upload) not in uploads:
                uploads.append((key, upload))
        for key, upload in uploads:
            store.request("DELETE", key, {"uploadId": upload}, statuses=(204, 404))
        for key in keys:
            store.request("DELETE", key, statuses=(204, 404))
        listed = ET.fromstring(store.request("GET", query={"list-type": 2, "prefix": prefix})[1])
        assert not listed.findall("{*}Contents")
        receipt["active_objects_after_cleanup"] = 0
        receipt["soft_delete_retention_days"] = 7


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("credential_file")
    parser.add_argument("bucket")
    parser.add_argument("--expect-network-denied", action="store_true")
    args = parser.parse_args()
    store = Store(json.loads(Path(args.credential_file).read_text()), args.bucket)
    if args.expect_network_denied:
        _, _, status = store.request("GET", "qualification/11814/negative-external", statuses=(403,))
        print(json.dumps({"external_signed_request": status}))
        return
    receipt = {"protocol": "AWS4-HMAC-SHA256 through the GCS XML API", "client": "stdlib protocol probe; native client qualification remains separate"}
    try:
        probe(store, receipt)
    finally:
        print(json.dumps(receipt, sort_keys=True))


if __name__ == "__main__":
    main()
