"""Bounded REFERENCE projections for tests/tooling, not relay admission.
No signatures/JWS are verified and no content is decrypted. Original bytes
must be retained and checked before returning them; parsed data grants nothing.
"""
import base64
from datetime import datetime
from decimal import Decimal
import hashlib
import json
import re
import uuid
from nostr.event.v1alpha1 import event_pb2 as event
from nostr.block.oa.v1alpha1.owner_pb2 import OwnerAttestation
from nostr.block.ae.v1alpha1.memory_pb2 import DecryptedBody
from nostr.block.am.v1alpha1.metrics_pb2 import TurnMetric
from nostr.block.ao.v1alpha1.observer_pb2 import Telemetry, Control
from nostr.block.ap.v1alpha1.persona_pb2 import Persona, AgentSettings, AgentInstructions, TeamConfig
from nostr.block.ar.v1alpha1.artifact_pb2 import ArtifactRevision
from nostr.block.cl.v1alpha1.channel_pb2 import ChannelIdentity, ChannelMutation
from nostr.block.fi.v1alpha1.identity_pb2 import Assertion
from validate import validate


def obj(raw, maximum=131072):
    if not isinstance(raw, bytes) or len(raw)>maximum: raise ValueError('JSON byte limit/type')
    def pairs(items):
        result={}
        for k,v in items:
            if k in result: raise ValueError('Duplicate JSON member')
            result[k]=v
        return result
    def bad(_): raise ValueError('Non-finite JSON number')
    try: value=json.loads(raw.decode('utf-8'),object_pairs_hook=pairs,parse_float=Decimal,parse_constant=bad)
    except (UnicodeError,json.JSONDecodeError,RecursionError) as e: raise ValueError('Malformed JSON') from e
    if not isinstance(value,dict): raise ValueError('JSON object required')
    return value


def text(value, maximum=262144):
    if not isinstance(value,str): raise ValueError('String required')
    try: size=len(value.encode('utf-8'))
    except UnicodeError as e: raise ValueError('Invalid Unicode scalar') from e
    if size>maximum: raise ValueError('String byte limit')
    return value


def timestamp(value):
    value=text(value)
    if not re.fullmatch(r'[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9]+)?(Z|[+-][0-9]{2}:[0-9]{2})',value):
        raise ValueError('RFC3339 timestamp required')
    datetime.fromisoformat(value.replace('Z','+00:00'))
    return value


def integer(value, signed=False):
    if type(value) is not int or not (-(2**63) if signed else 0)<=value<(2**63 if signed else 2**64):
        raise ValueError('Integer range/type')
    return value


def key(value, size=32):
    if not isinstance(value,str) or not re.fullmatch('[0-9a-f]{'+str(size*2)+'}',value): raise ValueError('Lowercase fixed-width hex required')
    return bytes.fromhex(value)


def object_bytes(value):
    # Only derived nested views use this encoder; authoritative raw bytes stay intact.
    if not isinstance(value,dict): raise ValueError('Object required')
    # Decimal numeric spellings cannot go through a float/Struct conversion.
    def encode(v):
        if isinstance(v,Decimal):return str(v)
        if isinstance(v,dict):return '{'+','.join(json.dumps(k,ensure_ascii=False)+':'+encode(x) for k,x in v.items())+'}'
        if isinstance(v,list):return '['+','.join(encode(x) for x in v)+']'
        return json.dumps(v,ensure_ascii=False,allow_nan=False,separators=(',',':'))
    return encode(value).encode('utf-8')


