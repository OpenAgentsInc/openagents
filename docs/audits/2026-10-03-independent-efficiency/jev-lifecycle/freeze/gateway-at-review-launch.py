"""One bounded Jev call through Vercel's unversioned gateway alias.

This benchmark helper does not retry, follow redirects, or request fallbacks.
Callers own the content of state/questions; artifacts can contain that private
content and must not be published without review. The API credential is never
written. The returned alias is a provider claim, not a model-version pin.

Contract: https://vercel.com/docs/ai-gateway/sdks-and-apis/typesafe
Primitives: https://docs.typesafe.ai/primitives
"""

import hashlib
import http.client
import json
import math
import os
from pathlib import Path
import re
import time
import urllib.error
import urllib.request
from decimal import Decimal, InvalidOperation
from datetime import datetime, timezone


ENDPOINT = "https://ai-gateway.vercel.sh/typesafe/v1/systemone"
MODEL = "typesafe-ai/jev"
MAX_REQUEST_BYTES = 128 * 1024
MAX_RESPONSE_BYTES = 1024 * 1024
# Accommodate rounded distributions without treating them as exact arithmetic.
PROBABILITY_TOLERANCE = 0.03
SCORE_TOLERANCE = 0.05
RESPONSE_HEADERS = (
    "x-typesafe-request-id", "x-vercel-id", "retry-after",
    "x-ai-gateway-evaluation-fallback-triggered",
    "x-ai-gateway-evaluation-fallback-final-model",
)


class ValidationError(ValueError):
    """The request or answer does not match this helper's contract."""


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise urllib.error.HTTPError(req.full_url, code, "Redirect refused", headers, fp)


def _object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValidationError("Duplicate JSON key")
        result[key] = value
    return result


def _constant(_value):
    raise ValidationError("Nonfinite JSON number")


def _float(value):
    result = float(value)
    if not math.isfinite(result):
        raise ValidationError("Nonfinite JSON number")
    return result


def loads(raw):
    """Read JSON without silently collapsing duplicate question or answer IDs."""
    return json.loads(raw, object_pairs_hook=_object, parse_constant=_constant,
                      parse_float=_float)


def _json(value):
    return json.dumps(value, ensure_ascii=False, allow_nan=False,
                      separators=(",", ":")).encode("utf-8")


def _number(value, low, high):
    if (type(value) not in (int, float) or not math.isfinite(value)
            or not low <= value <= high):
        raise ValidationError("Number outside the allowed range")
    return value


def _ids(value):
    return (isinstance(value, dict) and bool(value)
            and all(isinstance(key, str) and key and len(key) <= 256 for key in value))


def validate_questions(questions):
    """Validate the supported question schema before any HTTP request."""
    if not _ids(questions) or len(questions) > 256:
        raise ValidationError("Questions must have 1 to 256 distinct string IDs")
    for question in questions.values():
        if (not isinstance(question, dict)
                or set(question) - {"type", "instructions", "criteria"}):
            raise ValidationError("Unsupported question fields")
        kind = question.get("type")
        criteria = question.get("criteria")
        instructions = question.get("instructions")
        if (not isinstance(instructions, (str, dict, list)) or not instructions
                or (isinstance(instructions, str) and not instructions.strip())):
            raise ValidationError("Instructions must be nonempty text, an object, or an array")
        if kind == "noul":
            if criteria is not None and (not isinstance(criteria, dict)
                                         or set(criteria) != {"true", "false"}):
                raise ValidationError("Noul criteria must describe true and false")
        elif kind == "choice":
            if not _ids(criteria) or not 2 <= len(criteria) <= 255:
                raise ValidationError("Choice criteria must name 2 to 255 options")
        elif kind == "score":
            if not isinstance(criteria, list) or not 2 <= len(criteria) <= 10:
                raise ValidationError("Score criteria must contain 2 to 10 levels")
        else:
            raise ValidationError("Unsupported question type")
    _json(questions)


