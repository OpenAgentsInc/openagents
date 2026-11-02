#!/usr/bin/env python3
# Firm reason: 2 (independent compiler, ABI, diagnostic and native-effect oracles; no product implementation).
"""New forms are traversed or explicitly refused by published inspections."""
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest
import quiet_crash

ROOT = Path(__file__).resolve().parents[1]
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT / 'bin/coax'))


class ResultProfiles(unittest.TestCase):
    def setUp(self):
        retained = os.environ.get('COAX_RESULTS_PROFILE_DIR')
        if retained:
            self.directory = Path(retained) / self._testMethodName
            self.directory.mkdir(parents=True, exist_ok=False)
        else:
            self.temporary = tempfile.TemporaryDirectory(prefix='coax-results-profiles-')
            self.addCleanup(self.temporary.cleanup)
            self.directory = Path(self.temporary.name)
        self.command_count = 0
        self.source_count = 0

    def source(self, text):
        source = self.directory / ('profile-%02d.coax' % self.source_count)
        self.source_count += 1
        source.write_text(text + '\nmain :: () {}\n')
        return source

    def run_coax(self, *args):
        return self.run_tool([str(COMPILER), *map(str, args)])

    def run_tool(self, args):
        result = subprocess.run(quiet_crash.command(list(map(str, args))), cwd=ROOT.parent,
                                env={**os.environ, 'COAX_QUERY_CACHE': str(self.directory / 'cache')},
                                capture_output=True, text=True, timeout=120)
        prefix = self.directory / ('command-%02d' % self.command_count)
        self.command_count += 1
        prefix.with_suffix('.stdout').write_text(result.stdout)
        prefix.with_suffix('.stderr').write_text(result.stderr)
        prefix.with_suffix('.json').write_text(json.dumps({'argv':list(map(str,args)), 'returncode':result.returncode},indent=2)+'\n')
        return result

    def emit(self, source, stem):
        header = self.directory / (stem + '.h')
        result = self.run_coax('c', source, '--library', '--header', header)
        self.assertEqual(result.returncode, 0, result.stderr)
        c = self.directory / (stem + '.c')
        c.write_text(result.stdout)
        return c, header

    def test_legacy_inventories_refuse_without_partial_output_or_cache(self):
        source = self.source('Reply :: union { value:int; empty; } root :: (r:Reply)->int { switch r { case value(x) { return x; } case empty { return 0; } } }')
        for args in (('query', source, 'defines', 'root'),
                     ('deps', source, '--declarations'), ('deps', source, '--modules')):
            with self.subTest(command=args[0:1] + args[2:]):
                result = self.run_coax(*args, '--diagnostics', 'json')
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertEqual(result.stdout, '')
                diagnostics = [json.loads(line) for line in result.stderr.splitlines()]
                self.assertTrue(diagnostics)
                self.assertEqual(diagnostics[0]['code'], 'E0911')
        self.assertFalse(list((self.directory / 'cache').glob('*.index')))
        # Plain deps describes physical inputs, so its complete old contract
        # remains usable even for a source whose typed inventory is unsupported.
        result = self.run_coax('deps', source)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(str(source.resolve()), json.loads(result.stdout)['sources'])

    def test_lowering_v1_names_the_unsupported_procedure(self):
        source = self.source('root :: (x:int?)->int { return x or_else 7; }')
        result = self.run_coax('deps', source, '--lowering', '--proc', 'root')
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report['schema'], 'coax.lowering.v1')
        self.assertEqual(len(report['procedures']), 1)
        row = report['procedures'][0]
        self.assertEqual(row['name'], 'root')
        self.assertFalse(row['eligible'])
        self.assertIn('sum-capable', row['reason'])
        self.assertNotIn('operations', row)

    def test_logical_modules_keep_sum_exports_and_error_identity(self):
        left=self.directory/'left.coax'
        left.write_text('#module "results.left";\n#export Failure, Reply, produce;\nFailure :: enum u8 { bad; }\nReply :: union { bytes:[]u8; empty; }\nPrivate :: union { hidden:int; }\nproduce :: (n:int)->Result(int,Failure) { if n<0 { return Result(int,Failure).{err=Failure.bad}; } return Result(int,Failure).{ok=n}; }\n')
        right=self.directory/'right.coax'
        right.write_text('#module "results.right";\n#export Failure;\nFailure :: enum u8 { bad; }\n')
        source=self.source('#load "left.coax" as left;\nsize :: (reply:left.Reply)->int { switch reply { case bytes(data) { return data.count; } case empty { return 0; } } }\n#test "module sums" { data:[2]u8; #assert size(left.Reply.{bytes=data[..]})==2; #assert size(left.Reply.{empty})==0; #assert (left.produce(7) or_else 0)==7; #assert (left.produce(-1) or_else 9)==9; }')
        result=self.run_coax('test',source)
        self.assertEqual(result.returncode,0,result.stderr)
        c,header=self.emit(source,'module-sums')
        result=self.run_tool([os.environ.get('CC','cc'),'-std=c11','-Werror=return-type','-c',c,'-o',self.directory/'module-sums.o'])
        self.assertEqual(result.returncode,0,result.stderr)
        source=self.source('#load "left.coax" as left;\n#load "right.coax" as right;\nbad :: ()->Result(int,right.Failure) { value:=try left.produce(3); return Result(int,right.Failure).{ok=value}; }')
        result=self.run_coax('check',source,'--diagnostics','json')
        self.assertEqual(result.returncode,1,result.stderr)
        self.assertEqual(json.loads(result.stderr.splitlines()[0])['code'],'E0905')
        source=self.source('#load "left.coax" as left;\nbad :: (value:left.Private) {}')
        result=self.run_coax('check',source,'--diagnostics','json')
        self.assertEqual(result.returncode,1,result.stderr)
        self.assertEqual(json.loads(result.stderr.splitlines()[0])['code'],'E0389')

    def test_effects_v1_visits_each_case_and_fallback(self):
        source = self.source('io :: ()->int #foreign; Reply :: union { value:int; empty; } root :: (r:Reply)->int { switch r { case value(x) { return x; } case empty { return some(io()) or_else io(); } } }')
        result = self.run_coax('deps', source, '--effects', '--proc', 'root')
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report['schema'], 'coax.effects.v1')
        effects = {e['kind']: e for e in report['procedures'][0]['effects']}
        self.assertIn('unknown_effects', effects)
        self.assertIn('foreign_call', effects)
        self.assertEqual([p['name'] for p in effects['foreign_call']['path']], ['root', 'io'])

    def test_effects_v1_marks_enum_switch_unsupported_and_visits_case(self):
        source=self.source('io :: ()->int #foreign; Flag :: enum u8 { off; on; } root :: (f:Flag)->int { switch f { case Flag.off { return 0; } case Flag.on { return io(); } } }')
        result=self.run_coax('deps',source,'--effects','--proc','root')
        self.assertEqual(result.returncode,0,result.stderr)
        effects={e['kind']:e for e in json.loads(result.stdout)['procedures'][0]['effects']}
        self.assertIn('unknown_effects',effects)
        self.assertEqual([p['name'] for p in effects['foreign_call']['path']],['root','io'])

    def test_optional_none_does_not_construct_inactive_payload_storage(self):
        # A million-element payload would exceed this independently chosen
        # evaluator slot budget if none constructed it. Type layout alone is
        # valid and does not allocate evaluator element storage.
        source = self.source('root :: ()->([1000000]int)? { absent:([1000000]int)?; return absent; } #test "none constructs no payload" { #assert root()==null; }')
        result = self.run_coax('test', source, '--slots', '64')
        self.assertEqual(result.returncode, 0, result.stderr)
        result = self.run_coax('deps', source, '--effects', '--proc', 'root')
        self.assertEqual(result.returncode, 0, result.stderr)
        effects = {e['kind'] for e in json.loads(result.stdout)['procedures'][0]['effects']}
        self.assertNotIn('foreign_call', effects)

    def test_native_check_abi_refuses_new_control_without_partial_c(self):
        source = self.source('choose :: (x:int?)->int { return x or_else 7; } #test "active optional" { #assert choose(some(3))==3; }')
        result = self.run_coax('checks-c', source)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(result.stdout, '')
        self.assertIn('E0413', result.stderr)

    def test_r2_does_not_trust_unimplemented_effect_annotations(self):
        for declaration in ('#pure', '#effects()', '#effects(clock)'):
            with self.subTest(declaration=declaration):
                source = self.source('root :: ()->int ' + declaration + ' { return 7; }')
                result = self.run_coax('check', source, '--diagnostics', 'json')
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertEqual(result.stdout, '')
                self.assertEqual(json.loads(result.stderr.splitlines()[0])['code'], 'E0940')

    def test_compute_profiles_refuse_sums_and_exhaustive_switch(self):
        # These are valid native programs. Device profiles must refuse before
        # producing a partial kernel, including a switch over an ordinary enum.
        control=self.source('k :: (item:int,out:[]int) #export_c { out[item]=7; }')
        for command in ('cuda','metal-compute'):
            result=self.run_coax(command,control)
            self.assertEqual(result.returncode,0,result.stderr)
        for helper in (
            'choose :: ()->int { x:int?=some(7); return x or_else 0; }',
            'Flag :: enum u8 { off; on; } choose :: ()->int { switch Flag.on { case Flag.off { return 0; } case Flag.on { return 7; } } }',
        ):
            source=self.source(helper+' k :: (item:int,out:[]int) #export_c { out[item]=choose(); }')
            result=self.run_coax('check',source)
            self.assertEqual(result.returncode,0,result.stderr)
            for command,code in (('cuda','E0700'),('metal-compute','E0702')):
                with self.subTest(command=command,helper=helper):
                    result=self.run_coax(command,source,'--diagnostics','json')
                    self.assertEqual(result.returncode,1,result.stderr)
                    self.assertEqual(result.stdout,'')
                    self.assertEqual(json.loads(result.stderr.splitlines()[0])['code'],code)

    def test_shader_profiles_refuse_sums_and_exhaustive_switch(self):
        shader_types='#load "'+str(ROOT.parent/'verse/gpu/shader.coax')+'";\n'
        vertex=' r2_profile_vertex :: (vid:u32,iid:u32)->V4 { return v4(choose(),0,0,1); }'
        control=self.source(shader_types+'choose :: ()->f32 { return 7; }'+vertex)
        result=self.run_coax('spirv',control,'--vertex','r2_profile_vertex','-o',self.directory/'control.spv')
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertGreater((self.directory/'control.spv').stat().st_size,20)
        result=self.run_coax('wgsl',control,'--functions','choose')
        self.assertEqual(result.returncode,0,result.stderr)
        for helper in (
            'choose :: ()->f32 { x:f32?=some(7.0f32); return x or_else 0.0f32; }',
            'Flag :: enum u8 { off; on; } choose :: ()->f32 { switch Flag.on { case Flag.off { return 0; } case Flag.on { return 7; } } }',
        ):
            source=self.source(shader_types+helper+vertex)
            result=self.run_coax('check',source)
            self.assertEqual(result.returncode,0,result.stderr)
            refused=self.directory/('refused-%02d.spv'%self.source_count)
            for command,args in (('spirv',['--vertex','r2_profile_vertex','-o',refused]),('wgsl',['--functions','choose'])):
                with self.subTest(command=command,helper=helper):
                    result=self.run_coax(command,source,*args,'--diagnostics','json')
                    self.assertEqual(result.returncode,1,result.stderr)
                    self.assertEqual(result.stdout,'')
                    self.assertEqual(json.loads(result.stderr.splitlines()[0])['code'],'E0704')
                    self.assertFalse(refused.exists())

    def test_present_optionals_have_tags_independent_of_payload_bits(self):
        source = ROOT / 'tests/results-extra/optional-tags.coax'
        result = self.run_coax('test', source)
        self.assertEqual(result.returncode, 0, result.stderr)
        result = self.run_coax('c', source)
        self.assertEqual(result.returncode, 0, result.stderr)
        c = self.directory / 'optional-tags.c'
        c.write_text(result.stdout)
        for name, flags in (('O0',['-O0']),('O2',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all'])):
            with self.subTest(profile=name):
                binary = self.directory / name
                result = self.run_tool([os.environ.get('CC','cc'),'-std=c11',*flags,c,'-lm','-o',binary])
                self.assertEqual(result.returncode,0,result.stderr)
                result = self.run_tool([binary])
                self.assertEqual(result.returncode,0,result.stderr)
                self.assertEqual((result.stdout,result.stderr),('',''))

    def test_loop_versioning_tracks_switch_writes_and_case_bindings(self):
        for stem,traps in (('loop-switch-view-shrink',True),('loop-case-index',False)):
            source=ROOT/('tests/results-extra/'+stem+'.coax')
            result=self.run_coax('test',source)
            self.assertEqual(result.returncode,0,result.stderr)
            for mode,flags in (('checked',[]),('release',['--release'])):
                result=self.run_coax('c',source,*flags)
                self.assertEqual(result.returncode,0,result.stderr)
                c=self.directory/(stem+'-'+mode+'.c');c.write_text(result.stdout)
                for profile,ccflags in (('O0',['-O0']),('O2',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all'])):
                    binary=self.directory/(stem+'-'+mode+'-'+profile)
                    result=self.run_tool([os.environ.get('CC','cc'),'-std=c11',*ccflags,c,'-lm','-o',binary])
                    self.assertEqual(result.returncode,0,result.stderr)
                    result=self.run_tool([binary])
                    if traps:
                        self.assertNotEqual(result.returncode,0,'switch changed the descriptor after the loop guard')
                        self.assertEqual(result.stdout,'')
                        self.assertIn('index out of bounds',result.stderr)
                    else:
                        self.assertEqual((result.returncode,result.stdout,result.stderr),(0,'40\n',''))

    def test_fma_operand_propagation_and_cleanup(self):
        source=ROOT/'tests/results-extra/fma-propagation.coax'
        result=self.run_coax('test',source)
        self.assertEqual(result.returncode,0,result.stderr)
        result=self.run_coax('c',source)
        self.assertEqual(result.returncode,0,result.stderr)
        c=self.directory/'fma-propagation.c';c.write_text(result.stdout)
        for profile,flags in (('O0',['-O0']),('O2',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all'])):
            binary=self.directory/profile
            result=self.run_tool([os.environ.get('CC','cc'),'-std=c11',*flags,c,'-lm','-o',binary])
            self.assertEqual(result.returncode,0,result.stderr)
            result=self.run_tool([binary])
            self.assertEqual((result.returncode,result.stdout,result.stderr),(0,'',''))

    def test_explicit_records_once_only_headers_and_case_array_storage(self):
        for stem in ('explicit-records','loop-headers','case-array-storage'):
            with self.subTest(source=stem):
                source=ROOT/('tests/results-extra/'+stem+'.coax')
                result=self.run_coax('test',source)
                self.assertEqual(result.returncode,0,result.stderr)
                result=self.run_coax('c',source)
                self.assertEqual(result.returncode,0,result.stderr)
                c=self.directory/(stem+'.c');c.write_text(result.stdout)
                for profile,flags in (('O0',['-O0']),('O2',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all'])):
                    binary=self.directory/(stem+'-'+profile)
                    result=self.run_tool([os.environ.get('CC','cc'),'-std=c11','-Werror=return-type',*flags,c,'-lm','-o',binary])
                    self.assertEqual(result.returncode,0,result.stderr)
                    result=self.run_tool([binary])
                    self.assertEqual((result.returncode,result.stdout,result.stderr),(0,'',''))

    def test_print_and_constants_do_not_hide_errors_or_local_storage(self):
        for stem,code,line in (('print-bound-result','E0907',6),
                               ('print-fresh-result','E0907',4),
                               ('result-constant','E0900',2),
                               ('result-value-argument','E0900',2),
                               ('result-error-value-argument','E0900',2),
                               ('case-array-local','E0383',4)):
            with self.subTest(source=stem):
                source=ROOT/('tests/results-extra/'+stem+'.coax')
                result=self.run_coax('check',source,'--diagnostics','json')
                self.assertEqual(result.returncode,1,result.stderr)
                self.assertEqual(result.stdout,'')
                primary=json.loads(result.stderr.splitlines()[0])
                self.assertEqual((primary['code'],primary['span']['line']),(code,line))
                self.assertEqual(Path(primary['span']['file']).resolve(),source.resolve())

    def test_native_sum_layouts_signed_cases_and_invalid_tags(self):
        source = ROOT / 'tests/results-extra/shapes.coax'
        result = self.run_coax('test',source)
        self.assertEqual(result.returncode,0,result.stderr)
        c, header = self.emit(source,'shapes')
        text = header.read_text()
        self.assertIn('coax-c-sum-lp64-v1'.encode().hex(),text)
        def returns(name):
            match = re.search(r'\b(\w+)\s+'+name+r'\(void\);',text)
            self.assertIsNotNone(match,name)
            return match.group(1)
        choice, empty, nested, unit = (returns(n) for n in ('r_choice','r_empty','r_nested','r_unit'))
        optional = re.search(r'(?:_Bool|bool)\s+r_present\((\w+)\s+\w+\);',text).group(1)
        harness = self.directory/'observe.c'
        harness.write_text('''#include "shapes.h"
#include <assert.h>
#include <stddef.h>
typedef CHOICE Choice;
typedef EMPTY Empty;
typedef NESTED Nested;
typedef UNIT UnitResult;
typedef OPTIONAL Optional;
_Static_assert(sizeof(Choice)==16 && _Alignof(Choice)==8,"choice layout");
_Static_assert(offsetof(Choice,tag)==0 && offsetof(Choice,payload)==8,"choice offsets");
_Static_assert(sizeof(Empty)==4 && _Alignof(Empty)==4,"all-unit layout");
_Static_assert(sizeof(Nested)==24 && _Alignof(Nested)==8,"nested optional layout");
_Static_assert(sizeof(UnitResult)==12 && _Alignof(UnitResult)==4,"unit Result layout");
_Static_assert(offsetof(UnitResult,payload)==4,"error payload offset");
int main(int argc,char **argv){
 Choice c=r_choice();
 if(argc>1){
  if(argv[1][0]=='s'){c.tag=55;(void)r_choose(c);}
  if(argv[1][0]=='e'){(void)r_signed(2);}
  if(argv[1][0]=='o'){Optional o={.tag=2};(void)r_present(o);}
  return 99;
 }
 assert(c.tag==1 && c.payload.v1==9.5 && r_choose(c)==9);
 Empty e=r_empty();assert(e.tag==1);
 Nested n=r_nested();assert(n.tag==1 && n.payload.v1.tag==1 && n.payload.v1.payload.v1==7);
 UnitResult ok=r_unit(),bad=r_error();assert(ok.tag==0 && bad.tag==1);
 assert(bad.payload.v1.code==-41 && bad.payload.v1.operation==3);
 assert(r_signed(-1)==-7 && r_signed(0)==11 && r_signed(127)==19);
 Optional present={.tag=1,.payload.v1=0},none={.tag=0};
 assert(r_present(present) && !r_present(none));return 0;
}
'''.replace('CHOICE',choice).replace('EMPTY',empty).replace('NESTED',nested).replace('UNIT',unit).replace('OPTIONAL',optional))
        for name, flags in (('O0',['-O0']),('O2',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all'])):
            with self.subTest(profile=name):
                binary=self.directory/name
                result=self.run_tool([os.environ.get('CC','cc'),'-std=c11',*flags,c,harness,'-lm','-o',binary])
                self.assertEqual(result.returncode,0,result.stderr)
                result=self.run_tool([binary])
                self.assertEqual((result.returncode,result.stdout,result.stderr),(0,'',''))
                for invalid in ('sum','enum','optional'):
                    result=self.run_tool([binary,invalid])
                    self.assertNotEqual(result.returncode,0,invalid)
                    self.assertIn('invalid',result.stderr,invalid)
                    self.assertNotIn('runtime error:',result.stderr)
                    self.assertNotIn('ERROR: AddressSanitizer',result.stderr)

    def test_parameterized_sum_names_cannot_collide_at_a_separator(self):
        source=self.source('''A__u_B :: union {number:int;}
C :: union {number:int;}
A :: union {number:int;}
B__u_C :: union {number:int;}
first :: ()->Result(A__u_B,C) #export_c {return Result(A__u_B,C).{ok=A__u_B.{number=3}};}
second :: ()->Result(A,B__u_C) #export_c {return Result(A,B__u_C).{err=B__u_C.{number=7}};}
''')
        _,header=self.emit(source,'names')
        text=header.read_text()
        types=[re.search(r'\b(\w+)\s+'+name+r'\(void\);',text).group(1) for name in ('first','second')]
        self.assertNotEqual(*types)
        composed=self.directory/'compose.c'
        composed.write_text('#include "names.h"\n#include "names.h"\nint main(void){return 0;}\n')
        result=self.run_tool([os.environ.get('CC','cc'),'-std=c11',composed,'-o',self.directory/'compose'])
        self.assertEqual(result.returncode,0,result.stderr)

    def test_changed_sum_policies_in_the_actual_native_compiler(self):
        artifacts=Path(os.environ.get('COAX_TEST_ARTIFACT_DIR',ROOT/'bin')).resolve()
        # Include one complete emitted implementation, so these are the native
        # compiler's actual policies rather than translated test substitutes.
        source=ROOT/'tests/results-sum-native.c'
        for name,flags in (('O0',['-O0']),('O2',['-O2']),
                           ('sanitized',['-O1','-g','-fsanitize=address,undefined',
                                         '-fno-sanitize-recover=all','-fno-omit-frame-pointer'])):
            with self.subTest(profile=name):
                binary=self.directory/name
                result=self.run_tool([os.environ.get('CC','cc'),'-std=c11',*flags,
                                      '-I',artifacts,source,'-lm','-o',binary])
                self.assertEqual(result.returncode,0,result.stderr)
                result=self.run_tool([binary])
                self.assertEqual((result.returncode,result.stdout,result.stderr),(0,'',''))


if __name__ == '__main__':
    unittest.main()
