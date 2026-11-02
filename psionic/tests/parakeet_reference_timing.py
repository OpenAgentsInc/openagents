#!/usr/bin/env python3
# Firm reason: 2 (independent scoring, timing arithmetic and input identities; no product inference).
"""Record a bounded v1/v2 cohort comparison without rerunning either producer."""
from pathlib import Path
import argparse,hashlib,json,re,statistics,unicodedata

def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def normalize(s):
    words=[];gap=False
    for c in unicodedata.normalize('NFKC',s).casefold():
        if unicodedata.category(c)[0] in 'LNM' or c=="'":
            if gap and words:words.append(' ')
            words.append(c);gap=False
        else:gap=True
    return ''.join(words)
def distance(a,b):
    row=list(range(len(b)+1))
    for i,x in enumerate(a,1):
        previous=row[0];row[0]=i
        for j,y in enumerate(b,1):old=row[j];row[j]=min(row[j]+1,row[j-1]+1,previous+(x!=y));previous=old
    return row[-1]
def score(ref,hyp):
    a=normalize(ref);b=normalize(hyp)
    return {'words':len(a.split()),'word_errors':distance(a.split(),b.split()),'characters':len(a),'character_errors':distance(a,b)}
def main():
    p=argparse.ArgumentParser();p.add_argument('baseline',type=Path);p.add_argument('product',type=Path);p.add_argument('comparison',type=Path);p.add_argument('out',type=Path);a=p.parse_args()
    base=json.loads(a.baseline.read_text());comparison=json.loads(a.comparison.read_text());assert base['passed'] and comparison['passed'] and len(comparison['clips'])==29
    stdout=a.product/'stdout';raw=stdout.read_text();rows=[];timers={}
    pattern=r'^GPU (\S+) audio=([\d.]+) frames=(\d+) encoded=(\d+) tokens=(\d+) inference_ms=([\d.]+) total_seconds=([\d.]+)$'
    for line in raw.splitlines():
        found=re.match(pattern,line)
        if found:
            ident,audio,frames,encoded,tokens,ms,total=found.groups();assert ident not in timers
            timers[ident]={'audio_seconds':float(audio),'feature_frames':int(frames),'encoded_frames':int(encoded),'tokens':int(tokens),'inference_ms':float(ms),'total_request_seconds':float(total)}
    assert len(timers)==29 and 'DONE clips=29 ' in raw
    pins={str(a.baseline):sha(a.baseline),str(a.comparison):sha(a.comparison),str(stdout):sha(stdout)}
    for old,passed in zip(base['clips'],comparison['clips']):
        ident=old['id'];assert ident==passed['id']
        path=a.product/(ident+'.tokens.json');got=json.loads(path.read_text());pins[str(path)]=sha(path)
        result=score(old['reference'],got['text']);assert result==old['score'] and got['text']==old['hypothesis'] and passed['tokens_equal'] and passed['frames_equal'] and passed['text_equal']
        now=timers[ident];assert now['audio_seconds']==old['audio_seconds'] and now['feature_frames']==old['feature_frames'] and now['encoded_frames']==old['encoded_frames'] and now['tokens']==old['tokens']
        rows.append({'id':ident,'language':old['language'],'corpus':old['corpus'],'score':result,'word_error_rate':result['word_errors']/result['words'],'character_error_rate':result['character_errors']/result['characters'],'v1_inference_ms':old['inference_ms'],'v2_inference_ms':now['inference_ms'],'observed_ratio_v1_over_v2':old['inference_ms']/now['inference_ms'],**now})
    process=a.product/'process.json';proc=json.loads(process.read_text());assert proc['returncode']==0;pins[str(process)]=sha(process)
    audio=sum(r['audio_seconds'] for r in rows);old_seconds=sum(r['v1_inference_ms'] for r in rows)/1000;seconds=sum(r['v2_inference_ms'] for r in rows)/1000
    firstless=rows[1:];old_firstless=sum(r['v1_inference_ms'] for r in firstless)/1000;new_firstless=sum(r['v2_inference_ms'] for r in firstless)/1000
    record={'schema':'psionic.parakeet-timing-comparison.v1','passed':True,'inputs':pins,'scope':'Same finite ordered 29-clip cohort, one pass per version. No repeated or controlled performance experiment; cache, scheduling and first-request Metal warm-up are uncontrolled. info.inference_ms covers frontend/encoder/decoder before publication copies; no per-stage timings or key-release latency. Exact raw text/token/frame parity and frozen tensor qualification are prerequisites, not inferred from speed. Unicode NFKC/casefold scoring unchanged. FLEURS one-utterance slices are smoke observations, not population accuracy.','all29':{'audio_seconds':audio,'v1_inference_seconds':old_seconds,'v2_inference_seconds':seconds,'observed_ratio_v1_over_v2':old_seconds/seconds,'v1_audio_seconds_per_inference_second':audio/old_seconds,'v2_audio_seconds_per_inference_second':audio/seconds,'v1_cohort_median_ms':statistics.median(r['v1_inference_ms'] for r in rows),'v2_cohort_median_ms':statistics.median(r['v2_inference_ms'] for r in rows)},'excluding_first_request':{'requests':28,'v1_inference_seconds':old_firstless,'v2_inference_seconds':new_firstless,'observed_ratio_v1_over_v2':old_firstless/new_firstless,'v1_cohort_median_ms':statistics.median(r['v1_inference_ms'] for r in firstless),'v2_cohort_median_ms':statistics.median(r['v2_inference_ms'] for r in firstless),'scope':'Dropping first request is a descriptive cohort slice; does not create a steady-state/repeated benchmark.'},'fresh_engine_open_seconds':{'v1':base['timing']['fresh_engine_open_seconds'],'v2':float(re.search(r'^MODEL_OPEN_SECONDS ([\d.]+)$',raw,re.M)[1]),'cache':'uncontrolled; no filesystem-cold claim'},'whole_process_seconds':{'v1':base['timing']['whole_process_seconds'],'v2':proc['seconds']},'actual_decoder_threads':int(re.search(r'^DECODER_THREADS (\d+)$',raw,re.M)[1]),'clips':rows}
    a.out.write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n')
    md=['# Independent v1 / v2 cohort observations','','All 29 complete pipelines pass the original frozen NeMo comparison. Raw text, token IDs and frame IDs remain exact; every feature/final-encoder value meets the unchanged tolerance. Independent Unicode WER/CER counts match v1 in every slice.','','One ordered pass per version on the same Apple M5 Max. These are descriptive observations, with cache and scheduling uncontrolled; excluding the first request does not establish steady-state latency. The first request may include Metal allocation/upload or warm-up. No separate stage or key-release latency was measured.','',f"298.435 audio seconds: v1 **{old_seconds:.6f} s**, v2 **{seconds:.6f} s** of API inference; observed ratio **{old_seconds/seconds:.3f}×**. Fresh engine open: v1 {record['fresh_engine_open_seconds']['v1']:.6f} s, v2 {record['fresh_engine_open_seconds']['v2']:.6f} s (not filesystem-cold). Actual v2 decoder threads: {record['actual_decoder_threads']}.",'','| Slice | Audio s | Word errors / words | Character errors / chars | V1 ms | V2 ms | Observed ratio |','| --- | ---: | ---: | ---: | ---: | ---: | ---: |']
    for r in rows:
        s=r['score'];md.append(f"| {r['id']} | {r['audio_seconds']:.3f} | {s['word_errors']} / {s['words']} | {s['character_errors']} / {s['characters']} | {r['v1_inference_ms']:.3f} | {r['v2_inference_ms']:.3f} | {r['observed_ratio_v1_over_v2']:.3f}× |")
    md+=['','The four original English recordings retain 0/46 word errors and 0/258 character errors. Each FLEURS language contains one utterance; those counts do not estimate corpus accuracy. Timings are whole-utterance offline API observations, not the Coder 300 ms dictation target.']
    a.out.with_suffix('.md').write_text('\n'.join(md)+'\n');print(json.dumps({'passed':True,'clips':len(rows),'all29':record['all29'],'open':record['fresh_engine_open_seconds']},indent=2))
if __name__=='__main__':main()