def signed(raw):
    v=obj(raw,524288)
    for required in ('id','pubkey','created_at','kind','tags','content','sig'):
        if required not in v: raise ValueError('Missing event field: '+required)
    result=event.SignedEvent(original_json=raw)
    e=result.event;e.id=key(v['id']);e.pubkey=key(v['pubkey']);e.sig=key(v['sig'],64)
    e.created_at=integer(v['created_at']);e.kind=integer(v['kind'])
    if e.kind>65535:raise ValueError('Event kind range')
    e.content=text(v['content'])
    if not isinstance(v['tags'],list) or len(v['tags'])>2048:raise ValueError('Tag limit/type')
    for tag in v['tags']:
        if not isinstance(tag,list) or not 1<=len(tag)<=64:raise ValueError('Tag shape')
        e.tags.add().values.extend(text(x,131072) for x in tag)
    validate(result)
    return result


def original(message):
    projected=signed(message.original_json)
    if projected.event!=message.event:raise ValueError('Event projection differs from original')
    return message.original_json


def event_id(message):
    e=message.event
    encoded=json.dumps([0,e.pubkey.hex(),e.created_at,e.kind,[list(t.values) for t in e.tags],e.content],ensure_ascii=False,separators=(',',':')).encode('utf-8')
    return hashlib.sha256(encoded).digest()


def tag(e,name,required=False,size=2):
    tags=[list(t.values) for t in e.tags if t.values[0]==name]
    if len(tags)>1 or (required and not tags):raise ValueError('Tag cardinality: '+name)
    if tags and len(tags[0])!=size:raise ValueError('Tag shape: '+name)
    return tags[0] if tags else None


def attestation(values):
    if not isinstance(values,list) or len(values)!=4 or values[0]!='auth':raise ValueError('Owner tag shape')
    owner=key(values[1]);sig=key(values[3],64);conditions=text(values[2])
    if conditions:
        for clause in conditions.split('&'):
            if not re.fullmatch(r'(kind=|created_at[<>])(0|[1-9][0-9]*)',clause):raise ValueError('Condition syntax')
    return OwnerAttestation(owner_pubkey=owner,conditions=conditions,signature=sig)


def nullable(target, value, kind):
    if value is None:target.null=0
    elif kind=='text':target.text=text(value)
    elif kind in ('uint','int'):target.number=integer(value,signed=kind=='int')
    elif kind=='number':
        if isinstance(value,bool) or not isinstance(value,(int,Decimal)) or not Decimal(value).is_finite():raise ValueError('Finite JSON number required')
        target.number.decimal=text(str(value),128)


def memory(raw):
    v=obj(raw,65535);slug=text(v.get('slug'),255);r=DecryptedBody();r.original.utf8=raw
    if slug=='core':r.core.slug=slug;r.core.profile=text(v.get('profile'))
    elif re.fullmatch(r'mem/[a-z0-9][a-z0-9_-]{0,63}(/[a-z0-9][a-z0-9_-]{0,63})*',slug):
        if 'value' not in v:raise ValueError('Missing memory value')
        r.memory.slug=slug;nullable(r.memory.value,v['value'],'text')
    else:raise ValueError('Memory slug')
    return r


def persona(raw):
    v=obj(raw,65535);r=Persona();r.original.utf8=raw;r.display_name=text(v.get('display_name'))
    for field in ('system_prompt','acp_command','avatar_url','runtime','model','provider','respond_to'):
        if field in v:nullable(getattr(r,field),v[field],'text')
    if 'parallelism' in v:nullable(r.parallelism,v['parallelism'],'int')
    for field in ('name_pool','respond_to_allowlist'):
        if field in v:
            if not isinstance(v[field],list):raise ValueError('Persona list shape')
            getattr(r,field).extend(key(x) if field.endswith('allowlist') else text(x) for x in v[field])
    if 'session_policy' in v:r.session_policy=text(v['session_policy'])
    return r


