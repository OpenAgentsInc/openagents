# 16b. The protobuf wire

[Chapter 16](ch016-serve-systemone.md)'s server speaks the NIP-DEC JSON
contract `openagents.systemone.v1`. OpenAgents' protocol is defined in
`proto/`, and its specification already sketches `decision.v1` messages for
this endpoint, but no schema existed. This chapter adds one,
`openagents.decision.v1alpha1`, and lets the same server take and give it.

```shell
PROTO_PYTHON=/path/to/python-with-protobuf-5.29.3 psionic/tests/check_serve_proto.sh
```

The schema is [systemone.proto](../../proto/openagents/decision/v1alpha1/systemone.proto),
its descriptor `proto/descriptors/systemone.pb` (profile `systemone`, built by
`scripts/proto.sh generate` with the pinned protoc 29.3), and its notes are in
its [README](../../proto/openagents/decision/v1alpha1/README.md). The Coax codecs come
straight from the descriptor with `#import_proto`, so there is no hand-written
codec.

## One door, two encodings

The JSON contract stays the definition. A protobuf request is never admitted
on its own terms. It is transcoded to the NIP-DEC body it means, and that body
goes through chapter 16's decoder, bounds and refusals. The protobuf door can
therefore never admit what the JSON door would refuse. A missing model, for
example, gets exactly JSON's `invalid_request`.

<!-- source: psionic/lib/serve-proto.coax request -->
```jai
// One EntryType as JSON. Numbers must be integers within 2^53, the same set
// chapter 5's text format accepts; anything else is unsupported_input.
sp_entry :: (e:So_EntryType,out:[]u8,at:int,depth:int)->SpOut {
    if depth>SP_DEPTH_MAX {return SpOut.{code=SvCode.invalid_request};}
    which:=So_EntryType_case_value(e);if which.error!=0 {return SpOut.{code=SvCode.malformed};}
    n:=-1;
    if which.value==0 || which.value==1 {n=json_raw(out,at,"null");}
    else if which.value==2 {n=json_write_string(out,at,So_EntryType_get_text(e).value);}
    else if which.value==5 {
        v:=So_EntryType_get_number(e).value;
        if !(abs(v)<=f64(SP_INTEGER_MAX)) || v!=f64(cast(int)v) {return SpOut.{code=SvCode.unsupported_input};}
        n=json_write_int(out,at,cast(int)v);
    }
    else if which.value==6 {n=json_raw(out,at,"false");if So_EntryType_get_boolean(e).value {n=json_raw(out,at,"true");}}
    else if which.value==3 {
        n=json_raw(out,at,"{");first:=true;
        cur:=So_EntryObject_iter_fields(So_EntryType_get_object(e).value);
        while !cur.done {
            if !first {n=json_raw(out,n,",");}first=false;
            n=json_write_string(out,n,So_EntryField_get_key(cur.value).value);n=json_raw(out,n,":");
            if n<0 {return SpOut.{code=SvCode.limit_exceeded};}
            inner:=sp_entry(So_EntryField_get_value(cur.value).value,out,n,depth+1);if inner.code!=SvCode.none {return inner;}
            n=inner.at;cur=So_EntryObject_next_fields(cur);
        }
        if cur.error!=0 {return SpOut.{code=SvCode.malformed};}
        n=json_raw(out,n,"}");
    }
    else {
        n=json_raw(out,at,"[");first:=true;
        cur:=So_EntryArray_iter_items(So_EntryType_get_array(e).value);
        while !cur.done {
            if !first {n=json_raw(out,n,",");}first=false;
            if n<0 {return SpOut.{code=SvCode.limit_exceeded};}
            inner:=sp_entry(cur.value,out,n,depth+1);if inner.code!=SvCode.none {return inner;}
            n=inner.at;cur=So_EntryArray_next_items(cur);
        }
        if cur.error!=0 {return SpOut.{code=SvCode.malformed};}
        n=json_raw(out,n,"]");
    }
    if n<0 {return SpOut.{code=SvCode.limit_exceeded};}
    return SpOut.{at=n};
}
// A protobuf SystemOneRequest as the NIP-DEC body chapter 16 decodes. Absent
// fields are left out, so the JSON decoder refuses exactly what it would refuse.
sp_request_json :: (wire:[]u8,out:[]u8)->SpOut #export_c {
    if wire.count>SV_BODY_MAX {return SpOut.{code=SvCode.limit_exceeded};}
    r:=So_SystemOneRequest_decode(wire);if r.error!=0 {return SpOut.{code=SvCode.malformed};}
    n:=json_raw(out,0,"{");
    model:=So_SystemOneRequest_get_model(r);
    if model.present {n=json_raw(out,n,"\"model\":");n=json_write_string(out,n,model.value);n=json_raw(out,n,",");}
    state:=So_SystemOneRequest_get_state(r);
    if state.present {
        n=json_raw(out,n,"\"state\":");if n<0 {return SpOut.{code=SvCode.limit_exceeded};}
        s:=sp_entry(state.value,out,n,0);if s.code!=SvCode.none {return s;}
        n=json_raw(out,s.at,",");
    }
    n=json_raw(out,n,"\"questions\":{");first:=true;
    cur:=So_SystemOneRequest_iter_questions(r);
    while !cur.done {
        if !first {n=json_raw(out,n,",");}first=false;
        n=json_write_string(out,n,So_NamedQuestion_get_id(cur.value).value);n=json_raw(out,n,":{");
        q:=So_NamedQuestion_get_question(cur.value).value;kind:=So_Question_get_type(q).value;
        if kind==So_QuestionType_QUESTION_TYPE_NOUL {n=json_raw(out,n,"\"type\":\"noul\"");}
        else if kind==So_QuestionType_QUESTION_TYPE_CHOICE {n=json_raw(out,n,"\"type\":\"choice\"");}
        else if kind==So_QuestionType_QUESTION_TYPE_SCORE {n=json_raw(out,n,"\"type\":\"score\"");}
        else {return SpOut.{code=SvCode.invalid_request};}
        instructions:=So_Question_get_instructions(q);
        if instructions.present {
            n=json_raw(out,n,",\"instructions\":");if n<0 {return SpOut.{code=SvCode.limit_exceeded};}
            v:=sp_entry(instructions.value,out,n,0);if v.code!=SvCode.none {return v;}n=v.at;
        }
        criteria:=So_Question_get_criteria(q);
        if criteria.present {
            n=json_raw(out,n,",\"criteria\":");if n<0 {return SpOut.{code=SvCode.limit_exceeded};}
            v:=sp_entry(criteria.value,out,n,0);if v.code!=SvCode.none {return v;}n=v.at;
        }
        n=json_raw(out,n,"}");cur=So_SystemOneRequest_next_questions(cur);
    }
    if cur.error!=0 {return SpOut.{code=SvCode.malformed};}
    n=json_raw(out,n,"}}");
    if n<0 {return SpOut.{code=SvCode.limit_exceeded};}
    return SpOut.{at=n};
}
```

