# 36. Tokenizers and chat templates

Chapter 5 provides byte BPE. Here we add WordPiece's longest-match search and
Unigram's best-scoring path, with the spacing and normalization data of a
SentencePiece tokenizer. SentencePiece is a tokenizer system, rather than a
third segmentation algorithm: this chapter supports its **Unigram** profile.
SentencePiece BPE, stochastic segmentation and arbitrary normalization models
remain outside this implementation.

The runnable profiles are `bert-ascii-wordpiece-v1`, `t5-ascii-unigram-v1`
and `jinja-chat-subset-v1`. All product logic is Coax in
[tokenizer-families.coax](../lib/tokenizer-families.coax) and
[chat-template.coax](../lib/chat-template.coax). No Python library is required
at runtime, and no weights or device are opened.

## WordPiece and SentencePiece Unigram

WordPiece lowercases and cleans ASCII with mappings captured from the pinned
BERT normalizer, separates punctuation, then finds the longest vocabulary
piece at each position. Subsequent pieces carry `##`. A word over 100
characters, or any failed suffix, becomes one unknown ID: a partial success
never leaks into the resulting sequence.

<!-- source: psionic/lib/tokenizer-families.coax wordpiece -->
```jai
tf_wordpiece :: (v:[]TfPiece,input:string,unknown:int)->TfResult #export_c {
 if !tf_ascii_size(input.count) {return TfResult.{status=TfStatus.size};}if !tf_vocab(v) || unknown<0 || unknown>65535 {return TfResult.{status=TfStatus.vocabulary};}
 clean:[256]u8;n:=0;for c:input {if c>=128 {return TfResult.{status=TfStatus.unsupported};}mapped:=TF_BERT_ASCII[c];if mapped==-1 {continue;}clean[n]=cast(u8)mapped;n+=1;}
 r:=TfResult.{};at:=0;
 while at<n {if tf_space(clean[at]) {at+=1;continue;}end:=at+1;if !tf_punct(clean[at]) {while end<n && !tf_space(clean[end]) && !tf_punct(clean[end]) {end+=1;}}
  start:=r.written;pos:=at;failed:=end-at>100;
  while pos<end && !failed {candidate:=end;found:int;for back:0..<end-pos {candidate=end-back;query:[258]u8;k:=0;if pos>at {query[0]='#';query[1]='#';k=2;}for j:pos..<candidate {query[k]=clean[j];k+=1;}found=tf_find(v,cast(string)query[0..<k]);if found>=0 {break;}}
   if found<0 {failed=true;}else {r.ids[r.written]=v[found].id;r.written+=1;pos=candidate;}
  }
  if failed {r.written=start;r.ids[r.written]=unknown;r.written+=1;}at=end;
 }
 return r;
}
```

Unigram instead maximizes the sum of piece scores using dynamic programming.
Equal scores keep the first path reached by increasing start and end offsets.
Unknown edges consume one UTF-8 scalar with score `minimum vocabulary score - 10`;
consecutive unknown IDs fuse. The result includes the winning score, before
unknown fusion. This is a segmentation score, not a normalized language-model
likelihood. The core admits normalized UTF-8; the T5 wrapper accepts ASCII,
uses its pinned normalization mapping, collapses whitespace, trims ends and
inserts U+2581 before each word. The original T5 vocabulary has no piece with
an internal or repeated U+2581, so pieces cannot span those word boundaries.

<!-- source: psionic/lib/tokenizer-families.coax unigram -->
```jai
tf_unigram :: (v:[]TfPiece,input:string,unknown:int)->TfResult #export_c {
 if !tf_text_size(input.count) {return TfResult.{status=TfStatus.size};}if !tf_utf8(input) {return TfResult.{status=TfStatus.unsupported};}if !tf_vocab(v) || unknown<0 || unknown>65535 {return TfResult.{status=TfStatus.vocabulary};}
 scores:[1025]f64;previous:[1025]int;tokens:[1025]int;seen:[1025]bool;seen[0]=true;minimum:=v[tf_minimum(v)].score;
 for at:0..<input.count {if !seen[at] {continue;}
  for end:at+1..<input.count+1 {found:=tf_find(v,input[at..<end]);if found>=0 {score:=scores[at]+v[found].score;if !seen[end] || score>scores[end] {seen[end]=true;scores[end]=score;previous[end]=at;tokens[end]=v[found].id;}}}
  {end:=at+1;while end<input.count && (input[end]&192)==128 {end+=1;}score:=scores[at]+minimum-10;if !seen[end] || score>scores[end] {seen[end]=true;scores[end]=score;previous[end]=at;tokens[end]=unknown;}}
 }
 reverse:[1024]int;n:=0;at:=input.count;while at>0 {id:=tokens[at];if n==0 || id!=unknown || reverse[n-1]!=unknown {reverse[n]=id;n+=1;}at=previous[at];}
 r:=TfResult.{written=n,score=scores[input.count]};for i:0..<n {r.ids[i]=reverse[n-1-i];}return r;
}
```