def metrics(raw):
    v=obj(raw,65535);r=TurnMetric();r.original.utf8=raw
    r.harness=text(v.get('harness'));r.timestamp=timestamp(v.get('timestamp'))
    for field in ('model','channelId','sessionId','turnId','turnSeq'):
        if field in v:
            name=re.sub(r'[A-Z]',lambda m:'_'+m.group().lower(),field)
            nullable(getattr(r,name),v[field],'int' if field=='turnSeq' else 'text')
    for field in ('turn','cumulative'):
        if field not in v:continue
        values=v[field]
        if not isinstance(values,dict):raise ValueError('Usage object required')
        target=getattr(r,field);target.SetInParent()
        for source,name in [('inputTokens','input_tokens'),('outputTokens','output_tokens'),('totalTokens','total_tokens'),('costUsd','cost_usd')]:
            if source in values:
                if values[source] is not None:
                    n=values[source]
                    if isinstance(n,bool) or not isinstance(n,(int,Decimal)) or not Decimal(n).is_finite() or n<0:raise ValueError('Invalid usage number')
                nullable(getattr(target,name),values[source],'number' if source=='costUsd' else 'uint')
        for source,name in [('cacheReadTokens','cache_read_tokens'),('cacheWriteTokens','cache_write_tokens')]:
            if source in values:setattr(target,name,integer(values[source]))
    if 'cumulative' in v and (not r.HasField('session_id') or r.session_id.WhichOneof('value')!='text' or not r.HasField('turn_seq') or r.turn_seq.WhichOneof('value')!='number'):
        raise ValueError('Cumulative requires session and sequence')
    if 'deltaReliable' in v:
        if type(v['deltaReliable']) is not bool:raise ValueError('Reliability boolean')
        r.delta_reliable=v['deltaReliable']
    if 'pricingIdentity' in v:
        price=v['pricingIdentity']
        if not isinstance(price,dict):raise ValueError('Pricing identity must be omitted rather than null')
        r.pricing_identity.authority=text(price.get('authority'));r.pricing_identity.model=text(price.get('model'))
        if 'cacheClass' in price:r.pricing_identity.cache_class=text(price['cacheClass'])
    if 'stopReason' in v:r.stop_reason=text(v['stopReason'])
    return r


def telemetry(raw):
    v=obj(raw,65535);r=Telemetry();r.original.utf8=raw
    r.seq=integer(v.get('seq'),True);r.timestamp=timestamp(v.get('timestamp'));r.kind=text(v.get('kind'))
    r.payload.utf8=object_bytes(v.get('payload'))
    for source,name,kind in [('agentIndex','agent_index','int'),('channelId','channel_id','text'),('sessionId','session_id','text'),('turnId','turn_id','text')]:
        if source in v:nullable(getattr(r,name),v[source],kind)
    return r


def artifact(raw):
    s=signed(raw);e=s.event
    if e.kind!=45010 or len(e.tags)>256:raise ValueError('Artifact envelope')
    r=ArtifactRevision();r.original.CopyFrom(s)
    if tag(e,'ar',True)[1]!='1':raise ValueError('Artifact version')
    r.version=1
    for t,name in [('d','artifact_id'),('h','home_channel')]:
        value=tag(e,t,True)[1]
        try:valid=str(uuid.UUID(value))==value and uuid.UUID(value).int!=0
        except ValueError:valid=False
        if not valid:raise ValueError('Canonical non-nil UUID')
        setattr(r,name,value)
    r.type=tag(e,'type',True)[1]
    if len(r.type.encode())>128 or not re.fullmatch(r'[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)+',r.type):raise ValueError('Artifact type')
    r.operation=tag(e,'op',True)[1]
    if r.operation not in ('create','update','move','delete','restore'):raise ValueError('Artifact op')
    previous=tag(e,'prev');title=tag(e,'title');root=tag(e,'root')
    if (r.operation=='create') == bool(previous):raise ValueError('Artifact previous revision')
    if previous:r.previous=key(previous[1])
    if root:r.root=key(root[1])
    if r.operation=='delete':
        if title or e.content:raise ValueError('Delete shape')
    else:
        if not title or not text(title[1],512).strip():raise ValueError('Artifact title')
        r.title=title[1]
    known={'ar','d','h','type','title','op','prev','root'}
    for t in e.tags:
        if t.values[0] not in known:r.annotations.add().CopyFrom(t)
    return r