def validate_answers(response, questions):
    """Return typed answers or raise; this checks shape, not semantic accuracy.

    Choice/Score require the native probability distribution and confidence.
    Score legend is optional, but if present must match the requested rubric.
    The model field must be the requested gateway alias, not an inferred version.
    """
    validate_questions(questions)
    if not isinstance(response, dict) or response.get("model") != MODEL:
        raise ValidationError("Unexpected response model")
    answers = response.get("answers")
    if not isinstance(answers, dict) or set(answers) != set(questions):
        raise ValidationError("Answer IDs do not match question IDs")
    for key, question in questions.items():
        answer = answers[key]
        kind = question["type"]
        if not isinstance(answer, dict) or answer.get("type") != kind:
            raise ValidationError("Answer type does not match its question")
        if kind == "noul":
            _number(answer.get("noul"), 0, 1)
            continue
        _number(answer.get("confidence"), 0, 1)
        criteria = question["criteria"]
        expected = (set(criteria) if kind == "choice"
                    else {str(index) for index in range(len(criteria))})
        probabilities = answer.get("probabilities")
        if not isinstance(probabilities, dict) or set(probabilities) != expected:
            raise ValidationError("Probability IDs do not match the criteria")
        for probability in probabilities.values():
            _number(probability, 0, 1)
        if abs(sum(probabilities.values()) - 1) > PROBABILITY_TOLERANCE:
            raise ValidationError("Probabilities do not sum to one")
        if kind == "choice":
            if not isinstance(answer.get("choice"), str) or answer["choice"] not in expected:
                raise ValidationError("Choice is outside the requested options")
        else:
            score = _number(answer.get("score"), 0, len(criteria) - 1)
            mean = sum(int(level) * value for level, value in probabilities.items())
            if abs(score - mean) > SCORE_TOLERANCE:
                raise ValidationError("Score conflicts with its distribution")
            if "legend" in answer:
                legend = {str(index): value for index, value in enumerate(criteria)}
                if answer["legend"] != legend:
                    raise ValidationError("Score legend conflicts with the rubric")
    return answers


def _decimal(value):
    if type(value) not in (str, int, float) or len(str(value)) > 128:
        raise ValidationError("Invalid reported cost")
    try:
        number = Decimal(str(value))
        converted = float(number)
    except (InvalidOperation, OverflowError, ValueError):
        raise ValidationError("Invalid reported cost") from None
    if (not number.is_finite() or number < 0 or not math.isfinite(converted)
            or (number > 0 and converted == 0)):
        raise ValidationError("Invalid reported cost")
    return number


def accounting(response):
    """Keep raw accounting and use gateway.cost once, without price inference.

    An existing usage.cost must agree numerically. Other gateway cost fields
    are retained, not summed or treated as aliases for the billed total.
    """
    usage = response.get("usage")
    metadata = response.get("provider_metadata")
    gateway = metadata.get("gateway") if isinstance(metadata, dict) else None
    result = {"usage": usage, "gateway_metadata": gateway, "cost_usd": None,
              "cost_usd_decimal": None, "cost_status": "unknown",
              "accounting_error": None}
    if not isinstance(gateway, dict) or "cost" not in gateway:
        return result
    try:
        cost = _decimal(gateway["cost"])
        if usage is not None and not isinstance(usage, dict):
            raise ValidationError("Invalid usage object")
        if isinstance(usage, dict):
            for field in ("input_tokens", "output_tokens"):
                if field in usage and (type(usage[field]) is not int or usage[field] < 0):
                    raise ValidationError("Invalid token count")
            if "cost" in usage and _decimal(usage["cost"]) != cost:
                result["cost_status"] = "conflict"
                result["accounting_error"] = "conflicting_cost_fields"
                return result
        result.update(cost_usd=float(cost), cost_usd_decimal=str(cost),
                      cost_status="gateway_reported")
    except ValidationError:
        result["accounting_error"] = "invalid_accounting_fields"
    return result


def _write(path, raw):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "wb") as output:
        output.write(raw)
        output.flush()
        os.fsync(output.fileno())


def _has_credential(raw, credential):
    if not credential:
        return False
    decoded = raw.decode("utf-8", errors="replace")
    unescaped = re.sub(r"\\u([0-9a-fA-F]{4})", lambda match: chr(int(match[1], 16)), decoded)
    return (credential in decoded or credential in unescaped
            or json.dumps(credential, ensure_ascii=True)[1:-1] in decoded
            or any(credential.encode(encoding) in raw
                   for encoding in ("utf-16-le", "utf-16-be", "utf-32-le", "utf-32-be")))


