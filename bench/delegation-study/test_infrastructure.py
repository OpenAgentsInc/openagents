import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
import subprocess


def module(name):
    spec=importlib.util.spec_from_file_location(name,Path(__file__).with_name(name+'.py'))
    value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value)
    return value

broker=module('broker')
runner=module('run_remote')


class Infrastructure(unittest.TestCase):
    def test_only_frozen_inference_model_and_paths(self):
        body=json.dumps({'model':'frozen-model','messages':[]}).encode()
        for path in ['/v1/messages','/v1/messages?beta=true','/v1/messages/count_tokens']:
            self.assertEqual(broker.admitted('POST',path,body,{'frozen-model'}),(path,'frozen-model'))
        for method,path in [('GET','/v1/messages'),('POST','https://example.com/v1/messages'),('POST','//example.com/v1/messages'),('POST','/api/oauth/profile'),('POST','/v1/messages?redirect=https://example.com'),('POST','/v1/messages#secret')]:
            with self.assertRaises(ValueError):broker.admitted(method,path,body,{'frozen-model'})
        with self.assertRaises(ValueError):broker.admitted('POST','/v1/messages',body,{'different-model'})

    def test_provider_cannot_fetch_future_source_through_server_tools(self):
        for extra in ({'tools':[{'type':'web_search_20250305','name':'web_search'}]},{'mcp_servers':[{'url':'https://example.com'}]},{'container':'remote-container'},{'speed':'fast'},{'service_tier':'priority'},{'inference_geo':'us'}):
            body=json.dumps(dict(model='frozen-model',messages=[],**extra)).encode()
            with self.assertRaises(ValueError):broker.admitted('POST','/v1/messages',body,{'frozen-model'})
        body=json.dumps({'model':'frozen-model','tools':[{'name':'Bash','input_schema':{}},{'type':'tool_search_tool_regex_20251119','name':'tool_search'}]}).encode()
        self.assertEqual(broker.admitted('POST','/v1/messages',body,{'frozen-model'})[1],'frozen-model')

    def test_archive_refuses_traversal_and_links(self):
        for name,link in [('../hidden',None),('/absolute',None),('safe','../../private')]:
            with tempfile.TemporaryDirectory() as tmp:
                root=Path(tmp);archive=root/'source.tar'
                with tarfile.open(archive,'w') as out:
                    info=tarfile.TarInfo(name)
                    if link:info.type=tarfile.SYMTYPE;info.linkname=link
                    else:info.size=1
                    out.addfile(info,None if link else io.BytesIO(b'x'))
                with self.assertRaises(ValueError):runner.export(archive,root/'repo')

    def test_snapshot_keeps_mode_only_and_symlink_changes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);path=root/'file';path.write_text('source');path.chmod(0o600)
            before=runner.snapshot(root);path.chmod(0o700);after=runner.snapshot(root)
            self.assertEqual(before['file']['sha256'],after['file']['sha256'])
            self.assertNotEqual(before['file'],after['file'])
            (root/'link').symlink_to('file')
            self.assertEqual(runner.snapshot(root)['link']['target'],'file')

    def test_snapshot_disables_automatic_maintenance_without_changing_commit(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);workspace=root/'workspace';home=root/'home'
            workspace.mkdir();home.mkdir()
            source=workspace/'fixture.txt';source.write_bytes(b'baseline\n');source.chmod(0o644)
            before=runner.snapshot(workspace)
            commit=runner.initialize_snapshot(workspace,home,'1'*40)
            # Recorded with the previous initializer on the identical fixture.
            self.assertEqual(commit,'ea47a8b19202989f9ded707fc4c3bb27bc50b5be')
            self.assertEqual(runner.snapshot(workspace),before)
            for setting,expected in [('gc.auto','0'),('maintenance.auto','false')]:
                actual=subprocess.check_output(['git','config','--local','--get',setting],cwd=workspace,text=True).strip()
                self.assertEqual(actual,expected)

    def test_namespace_has_no_host_root_or_home_mount(self):
        argv=runner.arguments(Path('/owned/source'),Path('/owned/home'),Path('/owned/tools'),Path('/owned/inputs'),Path('/owned/provider.sock'))
        self.assertIn('--unshare-all',argv)
        self.assertIn('--clearenv',argv)
        self.assertNotIn('/home/user',argv)
        self.assertNotIn('/',argv)
        self.assertNotIn('BOAT_API_KEY',argv)



