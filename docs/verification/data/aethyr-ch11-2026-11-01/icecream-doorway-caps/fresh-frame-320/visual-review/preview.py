from pathlib import Path
import array,hashlib,json,math,struct,sys,zlib
out=Path('/tmp/aethyr-ch11-icecream-doorway-main-visual-review');out.mkdir(exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
base=Path('/tmp/aethyr-ch11-icecream-bands-half-320');fresh=Path('/tmp/aethyr-ch11-icecream-doorway-main-half-320')
a=array.array('I',(base/'s0.bin').read_bytes());b=array.array('I',(fresh/'s0.bin').read_bytes());assert sys.byteorder=='little';changed_words=0;changed_vertices=set()
for index,(x,y)in enumerate(zip(a,b)):
 if x!=y:assert index%6<3;changed_words+=1;changed_vertices.add(index//6)
proof={'mesh_changed_words':changed_words,'mesh_changed_vertices':len(changed_vertices),'changed_attributes_beyond_position':0,'preview':'Top WebGPU; bottom Vulkan; left band-only; right doorway caps. Linear half values converted to sRGB for visual review only.','pairs':{}}
def png(path,w,h,raw):
 def chunk(kind,payload):return struct.pack('>I',len(payload))+kind+payload+struct.pack('>I',zlib.crc32(kind+payload))
 rows=b''.join(b'\0'+raw[y*w*3:(y+1)*w*3]for y in range(h));path.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>2I5B',w,h,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(rows))+chunk(b'IEND',b''))
def preview(data):
 output=bytearray()
 for rgba in struct.iter_unpack('<4e',data):
  assert all(math.isfinite(n)for n in rgba)
  for c in rgba[:3]:
   c=max(0,min(1,c));c=12.92*c if c<=.0031308 else 1.055*c**(1/2.4)-.055;output.append(round(c*255))
 return output
images=[]
for backend,before,after in [('webgpu',base/'gpu.bin',fresh/'gpu.bin'),('vulkan',Path('/tmp/aethyr-ch11-native-icecream-bands-320/screen.raw'),Path('/tmp/aethyr-ch11-native-icecream-doorway-main-320/screen.raw'))]:
 data=[before.read_bytes(),after.read_bytes()];mask=channels=0;worst=0
 for old,new in zip(struct.iter_unpack('<4e',data[0]),struct.iter_unpack('<4e',data[1])):
  delta=[x!=y for x,y in zip(old,new)];mask+=any(delta);channels+=sum(delta);worst=max(worst,*[abs(x-y)for x,y in zip(old,new)])
 proof['pairs'][backend]={'before':str(before),'after':str(after),'before_sha256':sha(before),'after_sha256':sha(after),'changed_channels':channels,'changed_pixels':mask,'max_absolute_change':worst}
 for name,d in zip(['before','after'],data):
  rgb=preview(d);png(out/f'{backend}-{name}.png',320,180,rgb);images.append(rgb)
w,h=1280,740;raw=bytearray([30])*(w*h*3)
for i,rgb in enumerate(images):
 x0=(i%2)*640;y0=(i//2)*380
 for y in range(180):
  row=b''.join(rgb[(y*320+x)*3:(y*320+x+1)*3]*2 for x in range(320))
  for dy in range(2):at=((y0+y*2+dy)*w+x0)*3;raw[at:at+1920]=row
png(out/'review.png',w,h,raw);(out/'review-inputs.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