def channel(raw):
    s=signed(raw);e=s.event
    if e.kind not in (39000,39001,39002):raise ValueError('Channel state kind')
    r=ChannelIdentity();r.original.CopyFrom(s);r.channel_id=tag(e,'d',True)[1]
    types=[list(t.values) for t in e.tags if t.values[0]=='t']
    if any(len(t)!=2 for t in types):raise ValueError('Channel type/label shape')
    if types:r.channel_type=types[0][1];r.labels.extend(t[1] for t in types[1:])
    if len(set(r.labels))>8 or any(not re.fullmatch('[a-z0-9.:-]{1,64}',label) or label in ('stream','forum','dm','workflow','system') for label in r.labels):raise ValueError('Channel labels')
    creator=tag(e,'P');created=tag(e,'created_at')
    if creator:r.creator_pubkey=key(creator[1])
    if created:
        if e.kind!=39000 or not re.fullmatch('[0-9]+',created[1]):raise ValueError('Channel creation time')
        r.created_at=integer(int(created[1]))
    return r


def assertion(raw):
    if not isinstance(raw,bytes) or len(raw)>16384:raise ValueError('JWS local byte bound')
    parts=raw.split(b'.')
    if len(parts)!=3 or not all(re.fullmatch(b'[A-Za-z0-9_-]+',p) for p in parts):raise ValueError('Compact JWS framing')
    def decode(part):return base64.urlsafe_b64decode(part+b'='*((-len(part))%4))
    try:header_raw=decode(parts[0]);claims_raw=decode(parts[1])
    except ValueError as e:raise ValueError('JWS base64') from e
    header=obj(header_raw,4096);v=obj(claims_raw,12288);r=Assertion(original_compact_jws=raw)
    r.protected_header.utf8=header_raw;r.claims.original.utf8=claims_raw
    typ=header.get('typ')
    if typ not in ('nip-fi+jwt','nip-fi-agent+jwt','at+jwt'):raise ValueError('Token class')
    r.claims.issuer=text(v.get('iss'));r.claims.subject=text(v.get('sub'));r.claims.nostr_pubkey=key(v.get('nostr_pubkey'))
    aud=v.get('aud')
    if isinstance(aud,str):r.claims.audience.single=aud
    elif isinstance(aud,list) and aud:
        r.claims.audience.multiple.values.extend(text(x) for x in aud)
    else:raise ValueError('Audience shape')
    for field,name in [('iat','issued_at'),('exp','expires_at'),('nbf','not_before')]:
        if field not in v:
            if field!='nbf':raise ValueError('Missing NumericDate')
            continue
        value=v[field]
        if isinstance(value,bool) or not isinstance(value,(int,Decimal)) or not Decimal(value).is_finite():raise ValueError('Finite NumericDate')
        getattr(r.claims,name).decimal=str(value)
    if typ=='nip-fi-agent+jwt':
        r.claims.owner_pubkey=key(v.get('nostr_owner_pubkey'))
        if r.claims.owner_pubkey==r.claims.nostr_pubkey:raise ValueError('Self-owned agent')
    elif 'nostr_owner_pubkey' in v:raise ValueError('Owner claim on wrong token class')
    if typ=='at+jwt':
        r.claims.client_id=text(v.get('client_id'))
        if not r.claims.client_id:raise ValueError('Empty access-token client')
    return r


def filter_view(raw):
    v=obj(raw,65536);r=event.Filter(original_json=raw)
    for source in ('ids','authors','kinds'):
        if source in v:
            if not isinstance(v[source],list) or len(v[source])>2048:raise ValueError('Filter list')
            target=getattr(r,source);target.SetInParent()
            target.values.extend(integer(x) if source=='kinds' else key(x) for x in v[source])
    for name,values in v.items():
        if name.startswith('#'):
            if len(name)!=2 or not name[1].isascii() or not name[1].isalpha():raise ValueError('NIP-01 tag filter name')
            if not isinstance(values,list):raise ValueError('Tag filter list')
            r.tags.add(name=name[1:],values=[text(x) for x in values])
    for name in ('since','until','limit'):
        if name in v:setattr(r,name,integer(v[name]))
    if 'search' in v:r.search=text(v['search'])
    validate(r);return r