class Metering(unittest.TestCase):
    def config(self):
        return {'run_id':'fixed-run','models':{'frozen-model':{'max_requests':2,'usd_per_million':{'input':4,'output':20,'cache_write_5m':5,'cache_write_1h':8,'cache_read':0.2}}},'max_requests':3,'max_concurrent':1,'max_output_tokens':100,'admission_target_usd':2}

    def test_streaming_usage_is_cumulative_and_cache_is_split(self):
        usage=broker.Usage('text/event-stream')
        events=[{'type':'message_start','message':{'model':'frozen-model','usage':{'input_tokens':10,'output_tokens':1,'cache_read_input_tokens':100,'cache_creation_input_tokens':30,'cache_creation':{'ephemeral_5m_input_tokens':20,'ephemeral_1h_input_tokens':10}}}}, {'type':'message_delta','usage':{'output_tokens':8}}, {'type':'message_delta','usage':{'output_tokens':12}}, {'type':'message_stop'}]
        payload=b''.join(b'data: '+json.dumps(x).encode()+b'\n\n' for x in events)
        for i in range(0,len(payload),7):usage.feed(payload[i:i+7])
        self.assertTrue(usage.finish())
        self.assertEqual(usage.tokens['output_tokens'],12)
        cost,basis=broker.priced(usage.tokens,self.config()['models']['frozen-model']['usd_per_million'])
        self.assertAlmostEqual(cost,0.000480)
        self.assertEqual(basis,'reported_tokens')

    def test_json_and_incomplete_stream(self):
        usage=broker.Usage('application/json')
        usage.feed(json.dumps({'type':'message','model':'frozen-model','usage':{'input_tokens':7,'output_tokens':9}}).encode())
        self.assertTrue(usage.finish())
        missing=broker.Usage('text/event-stream')
        missing.feed(b'data: {"type":"message_start","message":{"model":"frozen-model","usage":{"input_tokens":10,"output_tokens":1}}}\n\n')
        self.assertFalse(missing.finish())
        self.assertEqual(missing.tokens['output_tokens'],1)

    def test_unknown_usage_closes_admission_and_preserves_reservation(self):
        with tempfile.TemporaryDirectory() as tmp:
            meter=broker.Meter(self.config(),Path(tmp)/'calls.jsonl')
            body=b'{"model":"frozen-model","max_tokens":100}'
            call=meter.begin('frozen-model','/v1/messages',body,json.loads(body))
            with self.assertRaises(ValueError):meter.begin('frozen-model','/v1/messages',body,json.loads(body))
            meter.end(call,{'status':'transport_error','cost_usd':None})
            with self.assertRaises(ValueError):meter.begin('frozen-model','/v1/messages',body,json.loads(body))
            self.assertGreater(meter.unknown_reservations,0)
            self.assertEqual(meter.spent,0)
            meter.handle.close()

    def test_direct_socket_call_is_charged_and_cap_is_enforced(self):
        import socket
        import threading
        class Response:
            status=200
            def __init__(self):self.body=io.BytesIO(b'{"type":"message","model":"frozen-model","usage":{"input_tokens":100,"output_tokens":25}}')
            def getheader(self,name,default=None):return 'application/json' if name=='content-type' else default
            def getheaders(self):return [('Content-Type','application/json')]
            def read1(self,n):return self.body.read(n)
        class Upstream:
            def request(self,*args):pass
            def getresponse(self):return Response()
            def close(self):pass
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);config=self.config();config['models']['frozen-model']['max_requests']=1
            meter=broker.Meter(config,root/'calls.jsonl')
            with broker.UnixServer(str(root/'provider.sock'),broker.Forward) as server:
                server.meter=meter;server.token='synthetic-only';server.connection_factory=Upstream
                thread=threading.Thread(target=server.serve_forever);thread.start()
                def request():
                    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as conn:
                        conn.connect(str(root/'provider.sock'))
                        body=b'{"model":"frozen-model","max_tokens":100}'
                        conn.sendall(b'POST /v1/messages HTTP/1.0\r\nContent-Length: '+str(len(body)).encode()+b'\r\n\r\n'+body)
                        result=b''
                        while chunk:=conn.recv(4096):result+=chunk
                        return result
                first=request();second=request()
                server.shutdown();thread.join()
            meter.handle.close()
            rows=[json.loads(x) for x in (root/'calls.jsonl').read_text().splitlines()]
            self.assertIn(b'200 OK',first);self.assertIn(b'403',second)
            self.assertEqual([r['phase'] for r in rows],['admitted','finished','refused'])
            self.assertAlmostEqual(rows[1]['cost_usd'],0.0009)
            self.assertEqual(rows[1]['usage']['output_tokens'],25)
            self.assertNotIn('synthetic-only',(root/'calls.jsonl').read_text())

    def test_toolchain_options_do_not_split_chdir_pair(self):
        argv=runner.arguments(Path('/owned/source'),Path('/owned/home'),Path('/owned/tools'),Path('/owned/inputs'),Path('/owned/provider.sock'),{'environment':{'CARGO_HOME':'/home/executor/.cargo'}})
        self.assertEqual(argv[argv.index('--chdir')+1],'/workspace')
        self.assertLess(argv.index('CARGO_HOME'),argv.index('--'))