`EntryType` numbers must be integers within 2⁵³. That isn't a new limit:
chapter 5's text format already refuses fractional JSON numbers as
unsupported, so a fraction here gets the same `unsupported_input`.

## Reading the head

The server picks the encodings from `Content-Type` and `Accept`, read from the
head only. A body that happens to contain those words changes nothing.

<!-- source: psionic/lib/serve-proto.coax head -->
```jai
// Content-Type and Accept, read from the head only, case-insensitively. A
// media type with parameters is not this one.
sp_flags :: (head:string)->SpFlags #export_c {
    f:SpFlags;end:=sv_index(head,"\r\n\r\n");if end<0 {return f;}
    at:=sv_index(head,"\r\n")+2;
    while at<end {
        next:=sv_find(head,at,"\r\n");line:=head[at..<next];at=next+2;
        colon:=sv_index(line,":");
        if colon>0 {
            name:=line[0..<colon];value:=sv_trim(line[colon+1..<line.count]);
            if sv_same(name,"content-type") && sv_same(value,SP_MEDIA) {f.proto_in=true;}
            if sv_same(name,"accept") && sv_same(value,SP_MEDIA) {f.proto_out=true;}
        }
    }
    return f;
}
```

`sp_bytes` takes the host's reads in place of `sv_bytes`. Until a protobuf body
is complete, and for every JSON request, it is `sv_bytes` unchanged. A
complete protobuf body is rewritten as a JSON request on the same route, so
deadlines, `Expect: 100-continue`, admission and the slot's next phase are
chapter 16's code.