The wrapper refuses non-ASCII source text: T5's complete precompiled Unicode
normalization map and BERT's Unicode case/accent/CJK handling are not implied
by this profile. Byte BPE and chapter 57's original DeepSeek text profile remain
available for their own Unicode contracts. These family calls encode ordinary
text without automatic added-token recognition or BOS/EOS postprocessing.
They do not promise the full upstream tokenizer pipeline for special-token
spellings such as `[MASK]`.

Vocabulary entries are caller-owned, immutable, unique and sorted by UTF-8
bytes. Admission checks 1–65,536 entries, each a nonempty UTF-8 piece of at most
256 bytes, IDs in 0–65,535, finite scores in [-1000,1000], and at most 2 MiB
aggregate piece bytes. Wrappers admit 256 source bytes; the Unigram core admits
1,024 normalized bytes. Results own room for 1,024 IDs. Failure returns a status
with zero count and payload. C callers retain valid view, pointer and lifetime
obligations; no host arithmetic implements tokenization.

## Interpret a bounded template

The template is parsed completely before rendering, including inactive
branches. The compiler produces a bounded instruction sequence and a halt
sentinel. Message content is emitted as data and is never parsed again.

<!-- source: psionic/lib/chat-template.coax compile -->
```jai
jt_compile :: (s:string)->JtProgram {
 if s.count>4096 {return JtProgram.{status=TfStatus.size};}p:=JtProgram.{};at:=0;loop:=-1;depth:=0;otherwise:[8]bool;
 while at<s.count {
  if jt_starts(s,at,"{{") || jt_starts(s,at,"{%") {expression:=s[at+1]=='{';close:=ifx expression then "}}" else "%}";begin:=at+2;end:=begin;while end<s.count && !jt_starts(s,end,close) {end+=1;}if end==s.count {return JtProgram.{status=TfStatus.unsupported};}body:=jt_trim(s[begin..<end]);at=end+2;
   if expression {p=jt_expression(p,body,loop>=0);}else {
    if body=="for message in messages" {if loop>=0 || depth!=0 {return JtProgram.{status=TfStatus.unsupported};}loop=p.count;p=jt_push(p,5,"",p.count);}
    else if body=="endfor" {if loop<0 || depth!=0 {return JtProgram.{status=TfStatus.unsupported};}p.ops[loop].target=p.count+1;p=jt_push(p,6,"",loop+1);loop=-1;}
    else if jt_starts(body,0,"if ") {condition:=jt_condition(jt_trim(body[3..<body.count]));if condition==0 || (condition!=4 && loop<0) {return JtProgram.{status=TfStatus.unsupported};}if depth==8 {return JtProgram.{status=TfStatus.size};}otherwise[depth]=false;depth+=1;p=jt_push(p,7,"",condition);}
    else if jt_starts(body,0,"elif ") {condition:=jt_condition(jt_trim(body[5..<body.count]));if depth==0 || condition==0 || (condition!=4 && loop<0) {return JtProgram.{status=TfStatus.unsupported};}if otherwise[depth-1] {return JtProgram.{status=TfStatus.unsupported};}p=jt_push(p,8,"",condition);}
    else if body=="else" {if depth==0 {return JtProgram.{status=TfStatus.unsupported};}if otherwise[depth-1] {return JtProgram.{status=TfStatus.unsupported};}otherwise[depth-1]=true;p=jt_push(p,9,"",0);}
    else if body=="endif" {if depth==0 {return JtProgram.{status=TfStatus.unsupported};}depth-=1;p=jt_push(p,10,"",0);}
    else {return JtProgram.{status=TfStatus.unsupported};}
    if jt_starts(s,at,"\r\n") {at+=2;}else if jt_starts(s,at,"\n") {at+=1;}
   }
  }else {begin:=at;while at<s.count && !jt_starts(s,at,"{{") && !jt_starts(s,at,"{%") {at+=1;}p=jt_push(p,0,jt_raw(s,begin,at,jt_starts(s,at,"{%")),0);}
  if p.status!=TfStatus.ok {return p;}
 }
 if loop>=0 || depth!=0 {return JtProgram.{status=TfStatus.unsupported};}p.ops[p.count].kind=11;return p;
}
```