def _utc():
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def _checkpoint(directory, receipt, started):
    receipt["status"] = receipt["outcome"]
    receipt["wall_s"] = time.monotonic() - started
    temporary = directory / ".receipt.json.tmp"
    _write(temporary, _json(receipt))
    os.replace(temporary, directory / "receipt.json")
    fd = os.open(directory, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def call(state, questions, out_dir, timeout=30):
    """Return (receipt, validated response or None), making at most one attempt.

    out_dir must not exist. It is created with mode 0700; artifacts use 0600.
    request.json holds exact transmitted bytes; response.json holds the exact
    bounded response, including HTTP failures, unless it echoes the credential.
    Receipt errors contain categories, never exception messages or auth headers.
    A received answer can have unknown/conflicting cost; callers must inspect
    cost_status separately. Setup/invalid-input errors raise before transmission.
    timeout is urllib's socket timeout, not a hard whole-call deadline.
    A pre-send launch_intent receipt reserves one possible attempt with unknown
    cost. It does not prove delivery. Abrupt termination leaves that checkpoint.
    """
    started = time.monotonic()
    started_at = _utc()
    _number(timeout, 0.001, 300)
    validate_questions(questions)
    body = _json({"model": MODEL, "state": state, "questions": questions})
    if len(body) > MAX_REQUEST_BYTES:
        raise ValidationError("Request exceeds the byte limit")
    credential = os.environ.get("AI_GATEWAY_API_KEY", "")
    if _has_credential(body, credential):
        raise ValidationError("Request contains the API credential")
    directory = Path(out_dir)
    directory.mkdir(mode=0o700, parents=False, exist_ok=False)
    _write(directory / "request.json", body)
    receipt = {
        "schema": "jev.gateway.call.v1", "endpoint": ENDPOINT,
        "requested_model": MODEL, "returned_model": None, "version_pinned": False,
        "model_identity": "unversioned_gateway_alias", "attempts": 0,
        "started_at": started_at, "finished_at": None, "launch_intent_at": None,
        "outcome": "not_sent", "answers_valid": False, "http_status": None,
        "request_sha256": hashlib.sha256(body).hexdigest(), "request_bytes": len(body),
        "response_sha256": None, "response_bytes": None,
        "response_artifact": None, "response_headers": {},
        "usage": None, "gateway_metadata": None, "cost_usd": 0.0,
        "cost_usd_decimal": "0", "cost_status": "no_call",
        "accounting_error": None, "error_type": None,
    }
    _checkpoint(directory, receipt, started)
    validated = None
    try:
        if not credential:
            receipt["error_type"] = "missing_credential"
            return receipt, None
        request = urllib.request.Request(
            ENDPOINT, data=body, method="POST",
            headers={"Authorization": "Bearer " + credential,
                     "Content-Type": "application/json", "Accept": "application/json"})
        # No environment proxy can reroute the bearer credential. TLS uses the
        # standard library's default certificate and hostname validation.
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
        receipt.update(attempts=1, outcome="launch_intent", cost_usd=None,
                       cost_usd_decimal=None, cost_status="unknown", launch_intent_at=_utc())
        _checkpoint(directory, receipt, started)
        try:
            response = opener.open(request, timeout=timeout)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            receipt["http_status"] = response.code
            for name in RESPONSE_HEADERS:
                value = response.headers.get(name)
                if value is not None and not _has_credential(value.encode(), credential):
                    receipt["response_headers"][name] = value[:1024]
            raw = response.read(MAX_RESPONSE_BYTES + 1)
        receipt["response_bytes"] = len(raw)
        if len(raw) > MAX_RESPONSE_BYTES:
            raise ValidationError("Response exceeds the byte limit")
        receipt["response_sha256"] = hashlib.sha256(raw).hexdigest()
        if _has_credential(raw, credential):
            receipt["error_type"] = "credential_echo_suppressed"
            receipt["outcome"] = "invalid_response"
            return receipt, None
        _write(directory / "response.json", raw)
        receipt["response_artifact"] = "response.json"
        data = loads(raw)
        if not isinstance(data, dict):
            raise ValidationError("Response is not an object")
        receipt.update(accounting(data))
        receipt["returned_model"] = data.get("model")
        if not 200 <= receipt["http_status"] < 300:
            receipt["outcome"] = "http_error"
            return receipt, None
        if receipt["response_headers"].get("x-ai-gateway-evaluation-fallback-triggered", "").lower() == "true":
            raise ValidationError("Gateway evaluation fallback answered")
        validate_answers(data, questions)
        receipt.update(outcome="answered", answers_valid=True)
        validated = data
    except (OSError, ValueError, TypeError, OverflowError, RecursionError,
            http.client.HTTPException) as error:
        receipt["outcome"] = ("invalid_response" if receipt["http_status"] is not None
                              else "transport_error")
        receipt["error_type"] = type(error).__name__
    finally:
        receipt["finished_at"] = _utc()
        _checkpoint(directory, receipt, started)
    return receipt, validated
