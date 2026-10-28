"""Source examples and counterexamples against fresh generated Nostr types."""
import base64
import copy
from decimal import Decimal
import hashlib
import json
import re
from pathlib import Path
import tempfile
import unittest
from google.protobuf import descriptor_pb2
from nostr.event.v1alpha1 import event_pb2 as E
from nostr.block.aa.v1alpha1.auth_pb2 import AgentAuthentication
from nostr.block.ae.v1alpha1.memory_pb2 import Engram
from nostr.block.ao.v1alpha1.observer_pb2 import ObserverFrame
from nostr.block.am.v1alpha1.metrics_pb2 import TurnMetricEvent
from nostr.block.ap.v1alpha1.persona_pb2 import PersonaEvent, TeamCatalog
from nostr.block.fa.v1alpha1.adapter_pb2 import Session, AssertionRequest
from nostr.block.ia.v1alpha1.archive_pb2 import ArchiveSnapshot
from nostr.block.cw.v1alpha1.window_pb2 import Bounds, Cursor
from nostr.block.dv.v1alpha1.visibility_pb2 import VisibilitySnapshot
from nostr.block.wp.v1alpha1.workspace_pb2 import WorkspaceProfileCommand
from check_nostr_sources import check as check_sources
from nostr_projection import signed, original, event_id, obj, attestation, memory, metrics, persona, telemetry, artifact, channel, assertion, filter_view, filter_matches_nothing, config, from_common, to_common, project, decode_signed
from protocol import ROOT, CONFIG, check_profile
from validate import validate

FIXTURES=ROOT/'nostr/fixtures'
KEY='11'*32;OTHER='22'*32

def raw_event(kind=1,tags=None,content=''):
    v=dict(id='00'*32,pubkey=KEY,created_at=0,kind=kind,tags=tags or [],content=content,sig='00'*64)
    return json.dumps(v,ensure_ascii=False).encode()

def encode(value):return json.dumps(value,separators=(',',':')).encode()

def jwt(claims,typ='nip-fi+jwt'):
    b64=lambda v:base64.urlsafe_b64encode(encode(v)).rstrip(b'=')
    return b'.'.join([b64({'alg':'ES256','typ':typ}),b64(claims),b64('unverified-fixture')])

