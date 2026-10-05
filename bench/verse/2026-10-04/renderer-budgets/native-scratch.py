from pathlib import Path
import os,subprocess,tempfile,glob,sys
binary='/home/christopherdavid/work/openagents-target-agent1/debug/deps/verse-71a9dca49338e764'
libraries=[]
for package in ('libx11','libxcursor','libxrandr','libxi','libxkbcommon','libxext','libxfixes','libxcb'):
 libraries.extend(p for p in glob.glob('/nix/store/*-'+package+'-*/lib') if Path(p).is_dir())
with tempfile.TemporaryDirectory(prefix='verse-v11-x11-') as root:
 os.mkdir(root+'/home');os.mkdir(root+'/runtime',0o700)
 with open('/tmp/verse-v11-xvfb-current.log','w') as diagnostics:
  server=subprocess.Popen(['/nix/store/qwbmq81rc42zf9qgfim64jf4b4fnqwgz-xorg-server-21.1.24/bin/Xvfb','-displayfd','1','-screen','0','1280x720x24','-nolisten','tcp'],stdout=subprocess.PIPE,stderr=diagnostics,text=True)
  try:
   number=server.stdout.readline().strip()
   if not number:raise RuntimeError('Scratch X11 failed')
   env={**os.environ,'HOME':root+'/home','XDG_RUNTIME_DIR':root+'/runtime','DISPLAY':':'+number,'WGPU_BACKEND':'vulkan','VERSE_GPU_TIMING':'1','VK_ICD_FILENAMES':'/nix/store/d8wvsjgnk6giw73jhyv5rbl99jl4j5nb-mesa-26.1.8/share/vulkan/icd.d/lvp_icd.x86_64.json','LD_LIBRARY_PATH':':'.join(libraries+[os.environ.get('LD_LIBRARY_PATH','')])}
   env.pop('WAYLAND_DISPLAY',None)
   with open(sys.argv[2],'w') as output:
    result=subprocess.run([binary,sys.argv[1],'--ignored','--nocapture'],env=env,stdout=output,stderr=subprocess.STDOUT,timeout=60)
   print('Native scratch test exit:',result.returncode)
  finally:
   server.terminate();server.wait(timeout=5)
sys.exit(result.returncode)
