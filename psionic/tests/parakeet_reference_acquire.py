#!/usr/bin/env python3
# Firm reason: 2 (independent, versioned public-audio/reference and model-configuration acquisition oracle).
"""Freeze a small multilingual evaluation before inference; never import downloaded code.

One finite smoke utterance per Parakeet-v3 language is not a corpus accuracy claim.
Only standard-library HTTP/tar/WAV/JSON readers are used. Pick the first archive
member present in test.tsv with nonempty text and 1..20.47 seconds at16kHz.
"""
from pathlib import Path
import concurrent.futures,csv,gzip,hashlib,io,json,subprocess,tarfile,urllib.request,wave
REV='70bb2e84b976b7e960aa89f1c648e09c59f894dd'
MODEL_REV='541d1f99c6b0c3cd0b11a95167540bb8edefd82b'
NEMO_REV='5f15875acd0885ed92eadfee874ae87b0529ffa2'
LANGS={'bg':'bg_bg','hr':'hr_hr','cs':'cs_cz','da':'da_dk','nl':'nl_nl','en':'en_us','et':'et_ee','fi':'fi_fi','fr':'fr_fr','de':'de_de','el':'el_gr','hu':'hu_hu','it':'it_it','lv':'lv_lv','lt':'lt_lt','mt':'mt_mt','pl':'pl_pl','pt':'pt_br','ro':'ro_ro','sk':'sk_sk','sl':'sl_si','es':'es_419','sv':'sv_se','ru':'ru_ru','uk':'uk_ua'}
ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'psionic/fixtures/parakeet-reference'
def digest(b):return hashlib.sha256(b).hexdigest()
def fetch(url):
 with urllib.request.urlopen(url,timeout=60) as r:return r.read()
def language(item):
 lang,config=item
 saved=OUT/(lang+'.source.json')
 if saved.exists():
  row=json.loads(saved.read_text());assert digest((OUT/row['pcm_file']).read_bytes())==row['pcm_sha256'];return row
 prefix=f'https://huggingface.co/datasets/google/fleurs/resolve/{REV}/data/{config}/';tsv=fetch(prefix+'test.tsv');rows={r[1]:r for r in csv.reader(io.StringIO(tsv.decode()),delimiter='\t')}
 with urllib.request.urlopen(prefix+'audio/test.tar.gz',timeout=60) as response:
  with tarfile.open(fileobj=response,mode='r|gz') as archive:
   for member in archive:
    row=rows.get(Path(member.name).name)
    if not member.isfile() or row is None or not row[2].strip() or not 16000<=int(row[5])<=327520:continue
    raw=archive.extractfile(member).read()
    with wave.open(io.BytesIO(raw),'rb') as audio:
     assert audio.getframerate()==16000 and audio.getnchannels()==1 and audio.getcomptype()=='NONE'
     assert audio.getnframes()==int(row[5]);source_width=audio.getsampwidth()
     if source_width==2:pcm=audio.readframes(audio.getnframes())
     else:
      pcm=subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-i','pipe:0','-af','aresample=dither_method=none','-f','s16le','-acodec','pcm_s16le','-ac','1','-ar','16000','pipe:1'],input=raw,stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True,timeout=30).stdout
     assert len(pcm)==2*int(row[5])
    path=OUT/(lang+'.pcm');path.write_bytes(pcm)
    result={'language':lang,'config':config,'split':'test','utterance_id':row[0],'audio_member':member.name,'samples':len(pcm)//2,'rate':16000,'channels':1,'pcm_file':path.name,'pcm_sha256':digest(pcm),'wav_sha256':digest(raw),'source_sample_width':source_width,'conversion':'PCM source to signed16le at16kHz, ffmpeg dither_method=none when source width is not2','tsv_sha256':digest(tsv),'raw_reference':row[2],'reference':row[3],'gender':row[6],'source_archive':prefix+'audio/test.tar.gz','source_tsv':prefix+'test.tsv'}
    saved.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');return result
 raise RuntimeError('no admitted utterance: '+lang)
def config():
 url=f'https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3/resolve/{MODEL_REV}/parakeet-tdt-0.6b-v3.nemo'
 class Range(io.RawIOBase):
  def __init__(self):self.at=0
  def tell(self):return self.at
  def seek(self,p,w=0):self.at=p if w==0 else self.at+p;return self.at
  def read(self,n=-1):
   if not 0<=n<=1048576:raise ValueError('bounded metadata read')
   if not n:return b''
   with urllib.request.urlopen(urllib.request.Request(url,headers={'Range':f'bytes={self.at}-{self.at+n-1}'}),timeout=60) as r:
    assert r.status==206;data=r.read(n)
   self.at+=len(data);return data
 with tarfile.open(fileobj=Range(),mode='r:') as archive:
  for member in archive:
   if member.name=='./model_config.yaml':
    raw=archive.extractfile(member).read();(OUT/'nemo-model-config.yaml').write_bytes(raw)
    return {'url':url,'member':member.name,'bytes':len(raw),'sha256':digest(raw),'offset_data':member.offset_data,'pickle_loaded':False}
 raise RuntimeError('missing config')
def main():
 OUT.mkdir(exist_ok=True,parents=True)
 if (OUT/'manifest.json').exists():raise SystemExit('refuse to overwrite an already frozen evaluation')
 cfg=config();rows=[]
 with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
  for row in pool.map(language,LANGS.items()):rows.append(row);print(row['language'],row['samples'],row['pcm_sha256'],flush=True)
 manifest={'schema':'psionic.parakeet-reference-set.v1','dataset':'google/fleurs','dataset_revision':REV,'license':'CC-BY-4.0','selection':'First test.tar.gz member listed in same-pinned test.tsv, nonempty reference and 16000..327520samples. Chosen before either implementation is run; no model-output selection.','normalization':'Unicode NFKC, casefold, Unicode letter/number/mark categories retained, apostrophe retained, other characters separate words; CER counts Unicode codepoints including normalized spaces.','purpose':'A fixed one-utterance smoke slice per supported language; report each WER/CER. Not an estimate of corpus/model quality.','acceptance':{'original_english_word_error_percent':5,'original_english_char_error_percent':3,'token_ids_match_reference':True,'token_frames_match_reference':True,'tensor_tolerance_abs':0.001,'tensor_tolerance_rel':0.001,'multilingual_quality_threshold':None},'model_revision':MODEL_REV,'nemo_source_revision':NEMO_REV,'profile':{'float':'float32','torch_tf32':False,'dither':0,'pad_to':0,'max_symbols_per_step':10,'decoder_strategy':'GreedyTDTInfer','cuda_graphs':False},'nemo_config':cfg,'clips':rows}
 (OUT/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n');print('frozen',len(rows),'language slices')
if __name__=='__main__':main()
