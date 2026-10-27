import sys
from pathlib import Path
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'generated/python'))
from agent_wire import encode, decode, PREFIX
from openagents.engine.v1.agent_pb2 import AgentDispatch, AgentProgress

class AgentWireTest(unittest.TestCase):
    def test_request_and_progress_preserve_identity_cursor_and_actual_tools(self):
        q=dict(v=1,op='start',id='task00001',target='peer00001',objective='Inspect',write=False,after=0)
        self.assertEqual(decode(encode(q,AgentDispatch)),q)
        p=dict(v=1,run=q['id'],state='succeeded',seq=2,exit=0,events=[dict(seq=2,kind='tool',tool=dict(id='actual-call',state='failed',exit=7,input='run',output='actual error'))])
        result=decode(encode(p),AgentProgress)
        self.assertEqual(result['events'][0]['tool']['id'],'actual-call')
        self.assertEqual(result['events'][0]['tool']['exit'],7)
        self.assertEqual(result['exit'],0)
    def test_limits_unknown_intent_and_malformed_bytes(self):
        for value in [dict(objective='x'*4001),dict(id='x\0y'),dict(after=9007199254740992)]:
            with self.assertRaises(ValueError):encode(value,AgentDispatch)
        import base64
        wire=AgentDispatch(v=1).SerializeToString()+b'\x98\x06\x01'
        with self.assertRaises(ValueError):decode(PREFIX+base64.b64encode(wire).decode())
        with self.assertRaises(Exception):decode(PREFIX+'AA==')
