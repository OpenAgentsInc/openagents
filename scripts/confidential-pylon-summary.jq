# Summarize an already verified oracle report. No cryptographic or release policy.
def hex64: if type=="string" then test("^[0-9a-f]{64}$") else false end;
def sha: if type=="string" then test("^sha256:[0-9a-f]{64}$") else false end;
def number_finite: type=="number" and isfinite;
def one($name): [.steps[] | select(.step==$name)] | if length==1 then .[0] else error("missing or repeated protocol step") end;
if type!="object" or has("refused") or (.steps|type)!="array" then error("invalid oracle report") else . end
| one("fetch") as $fetch | one("chain") as $chain | one("measure") as $measure
| one("bind") as $bind | one("encrypt") as $encrypt | one("relay") as $relay
| one("answer").response as $response | one("receipt").checked as $receipt
| $response.attested as $attested | $response.service as $service | $response.psionic as $psionic
| $response.answers.answer as $answer
| if
    ($bind.bound.key|hex64) and ($fetch.release|hex64)
    and ($fetch.endpoint|type)=="string"
    and ($fetch.endpoint|startswith("30203:"+$bind.bound.key+":"))
    and ($chain.hwmodel|type)=="string" and ($chain.hwmodel|length)>0 and ($measure.measured.reported|sha)
    and ($service.door|type)=="string" and ($service.door|length)>0
    and ($psionic.backend|type)=="string" and ($psionic.execution_mode|type)=="string"
    and $measure.measured.reported==$measure.measured.expected
    and $bind.bound.level=="tee-cloud" and $receipt.level==$bind.bound.level and $attested.level==$receipt.level
    and $service.provider==$bind.bound.key and $attested.endpoint==$fetch.endpoint and $attested.release==$fetch.release
    and $attested.measurement==$measure.measured.reported and $receipt.measurement==$attested.measurement
    and ($receipt.model|type)=="string" and ($receipt.model|length)>0
    and $response.model==$receipt.model and $attested.model==$receipt.model
    and ($receipt.model_digest|sha) and $receipt.model_digest==$attested.model_digest
    and $psionic.artifact_digest==$receipt.model_digest and $service.identity==($receipt.model+"@"+$receipt.model_digest)
    and ($receipt.request_ciphertext_digest|hex64) and $receipt.request_ciphertext_digest==$attested.request_ciphertext_digest
    and ($receipt.receipt_digest|sha) and ($encrypt.request|hex64)
    and ($relay.round_ms|number_finite) and $relay.round_ms>=0
    and ($psionic.latency_ms|number_finite) and $psionic.latency_ms>0
    and $answer.type=="noul" and ($answer.noul|number_finite) and $answer.noul>=0 and $answer.noul<=1
  then {
    ok:true,checked_at:$checked_at,provider:$bind.bound.key,endpoint:$fetch.endpoint,
    pylon:$service.door,model:$receipt.model,identity:$service.identity,model_digest:$receipt.model_digest,
    level:$receipt.level,hwmodel:$chain.hwmodel,image:$receipt.measurement,release:$fetch.release,
    backend:$psionic.backend,execution_mode:$psionic.execution_mode,support:$measure.measured.support,gpu:$measure.measured.gpu,
    receipt:$receipt.receipt_digest,request:$encrypt.request,request_ciphertext_digest:$receipt.request_ciphertext_digest,
    round_ms:$relay.round_ms,inference_ms:$psionic.latency_ms,score:$answer.noul,proof:$proof
  } else error("incomplete or inconsistent oracle report") end