class Retention(unittest.TestCase):
    def test_malformed_stream_never_discards_known_native_cost(self):
        with tempfile.TemporaryDirectory() as temporary:
            path=Path(temporary)/'events.jsonl'
            path.write_text('null\n'+json.dumps({'type':'result','is_error':False,'total_cost_usd':.25,'usage':{},'modelUsage':{'claude-opus-5-5':{}}})+'\n')
            result=runner.native_summary(path,0,False)
            self.assertFalse(result['model_completed'])
            self.assertEqual(result['cost_usd'],.25)
            self.assertIn('non_object_event',result['trace_parse_errors'])
            path.write_text(json.dumps({'type':'result','is_error':False,'total_cost_usd':.25,'usage':{},'modelUsage':[]})+'\n')
            result=runner.native_summary(path,0,False)
            self.assertFalse(result['model_completed'])
            self.assertIsNone(result['model_usage'])
            self.assertEqual(result['cost_usd'],.25)


class CaptureBounds(unittest.TestCase):
    def test_sparse_file_and_aggregate_size_are_bounded_without_large_reads(self):
        from capture_limits import Limits, CaptureLimit
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            with (root/'sparse').open('wb') as handle:handle.truncate(1024)
            with self.assertRaises(CaptureLimit):runner.snapshot(root,Limits({'max_file_bytes':32}))
            (root/'sparse').unlink()
            (root/'one').write_bytes(b'12345678');(root/'two').write_bytes(b'12345678')
            with self.assertRaises(CaptureLimit):runner.snapshot(root,Limits({'max_total_bytes':12}))

    def test_chunk_loop_rechecks_deadline_and_growth(self):
        from capture_limits import Limits, Reader, CaptureLimit
        limit=Limits({'max_file_bytes':4})
        reader=Reader(io.BytesIO(b'12345'),limit)
        self.assertEqual(reader.read(3),b'123')
        with self.assertRaises(CaptureLimit):reader.read(3)
        limit=Limits();limit.deadline=0
        with self.assertRaises(CaptureLimit):Reader(io.BytesIO(b'x'),limit).read(1)

class PayloadBound(unittest.TestCase):
    def test_compressed_payload_writer_stops_at_limit(self):
        import time
        from capture_limits import Writer, CaptureLimit
        output=io.BytesIO();writer=Writer(output,time.monotonic()+1,4)
        writer.write(b'123')
        with self.assertRaises(CaptureLimit):writer.write(b'45')
        self.assertEqual(output.getvalue(),b'123')


if __name__=='__main__':unittest.main()