class NostrTests(unittest.TestCase):
    def test_real_source_event_id_and_protobuf_fixture(self):
        raw=(FIXTURES/'owner-attested.event.json').read_bytes();value=signed(raw)
        examples=re.findall(r'```(?:json|jsonc)\n(.*?)```',(ROOT.parent/'nips/block/NIP-OA.md').read_text(),re.S)
        self.assertIn(raw,[(item.rstrip()+'\n').encode() for item in examples])
        self.assertEqual(from_common(to_common(value)),value)
        self.assertEqual(value.event.id,event_id(value))
        self.assertEqual(value.event.SerializeToString(deterministic=True),(FIXTURES/'owner-attested.event.pb').read_bytes())
        parsed=decode_signed(value.SerializeToString()+b'\x98\x06\x01')
        self.assertEqual(original(parsed),raw)
        self.assertIn(b'\x98\x06\x01',parsed.SerializeToString())
        credential=attestation(list(value.event.tags[0].values))
        self.assertEqual(credential.conditions,'kind=1&created_at<1713957000')
        value.event.content+='changed'
        with self.assertRaises(ValueError):original(value)

    def test_event_order_unicode_unknown_json_and_ciphertext(self):
        raw=raw_event(tags=[['x','é',''],['x','two'],['z']],content='\u0000 / \\ " ⛵ ciphertext==')
        data=json.loads(raw);data['future']={'untouched':9007199254740993};raw=json.dumps(data,indent=3,ensure_ascii=False).encode()
        p=signed(raw);self.assertEqual(original(E.SignedEvent.FromString(p.SerializeToString())),raw)
        self.assertEqual([list(t.values) for t in p.event.tags],[['x','é',''],['x','two'],['z']])
        self.assertEqual(p.event.created_at,0);self.assertTrue(p.event.HasField('created_at'))
        p.event.ClearField('created_at')
        with self.assertRaises(ValueError):validate(p)
        # Structural parsing intentionally accepts the unverified all-zero signature.
        self.assertEqual(signed(raw).event.sig,b'\0'*64)

    def test_bad_events_and_local_input_limits(self):
        base=json.loads(raw_event())
        cases=[]
        for name,value in [('id','0'),('pubkey',KEY.upper().replace('1','A')),('kind',65536),('kind',True),('created_at',-1),('tags',[[]]),('tags',[['x',None]]),('content',None)]:
            v=dict(base);v[name]=value;cases.append(encode(v))
        missing=dict(base);del missing['kind'];cases.append(encode(missing))
        cases += [b'{"id":1,"id":2}',b'\xff',b'{}'*262145]
        for raw in cases:
            with self.subTest(size=len(raw)),self.assertRaises(ValueError):signed(raw)
        with self.assertRaises(ValueError):decode_signed(b'\x00'*1048577)
        with self.assertRaises(ValueError):decode_signed(b'\x0a\xff')
        v=signed(raw_event());v.event.id=b'bad'
        with self.assertRaises(ValueError):validate(v)

    def test_empty_filter_is_distinct_from_missing(self):
        absent=filter_view(b'{}');empty=filter_view(b'{"authors":[],"kinds":[],"limit":0}')
        self.assertFalse(absent.HasField('authors'));self.assertTrue(empty.HasField('authors'))
        self.assertFalse(filter_matches_nothing(absent));self.assertTrue(filter_matches_nothing(empty))
        self.assertTrue(empty.HasField('limit'));self.assertEqual(empty.limit,0)
        self.assertTrue(filter_matches_nothing(filter_view(b'{"#p":[]}')))
        with self.assertRaises(ValueError):filter_view(b'{"#p":null}')
        with self.assertRaises(ValueError):filter_view(b'{"#multi":[]}')

    def test_owner_conditions_are_not_normalized(self):
        a=attestation(['auth',KEY,'created_at>0&kind=1&kind=1','00'*64])
        self.assertEqual(a.conditions,'created_at>0&kind=1&kind=1')
        self.assertNotEqual(hashlib.sha256(a.conditions.encode()).digest(),hashlib.sha256(b'kind=1&created_at>0').digest())
        for conditions in ['kind=01','kind=-1','kind=1&','created_at<1>',' kind=1']:
            with self.assertRaises(ValueError):attestation(['auth',KEY,conditions,'00'*64])

    def test_memory_null_and_unknown_fields(self):
        tombstone=memory(b'{"slug":"mem/a","value":null,"future":{"x":1}}')
        empty=memory(b'{"slug":"mem/a","value":""}')
        self.assertEqual(tombstone.memory.value.WhichOneof('value'),'null')
        self.assertEqual(empty.memory.value.WhichOneof('value'),'text')
        self.assertIn(b'"future"',tombstone.original.utf8)
        self.assertEqual(memory(b'{"slug":"core","profile":""}').WhichOneof('body'),'core')
        for raw in [b'{"slug":"mem/a"}',b'{"slug":"core","profile":null}',b'{"slug":"mem/a","value":false}',b'{"slug":"mem/a","value":"a","value":"b"}',b'{}'*32768]:
            with self.assertRaises(ValueError):memory(raw)

    def test_metrics_exact_counts_nulls_and_pricing(self):
        raw=b'{"harness":"codex","timestamp":"2026-10-27T00:00:00Z","sessionId":"s","turnSeq":0,"turn":{"inputTokens":9007199254740993,"outputTokens":null,"totalTokens":0,"costUsd":0.000000000000000123,"cacheReadTokens":0},"cumulative":{},"deltaReliable":false,"future":true}'
        m=metrics(raw)
        self.assertEqual(m.turn.input_tokens.number,9007199254740993)
        self.assertEqual(m.turn.output_tokens.WhichOneof('value'),'null')
        self.assertEqual(m.turn.total_tokens.number,0);self.assertTrue(m.turn.HasField('cache_read_tokens'))
        self.assertFalse(m.turn.HasField('cache_write_tokens'));self.assertFalse(m.HasField('pricing_identity'))
        self.assertEqual(Decimal(m.turn.cost_usd.number.decimal),Decimal('0.000000000000000123'))
        self.assertTrue(m.HasField('delta_reliable'));self.assertFalse(m.delta_reliable)
        self.assertEqual(m.original.utf8,raw)
        for patch in [dict(cumulative={}),dict(pricingIdentity=None),dict(turn={'cacheReadTokens':None}),dict(turn={'inputTokens':-1}),dict(turn={'costUsd':-0.1})]:
            v={'harness':'codex','timestamp':'2026-10-27T00:00:00Z'};v.update(patch)
            with self.assertRaises(ValueError):metrics(encode(v))

    def test_persona_and_config_states_preserve_presence(self):
        p=persona(b'{"display_name":"Agent","system_prompt":null,"acp_command":"buzz-acp","parallelism":0,"future":{"model":"new"}}')
        self.assertEqual(p.system_prompt.WhichOneof('value'),'null');self.assertFalse(p.HasField('model'))
        self.assertEqual(p.parallelism.number,0);self.assertIn(b'"future"',p.original.utf8)
        self.assertFalse(config(b'{"version":1,"settings":{"new":123}}','settings').HasField('agent_type'))
        self.assertEqual(config(b'{"version":1,"agentType":"codex","settings":{}}','settings').agent_type,'codex')
        self.assertTrue(config(b'{"version":1,"text":""}','instructions').HasField('text'))
        with self.assertRaises(ValueError):config(b'{"version":2,"text":""}','instructions')
        with self.assertRaises(ValueError):telemetry(b'{"seq":0,"timestamp":"bad","kind":"turn_started","payload":{}}')

    def test_artifact_uuid_previous_and_unfamiliar_tags(self):
        tags=[['ar','1'],['d','04737c81-e5e8-4412-bb47-f446813cfeba'],['h','9b353519-f4fe-4757-aef4-bec6cc0ae54c'],['type','buzz.task'],['title','Fix'],['op','create'],['future','keep','all']]
        a=artifact(raw_event(45010,tags,'opaque text, not necessarily JSON'))
        self.assertEqual(list(a.annotations[0].values),['future','keep','all'])
        self.assertFalse(a.HasField('previous'))
        wrong=copy.deepcopy(tags);wrong[5]=['op','update']
        with self.assertRaises(ValueError):artifact(raw_event(45010,wrong))
        wrong.append(['prev',KEY]);self.assertEqual(artifact(raw_event(45010,wrong)).previous,bytes.fromhex(KEY))
        wrong.append(['prev',KEY])
        with self.assertRaises(ValueError):artifact(raw_event(45010,wrong))

    def test_channel_creator_labels_and_missing_is_unknown(self):
        c=channel(raw_event(39000,[['d','channel'],['t','system'],['t','agent-config'],['P',OTHER],['created_at','0']]))
        self.assertEqual(c.channel_type,'system');self.assertEqual(list(c.labels),['agent-config'])
        self.assertEqual(c.creator_pubkey,bytes.fromhex(OTHER));self.assertTrue(c.HasField('created_at'))
        old=channel(raw_event(39000,[['d','channel']]))
        self.assertFalse(old.HasField('channel_type'));self.assertFalse(old.HasField('creator_pubkey'))
        with self.assertRaises(ValueError):channel(raw_event(39000,[['d','channel'],['t','system'],['t','system']]))

    def test_fi_agent_owner_and_token_class_boundaries(self):
        base={'iss':'https://issuer.example','sub':'opaque-subject','aud':['https://relay.example'],'nostr_pubkey':KEY,'iat':0,'exp':300}
        agent={**base,'nostr_owner_pubkey':OTHER}
        raw=jwt(agent,'nip-fi-agent+jwt');p=assertion(raw)
        self.assertEqual(p.original_compact_jws,raw);self.assertEqual(p.claims.owner_pubkey,bytes.fromhex(OTHER))
        self.assertEqual(p.claims.audience.WhichOneof('form'),'multiple');self.assertEqual(p.claims.issued_at.decimal,'0')
        for claims,typ in [(base,'nip-fi-agent+jwt'),({**agent,'nostr_owner_pubkey':KEY},'nip-fi-agent+jwt'),(agent,'nip-fi+jwt'),(agent,'at+jwt'),(base,'JWT'),(base,'at+jwt')]:
            with self.assertRaises(ValueError):assertion(jwt(claims,typ))
        self.assertEqual(assertion(jwt({**base,'client_id':'app'},'at+jwt')).claims.client_id,'app')

    def test_encrypted_agent_envelopes_do_not_decrypt_or_promote_claims(self):
        cipher='preserved-NIP44-ciphertext=='
        cases=[('AE',30174,[['d',KEY],['p',OTHER]]),('AO',24200,[['p',OTHER],['agent',KEY],['frame','telemetry']]),('AM',44200,[['p',OTHER],['agent',KEY]])]
        for nip,kind,tags in cases:
            value=project(raw_event(kind,tags,cipher),nip)
            self.assertEqual(value.original.event.content,cipher)
            self.assertEqual(original(value.original),value.original.original_json)
            self.assertNotIn('plaintext',value.DESCRIPTOR.fields_by_name)
        with self.assertRaises(ValueError):project(raw_event(44200,[['p',OTHER],['agent',KEY],['h','leak']],cipher),'AM')
        with self.assertRaises(ValueError):project(raw_event(24200,[['p',OTHER],['agent',OTHER],['frame','telemetry']],cipher),'AO')

    def test_auth_archive_visibility_and_workspace_examples(self):
        aa=project(raw_event(22242,[['relay','wss://relay.example'],['challenge','nonce'],['auth',OTHER,'kind=1','00'*64]]),'AA')
        self.assertEqual(aa.credential.conditions,'kind=1') # Must not be checked against kind 22242 at AA login.
        self.assertEqual(aa.authentication.challenge,'nonce')
        archive=project(raw_event(8002,[['-'],['p',OTHER],['consent','relay',KEY]]),'IA')
        self.assertEqual(archive.consent,'relay')
        with self.assertRaises(ValueError):project(raw_event(8002,[['-'],['p',OTHER],['consent','admin',KEY]]),'IA')
        snapshot=project(raw_event(30622,[['d',KEY],['p',KEY],['h','channel']]),'DV')
        self.assertEqual(list(snapshot.hidden_channel_ids),['channel'])
        with self.assertRaises(ValueError):project(raw_event(30622,[['d',KEY],['p',OTHER]]),'DV')
        self.assertFalse(project(raw_event(9033),'WP').HasField('icon'))
        self.assertTrue(project(raw_event(9033,[['icon','']]),'WP').HasField('icon'))
        with self.assertRaises(ValueError):project(raw_event(9033,[['icon','javascript:bad']]),'WP')

    def test_cursor_exhaustion_session_and_clear_presence(self):
        cursor=Cursor(created_at=0,id=bytes.fromhex(KEY));validate(cursor)
        self.assertTrue(cursor.HasField('created_at'))
        with self.assertRaises(ValueError):validate(Cursor(id=bytes.fromhex(KEY)))
        with self.assertRaises(ValueError):validate(Cursor(created_at=0,id=b'bad'))
        bound=Bounds(original=signed(raw_event(39006)),channel_id='channel',has_more=False,exhausted=0)
        validate(bound)
        decoded=Bounds.FromString(bound.SerializeToString())
        self.assertEqual(decoded.WhichOneof('next'),'exhausted');self.assertTrue(decoded.HasField('has_more'))
        session=Session(expires_at='2026-10-27T00:00:00Z',profile_null=0)
        session.original.utf8=b'{"expires_at":"2026-10-27T00:00:00Z","profile_projection":null}'
        validate(session);self.assertEqual(Session.FromString(session.SerializeToString()).WhichOneof('profile'),'profile_null')
        self.assertFalse(Session().WhichOneof('profile'))
        clear=WorkspaceProfileCommand(original=signed(raw_event(9033)),icon='')
        validate(clear);self.assertTrue(clear.HasField('icon'))
        self.assertNotIn('authorized',AgentAuthentication.DESCRIPTOR.fields_by_name)

    def test_source_pin_drift_is_blocked(self):
        check_sources(ROOT.parent)
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);(root/'proto/nostr').mkdir(parents=True);(root/'nips/official').mkdir(parents=True)
            (root/'proto/nostr/test.proto').write_text('syntax="proto3";')
            (root/'nips/official/01.md').write_bytes(b'changed')
            (root/'nips/manifest.json').write_text(json.dumps({'sources':[{'name':'official','commit':'a'*40}]}))
            (root/'proto/nostr/MANIFEST.json').write_text(json.dumps({'schema':1,'packages':[{'package':'nostr','proto':'nostr/test.proto','status':'alpha','sources':[{'path':'nips/official/01.md','commit':'a'*40,'sha256':'0'*64}]}]}))
            with self.assertRaises(ValueError):check_sources(root)

    def test_profiles_exclude_unrelated_protocols(self):
        roots=CONFIG['profiles']['block-agent'];d=descriptor_pb2.FileDescriptorSet()
        for root in roots:d.file.add(name=root,package='nostr.block.ap.v1alpha1')
        d.file.add(name='unexpected.proto',package='nostr.block.fi.v1alpha1')
        with self.assertRaises(ValueError):check_profile('block-agent',d)
