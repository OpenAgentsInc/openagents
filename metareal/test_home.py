#!/usr/bin/env python3
"""Real generated Coax + resident bridge + canonical protoc interoperability."""
import ctypes as C
import math
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
ROOT=Path(__file__).resolve().parents[1]
PROTOC=os.environ.get('PROTOC','protoc')
RESOURCE=ROOT/'apps/openagents-ios/OpenAgents/Resources/Home.pb'

class Dot(C.Structure): _fields_=[(name,C.c_float) for name in ['x','y','size','light','soft','crisp','pad0','pad1']]
class Line(C.Structure): _fields_=[(name,C.c_float) for name in ['ax','ay','bx','by','lightA','lightB','width','pad']]
class Bar(C.Structure): _fields_=[(name,C.c_float) for name in ['x','y','width','height','press','flash']]+[('label',C.c_char*33)]

def proto(command,data):
    return subprocess.run([PROTOC,'-I'+str(ROOT/'proto'),command,'openagents/metareal/home/v1alpha1/home.proto'],input=data,capture_output=True,check=True).stdout

def config(**changes):
    fields={'schema_version':1,'seed':42727,'mote_count':4000,'satellite_count':8}
    action=changes.pop('action','id:"home.start" label:"Start" enabled:true shown:true')
    fields.update(changes)
    text='\n'.join(f'{k}: {v}' for k,v in fields.items() if v is not None)+'\nstart {'+action+'}\n'
    return proto('--encode=openagents.metareal.home.v1alpha1.HomeConfiguration',text.encode())

class HomeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp=tempfile.TemporaryDirectory();d=Path(cls.tmp.name)
        for stem in ('home','light'):
            p=subprocess.run([str(ROOT/'coax/bin/coax'),'c',str(ROOT/f'metareal/{stem}.coax'),'--library'],capture_output=True,check=True)
            (d/f'{stem}.c').write_bytes(p.stdout)
        subprocess.run([os.environ.get('CC','cc'),'-std=c11','-O2','-shared','-fPIC',str(d/'home.c'),str(d/'light.c'),str(ROOT/'metareal/platform/host.c'),'-lm','-o',str(d/'home.dylib')],check=True)
        cls.lib=C.CDLL(str(d/'home.dylib'));l=cls.lib
        signatures={
            'mr_create':(C.c_int64,[C.c_void_p,C.c_size_t,C.c_size_t]),'mr_destroy':(None,[C.c_int64]),
            'mr_clear_frame':(None,[C.c_int64]),'mr_ok':(C.c_bool,[C.c_int64]),
            'mr_dot_count':(C.c_size_t,[C.c_int64]),'mr_line_count':(C.c_size_t,[C.c_int64]),
            'mr_dots':(C.POINTER(Dot),[C.c_int64]),'mr_lines':(C.POINTER(Line),[C.c_int64]),
            'mr_bar':(C.POINTER(Bar),[C.c_int64]),'mr_output':(C.c_void_p,[C.c_int64]),'mr_output_size':(C.c_size_t,[C.c_int64]),
            'host_load':(C.c_double,[C.c_int64,C.c_int64]),
            'metareal_home_capacity':(C.c_int64,[]),'metareal_home_init':(C.c_bool,[C.c_int64]),
            'metareal_home_frame':(None,[C.c_int64]+[C.c_double]*5+[C.c_bool]),
            'metareal_home_touch':(C.c_bool,[C.c_int64,C.c_int64,C.c_double,C.c_double]),
            'metareal_home_resume':(None,[C.c_int64]),'metareal_home_presses':(C.c_int64,[C.c_int64])}
        for name,(result,args) in signatures.items():
            f=getattr(l,name);f.restype=result;f.argtypes=args
    @classmethod
    def tearDownClass(cls): cls.tmp.cleanup()
    def setUp(self): self.handles=[]
    def tearDown(self):
        for h in self.handles:self.lib.mr_destroy(h)
    def create(self,data=None,valid=True):
        if data is None:data=RESOURCE.read_bytes()
        h=self.lib.mr_create(C.c_char_p(data),len(data),self.lib.metareal_home_capacity());self.handles.append(h)
        self.assertNotEqual(h,0)
        self.assertEqual(self.lib.metareal_home_init(h),valid)
        return h
    def frame(self,h,t=0,reduced=False,w=1206,hgt=2622):
        self.lib.mr_clear_frame(h);self.lib.metareal_home_frame(h,t,w,hgt,3,102,reduced)
        self.assertTrue(self.lib.mr_ok(h))
    def test_resource_reproducible(self):
        actual=proto('--encode=openagents.metareal.home.v1alpha1.HomeConfiguration',(ROOT/'metareal/home.textproto').read_bytes())
        self.assertEqual(RESOURCE.read_bytes(),actual)
    def test_admission(self):
        self.create(config(mote_count=0,satellite_count=0))
        for data in [config(schema_version=2),config(seed=0),config(seed=None),config(mote_count=None),
                     config(mote_count=4097),config(satellite_count=9),config(action='id:"wrong" label:"Start" enabled:true shown:true'),
                     config(action='id:"home.start" label:"" enabled:true shown:true'),
                     config(action='id:"home.start" label:"Start" enabled:false shown:true'),
                     config(action='id:"home.start" label:"Start" enabled:true shown:false'),
                     config(action='id:"home.start" label:"Start" enabled:true shown:true pressed:true'),
                     config()+b'\x08\x01',config()+b'\x30\x01',config()[:-1],b'\x08\x81\x00',b'\x08\xff\xff\xff\xff\xff',b'\0'*1025]:
            with self.subTest(data=data[:12]):
                h=self.create(data,False);self.lib.mr_destroy(h)
    def test_finite_bounded_animation_and_resume(self):
        h=self.create()
        for i in range(721):
            self.frame(h,i/60)
            self.assertLessEqual(self.lib.mr_dot_count(h),8192);self.assertLessEqual(self.lib.mr_line_count(h),8192)
        self.assertGreater(self.lib.mr_dot_count(h),4400);self.assertGreater(self.lib.mr_line_count(h),300)
        for packet,count in [(self.lib.mr_dots(h),self.lib.mr_dot_count(h)),(self.lib.mr_lines(h),self.lib.mr_line_count(h))]:
            for i in range(count):
                self.assertTrue(all(math.isfinite(getattr(packet[i],name)) for name,_ in packet[i]._fields_))
        t=self.lib.host_load(h,31);self.lib.metareal_home_resume(h);self.frame(h,10000)
        self.assertEqual(t,self.lib.host_load(h,31))
        for w,height in [(2622,1206),(2048,2732),(1,1),(0,0)]:self.frame(h,10000,w=w,hgt=height)
    def test_determinism_and_reduced_motion(self):
        a=self.create();b=self.create()
        self.frame(a,0,True);self.frame(b,0,True)
        def packets(h):return C.string_at(self.lib.mr_dots(h),self.lib.mr_dot_count(h)*C.sizeof(Dot))
        expected=packets(a);self.assertEqual(expected,packets(b));self.frame(a,100,True);self.assertEqual(expected,packets(a))
    def test_touch_and_shared_invocation(self):
        h=self.create();self.frame(h);bar=self.lib.mr_bar(h).contents
        self.assertEqual(bar.label,b'Start');x=bar.x+bar.width/2;y=bar.y+bar.height/2
        # Cancel and drag well outside do not activate; release alone never does.
        for phase,tx,ty in [(2,x,y),(0,x,y),(3,x,y),(0,x,y),(1,0,0),(2,0,0)]:
            self.assertFalse(self.lib.metareal_home_touch(h,phase,tx,ty))
        self.assertEqual(self.lib.metareal_home_presses(h),0)
        for n in range(1,13):
            self.assertFalse(self.lib.metareal_home_touch(h,0,x,y))
            self.assertTrue(self.lib.metareal_home_touch(h,2,x,y))
            self.assertFalse(self.lib.metareal_home_touch(h,2,x,y))
            data=C.string_at(self.lib.mr_output(h),self.lib.mr_output_size(h))
            result=subprocess.run([PROTOC,'-I'+str(ROOT/'proto'),'--decode=openagents.engine.v1.HudInvocation','openagents/engine/v1/hud.proto'],input=data,capture_output=True,check=True).stdout.decode()
            for text in ['schema: 1','world: "home"','action_id: "home.start"',f'request_id: "home-start-{n}"',f'revision: {n}']:self.assertIn(text,result)
        self.assertEqual(self.lib.metareal_home_presses(h),12)
    def test_generation_and_platform_bounds(self):
        a=self.create();self.lib.mr_destroy(a);b=self.create()
        self.assertNotEqual(a,b);self.assertFalse(self.lib.mr_ok(a));self.assertTrue(self.lib.mr_ok(b))
        self.assertEqual(self.lib.host_load(b,-1),0);self.assertFalse(self.lib.mr_ok(b))
    def test_shader_numeric_contract(self):
        dot=self.lib.metareal_dot;dot.restype=C.c_float;dot.argtypes=[C.c_float]*5
        self.assertEqual(dot(2,0,1,0,0),0)
        self.assertGreater(dot(0,0,1,0,1),dot(.7,0,1,0,1))
        tone=self.lib.metareal_tone;tone.restype=C.c_float;tone.argtypes=[C.c_float]
        self.assertAlmostEqual(tone(0),0);self.assertGreater(tone(2),tone(1))

if __name__=='__main__':unittest.main()