def filter_matches_nothing(message):
    return any(message.HasField(name) and not getattr(message,name).values for name in ('ids','authors','kinds')) or any(not tag.values for tag in message.tags)


def config(raw, kind):
    v=obj(raw,65535 if kind=='settings' else 131072)
    if type(v.get('version')) is not int or v['version']!=1:raise ValueError('Config version')
    if kind=='settings':
        r=AgentSettings(version=1);r.settings.utf8=object_bytes(v.get('settings'))
        if 'agentType' in v:r.agent_type=text(v['agentType'])
    elif kind=='instructions':r=AgentInstructions(version=1,text=text(v.get('text')))
    elif kind=='team':r=TeamConfig(version=1,name=text(v.get('name'),512),instructions=text(v.get('instructions')))
    else:raise ValueError('Unknown config type')
    r.original.utf8=raw
    return r


def from_common(value):
    # Explicit compatibility adapter; the published common.v1 JSON wrapper
    # and its field numbers are never replaced by this new alpha projection.
    return signed(value.json.encode('utf-8'))


def to_common(value):
    from openagents.common.v1.refs_pb2 import SignedEvent
    return SignedEvent(json=original(value).decode('utf-8'))


def project(raw,nip):
    """Project one explicitly selected Block NIP; no behavioral admission."""
    s=signed(raw);e=s.event
    from nostr.nip42.v1alpha1.auth_pb2 import Authentication
    from nostr.block.aa.v1alpha1.auth_pb2 import AgentAuthentication
    from nostr.block.ae.v1alpha1.memory_pb2 import Engram
    from nostr.block.ao.v1alpha1.observer_pb2 import ObserverFrame
    from nostr.block.am.v1alpha1.metrics_pb2 import TurnMetricEvent
    from nostr.block.ap.v1alpha1.persona_pb2 import PersonaEvent,TeamCatalog
    from nostr.block.ia.v1alpha1.archive_pb2 import ArchiveRequest,ArchiveDelta,ArchiveSnapshot
    from nostr.block.dv.v1alpha1.visibility_pb2 import VisibilitySnapshot
    from nostr.block.wp.v1alpha1.workspace_pb2 import WorkspaceProfileCommand
    if nip=='AA':
        if e.kind!=22242:raise ValueError('AUTH kind')
        auth=Authentication(relay=tag(e,'relay',True)[1],challenge=tag(e,'challenge',True)[1])
        auth.original.CopyFrom(s);r=AgentAuthentication(authentication=auth)
        credential=tag(e,'auth',size=4)
        if credential:
            r.credential.CopyFrom(attestation(credential))
            if r.credential.owner_pubkey==e.pubkey:raise ValueError('Self attestation')
    elif nip=='AE':
        if e.kind!=30174:raise ValueError('Engram kind')
        d=tag(e,'d',True)[1];key(d)
        r=Engram(blinded_identifier=d,owner_pubkey=key(tag(e,'p',True)[1]),original=s)
    elif nip=='AO':
        if e.kind!=24200:raise ValueError('Observer kind')
        r=ObserverFrame(original=s,recipient=key(tag(e,'p',True)[1]),agent=key(tag(e,'agent',True)[1]),frame=tag(e,'frame',True)[1])
        if r.frame not in ('telemetry','control'):raise ValueError('Unknown observer frame: ignore')
        if (r.frame=='telemetry' and e.pubkey!=r.agent) or (r.frame=='control' and r.recipient!=r.agent):raise ValueError('Observer direction')
        h=tag(e,'h')
        if h:r.channel=h[1]
    elif nip=='AM':
        if e.kind!=44200 or tag(e,'h') or key(tag(e,'agent',True)[1])!=e.pubkey:raise ValueError('Metric envelope')
        r=TurnMetricEvent(original=s,owner_pubkey=key(tag(e,'p',True)[1]))
    elif nip=='AP':
        d=tag(e,'d',True)[1];shared=tag(e,'shared')
        if e.kind==30175:
            if not re.fullmatch('[a-z0-9][a-z0-9_-]{0,63}',d):raise ValueError('Persona slug')
            r=PersonaEvent(original=s,slug=d,body=persona(e.content.encode('utf-8')))
        elif e.kind==30178:
            if not d or len(d)>64 or any(ord(x)<32 or x.isspace() for x in d):raise ValueError('Team identifier')
            r=TeamCatalog(original=s,team_id=d);r.body.utf8=e.content.encode();obj(r.body.utf8)
        else:raise ValueError('Persona/catalog kind')
        if shared:r.shared=shared[1]=='true'
    elif nip=='IA':
        if tag(e,'-',True,size=1) is None:raise ValueError('Protected archive event')
        if e.kind==13535:
            r=ArchiveSnapshot(original=s)
            for t in e.tags:
                if t.values[0]=='p':
                    if len(t.values)!=2:raise ValueError('Archive key tag')
                    r.archived_pubkeys.append(key(t.values[1]))
        elif e.kind in (9035,9036,8002,8003):
            target=key(tag(e,'p',True)[1])
            if e.kind in (9035,9036):
                r=ArchiveRequest(original=s,target_pubkey=target);auth=tag(e,'auth',size=4)
                if auth:r.credential.CopyFrom(attestation(auth))
            else:
                consent=tag(e,'consent',True,size=3)
                if consent[1] not in ('self','owner','admin','relay'):raise ValueError('Consent path')
                r=ArchiveDelta(original=s,target_pubkey=target,consent=consent[1],actor_or_owner=key(consent[2]));request=tag(e,'e')
                if request:r.request_event_id=key(request[1])
                elif consent[1]!='relay':raise ValueError('Missing archive request reference')
            reason=tag(e,'reason');replacement=tag(e,'replaced-by')
            if reason:r.reason=reason[1]
            if replacement and e.kind in (9035,8002):
                r.replaced_by=key(replacement[1])
                if r.replaced_by==target:raise ValueError('Replacement equals target')
        else:raise ValueError('Archive kind')
    elif nip=='AR':return artifact(raw)
    elif nip=='CL':return channel(raw)
    elif nip=='DV':
        if e.kind!=30622 or e.content or tag(e,'d',True)[1]!=tag(e,'p',True)[1]:raise ValueError('Visibility envelope')
        r=VisibilitySnapshot(original=s,owner_pubkey=key(tag(e,'p',True)[1]))
        for t in e.tags:
            if t.values[0]=='h':
                if len(t.values)!=2:raise ValueError('Hidden-channel tag')
                r.hidden_channel_ids.append(t.values[1])
    elif nip=='WP':
        if e.kind!=9033 or e.content:raise ValueError('Workspace command')
        r=WorkspaceProfileCommand(original=s);icon=tag(e,'icon')
        if icon:
            value=text(icon[1],98304)
            if value and (not value.startswith(('https://','http://','data:image/')) or any(c.isspace() or ord(c)<32 or ord(c)==127 for c in value)):raise ValueError('Workspace icon')
            r.icon=value
    else:raise ValueError('No reference event projector for this profile')
    validate(r);return r


def decode_signed(data):
    from google.protobuf.message import DecodeError
    if not isinstance(data,bytes) or len(data)>1048576:raise ValueError('Signed projection input bound')
    try:message=event.SignedEvent.FromString(data)
    except DecodeError as error:raise ValueError('Malformed signed projection') from error
    validate(message);original(message)
    return message