The supported subset consists of literal text, quoted strings with n/r/t,
quote and backslash escapes, `+` concatenation, `bos_token`, `eos_token`,
`message['role']`, `message['content']`, sequential `for message in messages`
loops, `if`/`elif`/`else`/`endif`, comparisons of role with user/system/assistant,
`add_generation_prompt`, and `loop.last and add_generation_prompt`.
Block indentation and the following LF/CRLF are stripped as in the reference's
`lstrip_blocks=True, trim_blocks=True` environment. Quoted expression strings
must not contain a template closing delimiter. Filters, calls, assignments,
arbitrary expressions, whitespace-control hyphens and nested loops refuse.
A reference to message or loop state outside a loop also refuses.

Limits are 4,096 template bytes, 256 instructions, eight nested conditions,
16 messages, 16-byte roles, 1,024-byte contents, 64-byte BOS/EOS strings and
8,192 output bytes. Results own their bytes; any refusal returns zero output.
The renderer produces template text. Typed control-token insertion and stream
protocols belong to chapter 57's boundary-aware adapters.

## Original data and an independent implementation

[source.json](../fixtures/tokenizer-families/source.json) pins exact revisions,
URLs, lengths, SHA-256 identities and Apache-2.0 license declarations for
`google-bert/bert-base-uncased`, `google-t5/t5-small` and
`TinyLlama/TinyLlama-1.1B-Chat-v1.0`. Complete tokenizer JSON and the real
TinyLlama chat template are retained as DATA. No downloaded code runs.
The capture tool uses installed **tokenizers 0.22.2** to record expected IDs
from the full original vocabularies and **Jinja2 3.1.6** for expected rendering.
The normal gate is offline and stdlib-only: it verifies recorded identities
and constructs native expectations, without importing those packages.

The native oracle checks 290 tokenizer cases, including every ASCII control
between letters, unknown words, punctuation, whitespace, digits, long words
and empty text. Twelve Jinja vectors include the original TinyLlama template,
empty conversations, role branches, literal template syntax inside content,
and generation both enabled and disabled. Unit checks add malformed grammar,
UTF-8 refusals, exact limits, tie scores and unknown-run fusion.

[The example](../examples/tokenizer-families.coax) uses deliberately small,
owned vocabularies so longest-match and best-score choices are easy to see.

<!-- source: psionic/examples/tokenizer-families.coax example -->
```jai
main :: () {
    words:=[3]TfPiece.[TfPiece.{text="##ing",id=2},TfPiece.{text="play",id=1},TfPiece.{text="playing",id=3}];
    w:=tf_wordpiece(words[..],"PLAYING",0);#assert w.status==TfStatus.ok;
    print("WordPiece: longest piece ID %\n",w.ids[0]);
    pieces:=[4]TfPiece.[TfPiece.{text="a",id=1,score=-1},TfPiece.{text="ab",id=2,score=-10},TfPiece.{text="b",id=3,score=-1},TfPiece.{text="▁",id=4,score=-1}];
    u:=tf_sentencepiece(pieces[..]," ab ",0);#assert u.status==TfStatus.ok;
    print("SentencePiece Unigram: % % %; best score %\n",u.ids[0],u.ids[1],u.ids[2],u.score);
    messages:=[1]JtMessage.[JtMessage.{role="user",content="{{ stays text }}"}];
    template:="{% for message in messages %}{{ message['role'] + ': ' + message['content'] + eos_token }}{% endfor %}{% if add_generation_prompt %}{{ 'assistant:' }}{% endif %}";
    chat:=jt_render(template,messages[..],true,"","</s>");#assert chat.status==TfStatus.ok;
    print("%\n",cast(string)chat.bytes[0..<chat.written]);
}
```

```shell
coax/bin/coax run psionic/examples/tokenizer-families.coax
psionic/check.sh --only tokenizer-families --jobs 8
```

[The exact transcript](../fixtures/tokenizer-families.out) and
[verification record](../../docs/verification/psionic-ch36-2026-10-30.md)
name the passing debug/release, mutation, upstream parity and sanitizer gates.