<!-- source: psionic/lib/serve-proto.coax bytes -->
```jai
// The host hands every read to sp_bytes instead of sv_bytes. A JSON request,
// an incomplete one, or one refused at the head goes to sv_bytes unchanged.
// A complete protobuf body is transcoded into scratch as a JSON request on the
// same route and admitted by sv_bytes, so deadlines, Expect: 100-continue,
// admission and the slot's next phase are chapter 16's. The decoded request
// borrows from scratch, which must outlive the decision. raw is the same
// bytes as text, as the host holds them.
sp_bytes :: (slot:SvSlot,limits:SvLimits,engine:SvEngine,pools:SvPools,text:string,raw:[]u8,scratch:[]u8,now:int)->SpStep #export_c {
    flags:=sp_flags(text);
    if raw.count!=text.count {return SpStep.{step=sv_refused(slot,SvCode.internal)};}
    h:=sv_head(text);
    if !flags.proto_in || h.status!=SvHeadStatus.ready || text.count<h.head+h.body || slot.phase!=SvPhase.reading || now>=slot.deadline {
        return SpStep.{step=sv_bytes(slot,limits,engine,pools,text,now),proto_out=flags.proto_out};
    }
    prefix:="POST /v1/systemone HTTP/1.1\r\nContent-Length: ";
    room:=scratch.count-prefix.count-32;if room<0 {return SpStep.{step=sv_refused(slot,SvCode.internal)};}
    body:=scratch[prefix.count+32..<scratch.count];
    t:=sp_request_json(raw[h.head..<h.head+h.body],body);
    if t.code!=SvCode.none {return SpStep.{step=sv_refused(slot,t.code),proto_out=flags.proto_out};}
    n:=json_raw(scratch,0,prefix);n=json_write_int(scratch,n,t.at);n=json_raw(scratch,n,"\r\n\r\n");
    for k:0..<t.at {scratch[n+k]=body[k];}
    return SpStep.{step=sv_bytes(slot,limits,engine,pools,cast(string)scratch[0..<n+t.at],now),proto_out=flags.proto_out};
}
```

## The answer

A protobuf answer re-encodes the JSON answer the engine already rendered, so
the choice, the score, the selected level and the rounding are the JSON
answer's. Its probabilities have at most nine decimals, and digits ÷ 10ᵏ is
correctly rounded in binary64, so they parse back exactly.

<!-- source: psionic/lib/serve-proto.coax build -->
```jai
// Space for building the answer: a message is rewritten by every setter, so
// each container alternates between two buffers and the finished message is
// copied once into the arena.
SpArena :: struct {bytes:[]u8;used:int;}
sp_take :: (a:[]SpArena,count:int)->[]u8 {
    if a[0].used+count>a[0].bytes.count {return a[0].bytes[0..<0];}
    s:=a[0].bytes[a[0].used..<a[0].used+count];a[0].used+=count;return s;
}
// A decimal from the answer body, as written there: an integer, or digits with
// at most nine places. digits / 10^k is the correctly rounded binary64 value.
sp_decimal :: (text:string)->So__pb_F64 {
    r:So__pb_F64;if text.count<1 || text.count>32 {return r;}
    at:=0;negative:=false;if text[0]=='-' {negative=true;at=1;}
    digits:=0;places:=-1;
    while at<text.count {
        c:=text[at];
        if c=='.' {if places>=0 {return r;}places=0;}
        else if c>='0' && c<='9' {digits=digits*10+cast(int)(c-'0');if places>=0 {places+=1;}}
        else {return r;}
        at+=1;
    }
    if places==0 || places>9 || digits>SP_INTEGER_MAX {return r;}
    scale:=1;for k:0..<places {scale*=10;}
    v:=f64(digits)/f64(scale);if places<0 {v=f64(digits);}
    if negative {v=-v;}
    return So__pb_F64.{value=v,present=true};
}
// One JSON value (the current token in j) as an EntryType, built in space
// taken from the arena; depth bounds the recursion.
sp_json_entry :: (j:[]Json,a:[]SpArena,depth:int)->So_EntryType {
    bad:=So_EntryType.{error=1};if depth>SP_DEPTH_MAX {return bad;}
    t:=j[0].tok;
    if t==JSON_NULL {return So_EntryType_set_null(So_EntryType_new(),So_Null_new(),sp_take(a,8));}
    if t==JSON_TRUE || t==JSON_FALSE {return So_EntryType_set_boolean(So_EntryType_new(),t==JSON_TRUE,sp_take(a,8));}
    if t==JSON_NUMBER {
        v:=sp_decimal(json_text(j));if !v.present {return bad;}
        return So_EntryType_set_number(So_EntryType_new(),v.value,sp_take(a,16));
    }
    if t==JSON_STRING {
        raw:=json_text(j);text:=sp_take(a,raw.count);n:=json_unescape(raw,text);if n<0 {return bad;}
        return So_EntryType_set_text(So_EntryType_new(),cast(string)text[0..<n],sp_take(a,n+16));
    }
    if t==JSON_ARRAY {
        ping:=sp_take(a,SP_LEVEL_BYTES);pong:=sp_take(a,SP_LEVEL_BYTES);arr:=So_EntryArray_new();flip:=false;
        while true {
            t=json_next(j);if t==JSON_ARRAY_END {break;}
            item:=sp_json_entry(j,a,depth+1);if item.error!=0 {return bad;}
            if flip {arr=So_EntryArray_append_items(arr,item,pong);} else {arr=So_EntryArray_append_items(arr,item,ping);}
            flip=!flip;if arr.error!=0 {return bad;}
        }
        return So_EntryType_set_array(So_EntryType_new(),arr,sp_take(a,arr.wire.count+16));
    }
    if t!=JSON_OBJECT {return bad;}
    ping:=sp_take(a,SP_LEVEL_BYTES);pong:=sp_take(a,SP_LEVEL_BYTES);obj:=So_EntryObject_new();flip:=false;
    while true {
        t=json_next(j);if t==JSON_OBJECT_END {break;}
        raw:=json_text(j);key:=sp_take(a,raw.count);kn:=json_unescape(raw,key);if kn<0 {return bad;}
        t=json_next(j);value:=sp_json_entry(j,a,depth+1);if value.error!=0 {return bad;}
        field:=So_EntryField_set_key(So_EntryField_new(),cast(string)key[0..<kn],sp_take(a,kn+16));
        field=So_EntryField_set_value(field,value,sp_take(a,field.wire.count+value.wire.count+16));
        if flip {obj=So_EntryObject_append_fields(obj,field,pong);} else {obj=So_EntryObject_append_fields(obj,field,ping);}
        flip=!flip;if obj.error!=0 {return bad;}
    }
    return So_EntryType_set_object(So_EntryType_new(),obj,sp_take(a,obj.wire.count+16));
}
```

<!-- source: psionic/lib/serve-proto.coax answer -->
```jai
// Numbered or keyed probabilities, in the body's order.
sp_probabilities :: (j:[]Json,a:[]SpArena,choice:So_ChoiceAnswer,score:So_ScoreAnswer,is_choice:bool)->SpPair {
    r:=SpPair.{choice=choice,score=score};ping:=sp_take(a,65536);pong:=sp_take(a,65536);flip:=false;
    if json_next(j)!=JSON_OBJECT {r.ok=false;return r;}
    while true {
        t:=json_next(j);if t==JSON_OBJECT_END {break;}
        raw:=json_text(j);key:=sp_take(a,raw.count);kn:=json_unescape(raw,key);
        t=json_next(j);v:=sp_decimal(json_text(j));if kn<0 || t!=JSON_NUMBER || !v.present {r.ok=false;return r;}
        p:=So_Probability_set_id(So_Probability_new(),cast(string)key[0..<kn],sp_take(a,kn+16));
        p=So_Probability_set_probability(p,v.value,sp_take(a,p.wire.count+16));
        buffer:=ping;if flip {buffer=pong;}flip=!flip;
        if is_choice {r.choice=So_ChoiceAnswer_append_probabilities(r.choice,p,buffer);}
        else {r.score=So_ScoreAnswer_append_probabilities(r.score,p,buffer);}
        if r.choice.error!=0 || r.score.error!=0 {r.ok=false;return r;}
    }
    r.ok=true;return r;
}
SpPair :: struct {ok:bool;choice:So_ChoiceAnswer;score:So_ScoreAnswer;}
// One answer object from the body (its opening token just read) as an Answer.
sp_one_answer :: (j:[]Json,a:[]SpArena)->So_Answer {
    bad:=So_Answer.{error=1};kind:="";noul:So__pb_F64;selected:="";has_selected:=false;
    choice:=So_ChoiceAnswer_new();score:=So_ScoreAnswer_new();
    while true {
        t:=json_next(j);if t==JSON_OBJECT_END {break;}
        key:=json_text(j);
        if json_is(key,"probabilities") {
            r:=sp_probabilities(j,a,choice,score,kind=="choice");if !r.ok {return bad;}choice=r.choice;score=r.score;
            continue;
        }
        if json_is(key,"legend") {
            if json_next(j)!=JSON_OBJECT {return bad;}
            ping:=sp_take(a,65536);pong:=sp_take(a,65536);flip:=false;
            while true {
                t=json_next(j);if t==JSON_OBJECT_END {break;}
                raw:=json_text(j);name:=sp_take(a,raw.count);kn:=json_unescape(raw,name);
                t=json_next(j);value:=sp_json_entry(j,a,0);if kn<0 || value.error!=0 {return bad;}
                f:=So_EntryField_set_key(So_EntryField_new(),cast(string)name[0..<kn],sp_take(a,kn+16));
                f=So_EntryField_set_value(f,value,sp_take(a,f.wire.count+value.wire.count+16));
                buffer:=ping;if flip {buffer=pong;}flip=!flip;
                score=So_ScoreAnswer_append_legend(score,f,buffer);if score.error!=0 {return bad;}
            }
            continue;
        }
        t=json_next(j);text:=json_text(j);
        if json_is(key,"type") {kind=text;}
        else if json_is(key,"noul") {noul=sp_decimal(text);if !noul.present {return bad;}}
        else if json_is(key,"choice") {choice=So_ChoiceAnswer_set_choice(choice,text,sp_take(a,choice.wire.count+text.count+16));}
        else if json_is(key,"selected") {selected=text;has_selected=true;}
        else if json_is(key,"confidence") || json_is(key,"score") {
            v:=sp_decimal(text);if !v.present {return bad;}
            if json_is(key,"score") {score=So_ScoreAnswer_set_score(score,v.value,sp_take(a,score.wire.count+16));}
            else if kind=="choice" {choice=So_ChoiceAnswer_set_confidence(choice,v.value,sp_take(a,choice.wire.count+16));}
            else {score=So_ScoreAnswer_set_confidence(score,v.value,sp_take(a,score.wire.count+16));}
        }
        else {return bad;}
    }
    answer:=So_Answer_new();
    if kind=="noul" {
        n:=So_NoulAnswer_set_noul(So_NoulAnswer_new(),noul.value,sp_take(a,16));
        answer=So_Answer_set_noul(answer,n,sp_take(a,n.wire.count+16));
    } else if kind=="choice" {answer=So_Answer_set_choice(answer,choice,sp_take(a,choice.wire.count+16));}
    else if kind=="score" {answer=So_Answer_set_score(answer,score,sp_take(a,score.wire.count+16));}
    else {return bad;}
    if has_selected {answer=So_Answer_set_selected(answer,selected,sp_take(a,answer.wire.count+selected.count+16));}
    return answer;
}
// The JSON 200 body as a SystemOneResponse, framed as a complete HTTP response
// in out. The arena holds every intermediate message.
sp_answer :: (body:string,out:[]u8,arena:[]u8)->SpBody #export_c {
    bad:=SpBody.{code=SvCode.internal};a:=[1]SpArena.[SpArena.{bytes=arena}];
    j:[1]Json;json_start_bounded(j[..],body,body.count,body.count);
    if json_next(j[..])!=JSON_OBJECT {return bad;}
    // r always lives in one of two buffers and is rewritten into the other: a
    // setter may not write over the message it reads (the codec refuses that).
    r:=So_SystemOneResponse_new();pair:=[2][]u8.[sp_take(a[..],SV_BODY_MAX),sp_take(a[..],SV_BODY_MAX)];held:=1;
    if pair[0].count==0 || pair[1].count==0 {return bad;}
    while true {
        t:=json_next(j[..]);if t==JSON_OBJECT_END {break;}
        key:=json_text(j[..]);t=json_next(j[..]);
        // Every write below goes to the buffer r is not in; then r is there.
        buffer:=pair[1-held];
        if json_is(key,"model") {r=So_SystemOneResponse_set_model(r,json_text(j[..]),buffer);}
        else if json_is(key,"answers") {
            if t!=JSON_OBJECT {return bad;}
            while true {
                t=json_next(j[..]);if t==JSON_OBJECT_END {break;}
                raw:=json_text(j[..]);id:=sp_take(a[..],raw.count);n:=json_unescape(raw,id);
                if json_next(j[..])!=JSON_OBJECT || n<0 {return bad;}
                answer:=sp_one_answer(j[..],a[..]);if answer.error!=0 {return bad;}
                named:=So_NamedAnswer_set_id(So_NamedAnswer_new(),cast(string)id[0..<n],sp_take(a[..],n+16));
                named=So_NamedAnswer_set_answer(named,answer,sp_take(a[..],named.wire.count+answer.wire.count+16));
                r=So_SystemOneResponse_append_answers(r,named,pair[1-held]);if r.error!=0 {return bad;}
                held=1-held;
            }
            held=1-held;  // undone by the swap below: this key wrote only through the loop
        }
        else if json_is(key,"usage") {if t!=JSON_NULL {return bad;}r=So_SystemOneResponse_set_usage_null(r,So_Null_new(),buffer);}
        else if json_is(key,"service") {
            if t!=JSON_OBJECT {return bad;}service:=So_ServiceIdentity_new();
            while true {
                t=json_next(j[..]);if t==JSON_OBJECT_END {break;}
                name:=json_text(j[..]);t=json_next(j[..]);value:=json_text(j[..]);w:=sp_take(a[..],service.wire.count+value.count+16);
                if json_is(name,"door") {service=So_ServiceIdentity_set_door(service,value,w);}
                else if json_is(name,"version") {service=So_ServiceIdentity_set_version(service,value,w);}
                else if json_is(name,"identity") {service=So_ServiceIdentity_set_identity(service,value,w);}
                else {return bad;}
            }
            r=So_SystemOneResponse_set_service(r,service,buffer);
        }
        else if json_is(key,"latency_ms") {
            v:=json_int(json_text(j[..]));if !v.ok || v.value<0 {return bad;}
            r=So_SystemOneResponse_set_latency_ms(r,cast(u64)v.value,buffer);
        }
        else {return bad;}
        if r.error!=0 {return bad;}
        held=1-held;
    }
    encoded:=So_SystemOneResponse_encode(r,sp_take(a[..],r.wire.count+16));if encoded.error!=0 {return bad;}
    n:=json_raw(out,0,"HTTP/1.1 200 OK\r\nContent-Type: ");n=json_raw(out,n,SP_MEDIA);
    n=json_raw(out,n,"\r\nContent-Length: ");n=json_write_int(out,n,encoded.wire.count);n=json_raw(out,n,"\r\nConnection: close\r\n\r\n");
    head:=n;n=json_raw(out,n,cast(string)encoded.wire);if n<0 {return bad;}
    return SpBody.{from=0,to=n};
}
```

Each message the generated setters write is rewritten whole, and a setter may
not write over the message it reads. The first version alternated two buffers
and lost track of which one held the response; the codec refused the aliasing
write. The fix is to record which buffer holds it and always write to the
other.

## How it is checked

- **An independent encoder:** `psionic/tests/serve_proto_oracle.py` uses the
  official Python protobuf runtime and the committed descriptor, not the Coax
  codecs. It produces:
  - six requests with the exact NIP-DEC JSON each must become, including
    nested state, quotes and backslashes, ordered criteria, and 2⁵³ integers;
  - three refusals: a fraction, an integer past 2⁵³, and an unspecified
    question type;
  - a complete protobuf HTTP request, which is driven through the toy engine's
    slot: partial read, decision, timeout, a fraction refused, and a JSON
    request on the old path.

  These checks live in `serve-proto.coax` between generated markers, and the
  gate fails if they're stale.
- **The official runtime as decoder:** the native harness
  [serve_proto.c](../tests/serve_proto.c) encodes two JSON answers, a toy
  answer with choice, noul and score and an Elephant-shaped one. The same
  oracle decodes them and compares every value with the JSON, including
  ordered probabilities and nested legends. This runs plain and under
  ASan/UBSan, in debug and release.
- **No mutation run:** `serve-proto.coax` decides nothing, so it isn't
  `#authority`. Admission stays in `serve-request.coax`, and selection and
  rounding in `serve-engine.coax`.

The [verification record](../../docs/verification/psionic-ch16b-2026-10-29.md)
has the commands and results.

## Limits

- **Projected messages:** only the request and response. Results, status,
  cancel and receipts come when `decision.v1` is published with its common
  types.
- **Precision:** probabilities carry nine decimals, as the JSON answer does.
- **Refusals:** they stay JSON bodies in either encoding.
- **The compiler's source budget:** `serve-proto.coax` with `serve-run.coax`
  is at the compiler's 64-source-unit limit, so its vectors are generated into
  the file instead of a separate fixture.
