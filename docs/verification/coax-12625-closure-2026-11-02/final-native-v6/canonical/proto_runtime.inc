static constexpr char PROTO_CORE_SOURCE[] = R"COAX_CORE(// Shared protobuf wire core. All views borrow caller storage; no allocation.
PB_OK :: 0;
PB_WIRE :: 1;
PB_LIMIT :: 2;
PB_DUPLICATE :: 3;
PB_UTF8 :: 4;
PB_REQUIRED :: 5;
PB_ALIAS :: 6;
PB_VALUE :: 7;
PB_MAX_BYTES :: 1048576;
PB_MAX_FIELDS :: 524288;
PB_MAX_DEPTH :: 64;
PB_View :: struct { wire: [] u8; error: u32; }
PB_Check :: struct { error: u32; left: int; }
PB_Var :: struct { value:u64; next:int; error:u32; }
PB_Field :: struct { number:u32; wt:u8; raw:u64; payload:[] u8; start:int; next:int; error:u32; done:bool; }
PB_U32 :: struct { value:u32; present:bool; error:u32; }
PB_U64 :: struct { value:u64; present:bool; error:u32; }
PB_S32 :: struct { value:s32; present:bool; error:u32; }
PB_S64 :: struct { value:s64; present:bool; error:u32; }
PB_F32 :: struct { value:f32; present:bool; error:u32; }
PB_F64 :: struct { value:f64; present:bool; error:u32; }
PB_Bool :: struct { value:bool; present:bool; error:u32; }
PB_Bytes :: struct { value:[] u8; present:bool; error:u32; }
PB_String :: struct { value:string; present:bool; error:u32; }
// Count-only admission makes the native span boundary explicit and testable.
PB_small :: (count:int)->bool {return count>=0 && count<=PB_MAX_BYTES;}
// Check metadata before any generated decoder accesses its borrowed view.
PB_message_admitted :: (depth:int,count:int)->bool {return depth>=0 && depth<=PB_MAX_DEPTH && PB_small(count);}
PB_root_depth :: ()->int {return 0;}
PB_default_fields :: ()->int {return PB_MAX_FIELDS;}
PB_require_fields :: ()->bool {return true;}
PB_partial_fields :: ()->bool {return false;}
PB_cursor :: (count:int,at:int)->bool {return PB_small(count) && at>=0 && at<=count;}
PB_fits :: (total:int,capacity:int)->bool {return PB_small(total) && total<=capacity;}
PB_inputs :: (wire:int,capacity:int,data:int,text:int)->u32 {
  if !PB_small(wire) || capacity<0 || !PB_small(data) || !PB_small(text) {return PB_LIMIT;}
  return PB_OK;
}
PB_var :: (wire:[] u8,at:int)->PB_Var {
  pos:=at;value:u64=0;shift:int=0;
  while true {
    if pos<0 || pos>=wire.count {return PB_Var.{error=PB_WIRE};}
    b:=wire[pos];pos+=1;
    if shift==63 && b>1 {return PB_Var.{error=PB_WIRE};}
    value=value | (cast(u64)(b&127)<<shift);
    if b<128 {return PB_Var.{value=value,next=pos};}
    shift+=7;
  }
  return PB_Var.{}; // Unreachable: each tenth byte either terminates or overflows.
}
PB_scalar :: (wire:[] u8,at:int,wt:u8)->PB_Field {
  if !PB_cursor(wire.count,at) {return PB_Field.{error=PB_WIRE};}
  if wt==0 {
    v:=PB_var(wire,at);return PB_Field.{raw=v.value,start=at,next=v.next,error=v.error};
  }
  n:int=---;if wt==1 {n=8;} else if wt==5 {n=4;} else {return PB_Field.{error=PB_WIRE};}
  if n>wire.count-at {return PB_Field.{error=PB_WIRE};}
  value:u64=0;
  for i:0..<n {value=value | (cast(u64)wire[at+i]<<(i*8));}
  return PB_Field.{raw=value,start=at,next=at+n};
}
PB_scan :: (wire:[] u8,at:int)->PB_Field {
  if !PB_cursor(wire.count,at) {return PB_Field.{error=PB_LIMIT};}
  if at==wire.count {return PB_Field.{start=at,next=at,done=true};}
  key:=PB_var(wire,at);
  if key.error!=0 || (key.value>>3)>536870911 {return PB_Field.{error=PB_WIRE};}
  number:=cast(u32)(key.value>>3);if number==0 {return PB_Field.{error=PB_WIRE};}wt:=cast(u8)(key.value&7);
  if wt==2 {
    length:=PB_var(wire,key.next);
    if length.error!=0 || length.value>cast(u64)(wire.count-length.next) {return PB_Field.{error=PB_WIRE};}
    end:=length.next+cast(int)length.value;
    return PB_Field.{number=number,wt=wt,start=at,next=end,payload=wire[length.next..<end]};
  }
  f:=PB_scalar(wire,key.next,wt);f.number=number;f.wt=wt;f.start=at;return f;
}
// This searches to the first match, not the end of the message. Callers that
// need whole-message validity must validate first. An error before a match
// returns an empty field and reports no presence.
PB_find :: (wire:[] u8,number:u32)->PB_Field {
  pos:=0;
  while true {
    f:=PB_scan(wire,pos);
    if f.error!=0 {return PB_Field.{error=f.error,done=true};}
    if f.done {return PB_Field.{done=true};}
    if f.number==number {return f;}
    pos=f.next;
  }
  return PB_Field.{};
}
// Optional minimal-varint policy, used by closed resource readers. The normal
// protobuf decoder still accepts legal non-minimal encodings.
PB_minimal :: (wire:[] u8,f:PB_Field)->bool {
  key:=PB_var(wire,f.start);
  if key.error!=0 || PB_vsize(key.value)!=key.next-f.start {return false;}
  if f.wt==0 {return PB_vsize(f.raw)==f.next-key.next;}
  if f.wt==2 {length:=PB_var(wire,key.next);return length.error==0 && PB_vsize(length.value)==length.next-key.next;}
  return true;
}
PB_utf8 :: (wire:[] u8)->bool {
  at:=0;
  while at<wire.count {
    a:=wire[at];at+=1;if a<128 {continue;}
    needed:int=---;low:u8=128;high:u8=191;
    if a>=194 && a<=223 {needed=1;}
    else if a>=224 && a<=239 {needed=2;if a==224 {low=160;} if a==237 {high=159;}}
    else if a>=240 && a<=244 {needed=3;if a==240 {low=144;} if a==244 {high=143;}}
    else {return false;}
    if needed>wire.count-at || wire[at]<low || wire[at]>high {return false;}
    at+=1;
    for i:1..<needed {if wire[at]<128 || wire[at]>191 {return false;} at+=1;}
  }
  return true;
}
PB_no_nul :: (wire:[] u8)->bool {for b:wire {if b==0 {return false;}}return wire.count>0;}
PB_sint :: (raw:u64)->s64 {v:=cast(s64)(raw>>1);if (raw&1)!=0 {return -v-1;} return v;}
PB_zigzag :: (value:s64)->u64 {if value<0 {return cast(u64)(-(value+1))*2+1;} return cast(u64)value*2;}
PB_vsize :: (value:u64)->int {v:=value;n:=1;while v>=128 {v=v>>7;n+=1;} return n;}
PB_put :: (out:[] u8,at:int,value:u64)->int {
  v:=value;pos:=at;
  while v>=128 {out[pos]=cast,trunc(u8)(v&127)|128;pos+=1;v=v>>7;}
  out[pos]=cast(u8)v;return pos+1;
}
PB_rewrite_checked :: (wire:[] u8,removed_fields:[] u32,number:u32,wt:u8,raw:u64,data:[] u8,txt:string,is_text:bool,clear:bool,out:[] u8,overlap:bool)->PB_View {
  error:=PB_inputs(wire.count,out.count,data.count,txt.count);
  if error!=0 {return PB_View.{error=error};}
  if overlap {return PB_View.{error=PB_ALIAS};}
  total:=0;at:=0;
  while at<wire.count {
    f:=PB_scan(wire,at);if f.error!=0 {return PB_View.{error=f.error};}
    keep:=true;for id:removed_fields {if f.number==id {keep=false;}}
    if keep {total+=f.next-at;} at=f.next;
  }
  n:=data.count;if is_text {n=txt.count;}
  if !clear {
    total+=PB_vsize(cast(u64)number*8+cast(u64)wt);
    if wt==0 {total+=PB_vsize(raw);} else if wt==1 {total+=8;} else if wt==5 {total+=4;}
    else if wt==2 {total+=PB_vsize(cast(u64)n)+n;} else {return PB_View.{error=PB_WIRE};}
  }
  if !PB_fits(total,out.count) {return PB_View.{error=PB_LIMIT};}
  at=0;pos:=0;
  while at<wire.count {
    f:=PB_scan(wire,at);keep:=true;for id:removed_fields {if f.number==id {keep=false;}}
    if keep {for i:at..<f.next {out[pos]=wire[i];pos+=1;}} at=f.next;
  }
  if !clear {
    pos=PB_put(out,pos,cast(u64)number*8+cast(u64)wt);
    if wt==0 {pos=PB_put(out,pos,raw);}
    else if wt==2 {pos=PB_put(out,pos,cast(u64)n);for i:0..<n {if is_text {out[pos]=txt[i];} else {out[pos]=data[i];} pos+=1;}}
    else {count:=4;if wt==1 {count=8;} for i:0..<count {out[pos]=cast,trunc(u8)(raw>>(i*8));pos+=1;}}
  }
  return PB_View.{wire=out[0..<pos]};
}
PB_copy_checked :: (wire:[] u8,out:[] u8,overlap:bool)->PB_View {
  if !PB_fits(wire.count,out.count) {return PB_View.{error=PB_LIMIT};}
  if overlap {return PB_View.{error=PB_ALIAS};}
  for b,i:wire {out[i]=b;} return PB_View.{wire=out[0..<wire.count]};
}
// @proto-runtime-checks
#authority;

#test "field search distinguishes presence absence and malformed input" {
  wire:=[5]u8.[8,1,18,1,97];
  a:=PB_find(wire[..],1);#assert a.error==0 && !a.done && a.number==1 && a.raw==1;
  b:=PB_find(wire[..],2);#assert b.error==0 && !b.done && b.payload.count==1 && b.payload[0]==97;
  missing:=PB_find(wire[..],3);#assert missing.error==0 && missing.done && missing.raw==0 && missing.payload.count==0;
  empty:=PB_find(wire[0..<0],1);#assert empty.error==0 && empty.done;
  bad:=[1]u8.[128];invalid:=PB_find(bad[..],1);#assert invalid.error==PB_WIRE && invalid.done && invalid.raw==0 && invalid.payload.count==0;
  trailing:=[3]u8.[8,1,128];found:=PB_find(trailing[..],1);#assert found.error==0 && !found.done && found.raw==1;
}

#test "message admission preserves depth and foreign view bounds" {
  #assert PB_message_admitted(0,0) && PB_message_admitted(64,1048576);
  #assert !PB_message_admitted(-1,0) && !PB_message_admitted(65,0) && !PB_message_admitted(0,-1) && !PB_message_admitted(0,1048577);
  #assert PB_root_depth()==0 && PB_default_fields()==524288 && PB_require_fields() && !PB_partial_fields();
}

#test "count admission includes both endpoints and rejects neighbors" {
  #assert !PB_small(-1) && PB_small(0) && PB_small(1048576) && !PB_small(1048577);
  #assert PB_inputs(0,0,0,0)==PB_OK && PB_inputs(1048576,0,1048576,1048576)==PB_OK;
  #assert PB_inputs(-1,0,0,0)==PB_LIMIT && PB_inputs(1048577,0,0,0)==PB_LIMIT;
  #assert PB_inputs(0,-1,0,0)==PB_LIMIT;
  #assert PB_inputs(0,0,-1,0)==PB_LIMIT && PB_inputs(0,0,1048577,0)==PB_LIMIT;
  #assert PB_inputs(0,0,0,-1)==PB_LIMIT && PB_inputs(0,0,0,1048577)==PB_LIMIT;
  #assert PB_cursor(0,0) && !PB_cursor(0,1) && !PB_cursor(0,-1);
  #assert PB_cursor(1048576,1048576) && !PB_cursor(1048577,0) && !PB_cursor(-1,0);
  #assert PB_fits(0,0) && !PB_fits(0,-1) && PB_fits(1048576,1048576);
  #assert !PB_fits(1048577,1048577) && !PB_fits(-1,0) && !PB_fits(2,1);
}
#test "exact capacities preserve preflight and full field copies" {
  wire:=[4] u8.[8,8,24,2];out:[32] u8;removed:=[1] u32.[1];empty:=wire[0..<0];
  for i:0..<32 {out[i]=99;}
  ids:=[2] u32.[1,4];
  v0:=PB_rewrite_checked(wire[..],ids[..],2,0,0,empty,"",false,true,out[0..<2],false);
  #assert v0.error==0 && v0.wire.count==2 && out[0]==24 && out[1]==2;
  #assert PB_rewrite_checked(wire[..],ids[..],2,0,0,empty,"",false,true,out[0..<1],false).error==PB_LIMIT;
  out[0]=99;out[1]=99;
  #assert PB_rewrite_checked(empty,ids[..],2,0,0,empty,"",false,true,out[0..<0],false).error==PB_OK;
  #assert PB_copy_checked(wire[..],out[0..<4],false).error==0;
  v:=PB_rewrite_checked(wire[..],removed[0..<0],2,0,0,empty,"",false,true,out[0..<4],false);
  #assert v.error==0 && v.wire.count==4;
  for i:0..<4 {#assert out[i]==wire[i];}#assert out[4]==99;
  out[0]=99;
  #assert PB_rewrite_checked(wire[..],removed[0..<0],2,0,0,empty,"",false,true,out[0..<3],false).error==PB_LIMIT;
  #assert out[0]==99;
  v=PB_rewrite_checked(wire[..],removed[..],2,0,0,empty,"",false,true,out[0..<2],false);
  #assert v.error==0 && v.wire.count==2 && out[0]==24 && out[1]==2;
  #assert PB_rewrite_checked(wire[..],removed[..],2,0,0,empty,"",false,true,out[0..<1],false).error==PB_LIMIT;
  v=PB_rewrite_checked(empty,removed[..],2,1,0,empty,"",false,false,out[0..<9],false);
  #assert v.error==0 && v.wire.count==9;
  #assert PB_rewrite_checked(empty,removed[..],2,1,0,empty,"",false,false,out[0..<8],false).error==PB_LIMIT;
  v=PB_rewrite_checked(empty,removed[..],2,5,0,empty,"",false,false,out[0..<5],false);
  #assert v.error==0 && v.wire.count==5;
  #assert PB_rewrite_checked(empty,removed[..],2,5,0,empty,"",false,false,out[0..<4],false).error==PB_LIMIT;
  v=PB_rewrite_checked(empty,removed[..],2,2,0,wire[..],"",false,false,out[0..<6],false);
  #assert v.error==0 && v.wire.count==6;
  #assert PB_rewrite_checked(empty,removed[..],2,2,0,wire[..],"",false,false,out[0..<5],false).error==PB_LIMIT;
  v=PB_rewrite_checked(empty,removed[..],16,5,0,empty,"",false,false,out[0..<6],false);
  #assert v.error==0 && v.wire.count==6 && out[0]==133 && out[1]==1;
  #assert PB_rewrite_checked(empty,removed[..],16,5,0,empty,"",false,false,out[0..<5],false).error==PB_LIMIT;
  v=PB_rewrite_checked(empty,removed[..],15,1,0,empty,"",false,false,out[0..<9],false);
  #assert v.error==0 && v.wire.count==9 && out[0]==121;
  #assert PB_rewrite_checked(empty,removed[..],15,1,0,empty,"",false,false,out[0..<8],false).error==PB_LIMIT;
}
#test "field boundaries, fixed widths and minimal encodings" {
  zeros:[8] u8;#assert PB_scalar(zeros[..],0,1).raw==0 && PB_scalar(zeros[..],0,5).raw==0;
  bytes:=[16] u8.[8,150,1,17,1,2,3,4,5,6,7,8,29,9,10,11];
  f:=PB_scan(bytes[..],0);#assert f.error==0 && f.number==1 && f.wt==0 && f.raw==150 && f.start==0 && f.next==3 && !f.done;
  #assert PB_minimal(bytes[..],f);
  f=PB_scan(bytes[..],3);#assert f.error==0 && f.number==2 && f.wt==1 && f.raw==578437695752307201 && f.start==3 && f.next==12;
  #assert PB_minimal(bytes[..],f);
  #assert PB_scalar(bytes[..],12,5).raw==185207069;
  #assert PB_scalar(bytes[..],13,5).error==PB_WIRE;
  #assert PB_scalar(bytes[..],9,1).error==PB_WIRE;
  #assert PB_scalar(bytes[..],-1,0).error==PB_WIRE;
  #assert PB_scalar(bytes[..],17,0).error==PB_WIRE;
  #assert PB_scalar(bytes[..],0,3).error==PB_WIRE;
  #assert PB_scan(bytes[..],-1).error==PB_LIMIT;
  #assert PB_scan(bytes[..],17).error==PB_LIMIT;
  f=PB_scan(bytes[..],16);#assert f.done && f.start==16 && f.next==16 && f.error==0;
  maxkey:=[6] u8.[248,255,255,255,15,0];f=PB_scan(maxkey[..],0);
  #assert f.error==0 && f.number==536870911 && f.next==6;
    too_high:=[6] u8.[128,128,128,128,16,0];#assert PB_scan(too_high[..],0).error==PB_WIRE;
  maxkey[0]=128;maxkey[4]=16;#assert PB_scan(maxkey[..],0).error==PB_WIRE;
  nonminimal:=[4] u8.[136,0,128,0];f=PB_scan(nonminimal[..],0);
  #assert f.error==0 && !PB_minimal(nonminimal[..],f);
  nonminimal[0]=8;f=PB_scan(nonminimal[0..<3],0);#assert f.error==0 && PB_minimal(nonminimal[..],f);
  values:=[3] u8.[8,128,0];f=PB_scan(values[..],0);#assert f.error==0 && !PB_minimal(values[..],f);
  lengths:=[4] u8.[10,129,0,65];f=PB_scan(lengths[..],0);
  #assert f.error==0 && f.payload.count==1 && f.payload[0]==65 && f.next==4 && !PB_minimal(lengths[..],f);
  lengths[1]=1;f=PB_scan(lengths[0..<3],0);#assert f.error==0 && f.next==3 && PB_minimal(lengths[..],f);
    badlen:=[2] u8.[10,128];#assert !PB_minimal(badlen[..],PB_Field.{start=0,wt=2});
  #assert !PB_minimal(bytes[..],PB_Field.{start=-1,wt=1});
  broken:=[1] u8.[128];#assert !PB_minimal(broken[..],PB_Field.{start=0});
  #assert !PB_minimal(broken[..],PB_Field.{start=0,wt=2});
  empty:=[2] u8.[10,0];f=PB_scan(empty[..],0);#assert f.error==0 && f.payload.count==0 && f.next==2;
  truncated:=[2] u8.[10,128];#assert PB_scan(truncated[..],0).error==PB_WIRE;
}
#test "Unicode scalar boundaries and text policy" {
  u:=[4] u8.[240,144,128,128];#assert PB_utf8(u[..]);
  u[1]=143;#assert !PB_utf8(u[..]);u[1]=144;
  u[0]=244;u[1]=143;u[2]=191;u[3]=191;#assert PB_utf8(u[..]);
  u[1]=144;#assert !PB_utf8(u[..]);u[0]=245;#assert !PB_utf8(u[..]);
  u[0]=240;u[1]=144;u[2]=128;u[3]=128;
  for n:1..<4 {#assert !PB_utf8(u[0..<n]);}
  u[2]=127;#assert !PB_utf8(u[..]);u[2]=192;#assert !PB_utf8(u[..]);u[2]=128;
  u[3]=127;#assert !PB_utf8(u[..]);u[3]=192;#assert !PB_utf8(u[..]);
  u[0]=224;u[1]=160;u[2]=128;#assert PB_utf8(u[0..<3]);
  u[1]=159;#assert !PB_utf8(u[0..<3]);u[0]=237;u[1]=159;#assert PB_utf8(u[0..<3]);
  u[1]=160;#assert !PB_utf8(u[0..<3]);u[0]=239;u[1]=191;u[2]=191;#assert PB_utf8(u[0..<3]);
  u[0]=225;u[1]=128;u[2]=128;#assert PB_utf8(u[0..<3]);
  u[0]=241;u[1]=128;u[2]=128;u[3]=128;#assert PB_utf8(u[..]);
  #assert PB_utf8(u[0..<0]);
  text:=[3] u8.[65,66,67];#assert PB_no_nul(text[..]) && PB_no_nul(text[0..<1]);
  text[1]=0;#assert !PB_no_nul(text[..]);#assert !PB_no_nul(text[0..<0]);
}
#test "copy policy and preflight do not corrupt caller storage" {
  wire:=[2] u8.[8,1];out:[16] u8;
  for i:0..<16 {out[i]=99;}
  v:=PB_copy_checked(wire[..],out[..],false);
  #assert v.error==0 && v.wire.count==2 && out[0]==8 && out[1]==1 && out[2]==99;
  out[0]=99;v=PB_copy_checked(wire[..],out[..],true);#assert v.error==PB_ALIAS && out[0]==99;
  v=PB_copy_checked(wire[..],out[0..<1],false);#assert v.error==PB_LIMIT && out[0]==99;
  v=PB_copy_checked(wire[0..<0],out[0..<0],false);#assert v.error==0 && v.wire.count==0;
  removed:=[2] u32.[1,3];empty:=wire[0..<0];
  v=PB_rewrite_checked(wire[..],removed[..],2,0,150,empty,"",false,false,out[..],false);
  #assert v.error==0 && v.wire.count==3 && out[0]==16 && out[1]==150 && out[2]==1 && out[3]==99;
  out[0]=99;
  v=PB_rewrite_checked(wire[..],removed[..],2,0,150,empty,"",false,false,out[..],true);
  #assert v.error==PB_ALIAS && out[0]==99;
  v=PB_rewrite_checked(wire[..],removed[..],2,0,150,empty,"",false,false,out[0..<2],false);
  #assert v.error==PB_LIMIT && out[0]==99;
  v=PB_rewrite_checked(wire[..],removed[..],2,3,0,empty,"",false,false,out[..],false);
  #assert v.error==PB_WIRE && out[0]==99;
  bad:=[2] u8.[10,2];
  v=PB_rewrite_checked(bad[..],removed[..],2,0,0,empty,"",false,false,out[..],false);
  #assert v.error==PB_WIRE && out[0]==99;
  v=PB_rewrite_checked(wire[..],removed[..],2,3,0,empty,"",false,true,out[..],false);
  #assert v.error==0 && v.wire.count==0 && out[0]==99;
}
#test "rewrite preserves unknown siblings and emits each wire representation" {
  wire:=[4] u8.[8,8,24,2];out: [32] u8;
  removed:=[1] u32.[1];empty:=wire[0..<0];
  v:=PB_rewrite_checked(wire[..],removed[..],2,5,67305985,empty,"",false,false,out[..],false);
  #assert v.error==0 && v.wire.count==7 && out[0]==24 && out[1]==2 && out[2]==21;
  for i:0..<4 {#assert out[i+3]==cast(u8)(i+1);}
  v=PB_rewrite_checked(empty,removed[..],2,1,578437695752307201,empty,"",false,false,out[..],false);
  #assert v.error==0 && v.wire.count==9 && out[0]==17;
  for i:0..<8 {#assert out[i+1]==cast(u8)(i+1);}
  v=PB_rewrite_checked(empty,removed[..],2,2,0,wire[..],"unused",false,false,out[..],false);
  #assert v.error==0 && v.wire.count==6 && out[0]==18 && out[1]==4;
  for i:0..<4 {#assert out[i+2]==wire[i];}
  v=PB_rewrite_checked(empty,removed[..],2,2,0,wire[..],"hi",true,false,out[..],false);
  #assert v.error==0 && v.wire.count==4 && out[0]==18 && out[1]==2 && out[2]==104 && out[3]==105;
  v=PB_rewrite_checked(wire[..],removed[..],2,0,0,empty,"",false,true,out[..],false);
  #assert v.error==0 && v.wire.count==2 && out[0]==24 && out[1]==2;
  for value:0..<260 {
    pos:=PB_put(out[..],1,cast(u64)value);
    #assert out[0]==24 && pos==1+PB_vsize(cast(u64)value);
    #assert PB_var(out[0..<pos],1).value==cast(u64)value;
  }
}

#test "full-width varints, overflow and every truncated prefix" {
  bytes:=[10] u8.[255,255,255,255,255,255,255,255,255,1];
  v:=PB_var(bytes[..],0);
  #assert v.error==0 && v.value==18446744073709551615 && v.next==10;
  for size:0..<10 {#assert PB_var(bytes[0..<size],0).error==PB_WIRE;}
  bytes[9]=2;#assert PB_var(bytes[..],0).error==PB_WIRE;
  #assert PB_var(bytes[..],-1).error==PB_WIRE;
  #assert PB_var(bytes[..],11).error==PB_WIRE;
}

#test "malformed field lengths and invalid keys" {
  truncated:=[3] u8.[10,2,0];#assert PB_scan(truncated[..],0).error==PB_WIRE;
  zero:=[1] u8.[0];#assert PB_scan(zero[..],0).error==PB_WIRE;
  group:=[1] u8.[11];#assert PB_scan(group[..],0).error==PB_WIRE;
  #assert PB_scan(zero[0..<0],0).done;
}

#test "zigzag endpoints and caller-buffer writer" {
  #assert PB_zigzag(0)==0 && PB_zigzag(-1)==1 && PB_zigzag(1)==2;
  minimum:s64=-9223372036854775808;maximum:s64=9223372036854775807;
  #assert PB_sint(PB_zigzag(minimum))==minimum;
  #assert PB_sint(PB_zigzag(maximum))==maximum;
  #assert PB_zigzag(minimum)==18446744073709551615;
  #assert PB_zigzag(maximum)==18446744073709551614;
  out:[10] u8;end:=PB_put(out[..],0,18446744073709551615);
  #assert end==10 && PB_vsize(18446744073709551615)==10;
  decoded:=PB_var(out[0..<end],0);
  #assert decoded.error==0 && decoded.value==18446744073709551615;
}
#forall "every two-byte varint input" bits: u16 {
  a:=cast,trunc(u8)bits;b:=cast(u8)(bits>>8);
  bytes:=[2] u8.[a,b];v:=PB_var(bytes[..],0);
  if a<128 {
    #assert v.error==0 && v.next==1 && v.value==cast(u64)a;
  } else if b<128 {
    #assert v.error==0 && v.next==2;
    #assert v.value==cast(u64)(a&127)+cast(u64)b*128;
  } else {
    #assert v.error==PB_WIRE;
  }
}

#forall "all one-byte truncations and UTF8" byte: u8 {
  bytes:=[1] u8.[byte];v:=PB_var(bytes[..],0);
  #assert (v.error==0)==(byte<128);
  #assert PB_utf8(bytes[..])==(byte<128);
}

#forall "all two-byte UTF8 inputs" bits: u16 {
  a:=cast,trunc(u8)bits;b:=cast(u8)(bits>>8);
  bytes:=[2] u8.[a,b];
  expected:=(a<128 && b<128) || (a>=194 && a<=223 && b>=128 && b<=191);
  #assert PB_utf8(bytes[..])==expected;
}

main :: () {}
)COAX_CORE";
static constexpr char PROTO_MEMORY_SOURCE[] = R"COAX_MEMORY(// Native-address bridge. All limits, parsing and copy policy live in wire.coax.
// These wrappers only observe storage overlap and forward to the checked core.
// Native ABI/sanitizer tests cover physical pointer validity and aliasing.
PB_rewrite :: (wire:[] u8,removed_fields:[] u32,number:u32,wt:u8,raw:u64,data:[] u8,txt:string,is_text:bool,clear:bool,out:[] u8)->PB_View {
  overlap:=bytes_overlap(wire,out) || bytes_overlap(data,out) || bytes_overlap(txt,out);
  return PB_rewrite_checked(wire,removed_fields,number,wt,raw,data,txt,is_text,clear,out,overlap);
}
PB_copy :: (wire:[] u8,out:[] u8)->PB_View {
  return PB_copy_checked(wire,out,bytes_overlap(wire,out));
}
)COAX_MEMORY";
static constexpr char PROTO_IMPORTER_SOURCE[] = R"COAX_IMPORTER(// Descriptor parser and Coax source generator. Host-owned bounded storage only.
// The same procedures execute in checks and in the native compiler.
#authority;
D_MAX_BYTES :: 8388608;
// Must match PB_MAX_BYTES in wire.coax; importer tests check the shared cap.
D_WIRE_MAX_BYTES :: 1048576;
D_tighter_message_limit :: (n:u64)->bool {return n>0 && n<cast(u64)D_WIRE_MAX_BYTES;}
#test "only tighter message limits need a generated guard" {
 #assert !D_tighter_message_limit(0) && D_tighter_message_limit(1);
 #assert D_tighter_message_limit(1048575) && !D_tighter_message_limit(1048576) && !D_tighter_message_limit(1048577);
}
D_Wire :: struct {number:u32;wt:u8;raw:u64;data:string;next:int;present:bool;}
D_Opts :: struct {n:[10] u64;text:[10] string;bits:u32;map_entry:bool;}
D_Type :: struct {full:string;name:string;wire:string;first:int;count:int;groups:int;group_count:int;opts:D_Opts;}
D_Field :: struct {name:string;type_name:string;number:u32;label:int;kind:int;oneof:int=-1;optional:bool;opts:D_Opts;target:string;}
D_Value :: struct {name:string;number:s64;}
D_State :: struct {
  types:[] D_Type;enums:[] D_Type;fields:[] D_Field;values:[] D_Value;groups:[] string;files:[] string;deps:[] string;
  arena:[] u8;output:[] u8;symbols:[] string;
  nt:int;ne:int;nf:int;nv:int;ng:int;np:int;nd:int;ns:int;used:int;written:int;visited:int;
  error:string;
}
#test "all admission policy endpoints" {
  c:=D_capacities();
  #assert c.types==256 && c.fields==2048 && c.values==8192 && c.groups==65536 && c.files==256 && c.deps==8192 && c.symbols==32768 && c.arena==134217728 && c.output==8388608;
  #assert D_input(8388608,"P") && !D_input(8388609,"P") && !D_input(0,"bad-");
  #assert D_type_count(0) && D_type_count(255) && !D_type_count(256) && !D_type_count(-1);
  #assert D_depth(0) && D_depth(31) && !D_depth(32) && !D_depth(-1);
  #assert D_storage(0,1) && !D_storage(0,0) && !D_storage(-1,1);
  #assert D_work(40000,400000) && !D_work(40001,400000) && !D_work(40000,400001);
  #assert D_wire_number(1) && D_wire_number(536870911) && !D_wire_number(0) && !D_wire_number(536870912);
  #assert D_field_number(1) && D_field_number(18999) && !D_field_number(19000) && !D_field_number(19999) && D_field_number(20000);
  #assert G_output_room(8388608,0,8388608) && !G_output_room(8388608,1,8388609);
  #assert G_output_room(8388607,1,8388608) && !G_output_room(0,2,1);
}
#test "exact arena fit, invalid scalar, template edge and symbol collisions" {
  arena:[6] u8;output:[64] u8;symbols:[4] string;
  state:=[1] D_State.[D_State.{arena=arena[..],output=output[..],symbols=symbols[..]}];
  parts:=[1] string.["abcdef"];#assert G_cat(state[..],parts[..])=="abcdef" && state[0].error=="";
  state[0].used=0;x:=G_fmt(state[..],"$:",parts[..]);#assert state[0].error=="invalid source template";
  state[0].error="";a:[10] string;x=G_value(state[..],D_Field.{kind=10},a[..]);#assert state[0].error=="unimplemented descriptor scalar";
  state[0].error="";G_write(state[..],"A_9 ::\n// skip\nz ::\nA_9 ::");G_collisions(state[..]);#assert state[0].error!="";
  #assert state[0].ns==2 && symbols[0]=="A_9" && symbols[1]=="z";
  state[0].error="";state[0].ns=0;state[0].written=0;G_write(state[..],"A");G_collisions(state[..]);#assert state[0].error=="" && state[0].ns==0;
  state[0].error="";state[0].ns=4;D_symbol(state[..],"another");#assert state[0].error!="";
}
#test "native span independent text and work ceilings" {
  state:[1] D_State;bytes:[16385] u8;for i:0..<16385 {bytes[i]=65;}
  x:=D_text(state[..],cast(string)bytes[0..<16384]);#assert state[0].error=="" && x.count==16384;
  x=D_text(state[..],cast(string)bytes[..]);#assert state[0].error!="";
  state[0].error="";state[0].visited=399999;wire:=[2] u8.[8,1];D_object(state[..],cast(string)wire[..]);
  #assert state[0].visited==400000 && state[0].error=="";
  D_object(state[..],cast(string)wire[..]);#assert state[0].error!="" && state[0].visited==400001;
}

#forall "ASCII identifier classes" c:u8 {
  alpha:=false;digit:=false;
  for x:"ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz" {if x==c {alpha=true;}}
  for x:"0123456789" {if x==c {digit=true;}}
  #assert D_alpha(c)==alpha && D_digit(c)==digit;
}
#test "identifier, namespace and capacity endpoints" {
  #assert D_identifier("Z_09az") && !D_identifier("") && !D_identifier("0x") && !D_identifier("a-");
  bytes:[257] u8;for i:0..<257 {bytes[i]=65;}
  #assert D_identifier(cast(string)bytes[0..<256]) && !D_identifier(cast(string)bytes[..]);
  #assert D_package("") && D_package("a.b") && !D_package("a..b") && !D_package("a.") && !D_package(".a");
  #assert D_leaf(".one.two")=="two" && D_leaf("Item")=="Item";
  #assert D_small(0,0) && !D_small(-1,0) && D_small(8,8) && !D_small(9,8);
}
#test "text assembly, number formatting and bounded templates" {
  arena:[256] u8;out:[16] u8;state:=[1] D_State.[D_State.{arena=arena[..],output=out[..]}];
  a:=[3] string.["abc","","def"];
  #assert G_cat(state[..],a[..])=="abcdef";
  #assert G_num(state[..],0)=="0" && G_num(state[..],1)=="1" && G_num(state[..],-1)=="-1";
  #assert G_num(state[..],9223372036854775807)=="9223372036854775807";
  #assert G_num(state[..],-9223372036854775808)=="-9223372036854775808";
  #assert G_fmt(state[..],"<$0:$2>",a[..])=="<abc:def>";
  G_line(state[..],"$1$0",a[..]);#assert state[0].written==4 && cast(string)out[0..<4]=="abc\n";
  G_write(state[..],"012345678901");#assert state[0].written==16 && state[0].error=="";
  G_write(state[..],"x");#assert state[0].error!="" && state[0].written==16;
  D_fail(state[..],"another");#assert state[0].error=="generated source budget";
  state[0].error="";x:=G_fmt(state[..],"$3",a[..]);#assert state[0].error=="missing source template argument";
  state[0].error="";x=G_fmt(state[..],"$",a[..]);#assert state[0].error=="invalid source template";
  state[0].error="";x=G_fmt(state[..],"$A",a[..]);#assert state[0].error=="invalid source template";
  state[0].error="";state[0].arena=arena[0..<0];x=G_cat(state[..],a[..]);#assert state[0].error!="";
}
#test "varint, slicing, fixed-width and malformed wire boundaries" {
  state:[1] D_State;
  max:=[10] u8.[255,255,255,255,255,255,255,255,255,1];t:=cast(string)max[..];
  v:=D_var(state[..],t,0);#assert v.raw==18446744073709551615 && v.next==10 && v.present && state[0].error=="";
  for n:0..<10 {state[0].error="";v=D_var(state[..],t[0..<n],0);#assert state[0].error!="";}
  state[0].error="";max[9]=2;v=D_var(state[..],cast(string)max[..],0);#assert state[0].error=="descriptor varint overflow";
  state[0].error="";v=D_var(state[..],"",-1);#assert state[0].error!="";
  fields:=[16] u8.[9,1,2,3,4,5,6,7,8,21,9,10,11,12,26,0];state[0].error="";
  v=D_scan(state[..],cast(string)fields[..],0);#assert v.number==1 && v.wt==1 && v.data.count==8 && v.data[0]==1 && v.data[7]==8 && v.next==9;
  v=D_scan(state[..],cast(string)fields[..],9);#assert v.number==2 && v.wt==5 && v.data.count==4 && v.data[3]==12 && v.next==14;
  v=D_scan(state[..],cast(string)fields[..],14);#assert v.number==3 && v.wt==2 && v.data=="" && v.next==16;
  v=D_scan(state[..],cast(string)fields[..],16);#assert !v.present && state[0].error=="";
  v=D_scan(state[..],cast(string)fields[0..<8],0);#assert state[0].error!="";
  state[0].error="";zero:=[1] u8.[0];v=D_scan(state[..],cast(string)zero[..],0);#assert state[0].error!="";
  state[0].error="";zero[0]=11;v=D_scan(state[..],cast(string)zero[..],0);#assert state[0].error=="descriptor groups are unsupported";
  state[0].error="";x:=D_bytes(state[..],D_Wire.{present=true,wt=0});#assert state[0].error!="";
  state[0].error="";i:=D_integer(state[..],cast(string)fields[..],1);#assert state[0].error!="";
  state[0].error="";#assert D_integer(state[..],"",1)==0 && !D_one(state[..],"",1).present;
}
#test "UTF8 scalar boundaries" {
  #assert D_utf8("") && D_utf8("ASCII");
  u:=[4] u8.[240,144,128,128];#assert D_utf8(cast(string)u[..]);u[1]=143;#assert !D_utf8(cast(string)u[..]);u[1]=144;
  for n:1..<4 {#assert !D_utf8(cast(string)u[0..<n]);}
  u[0]=244;u[1]=143;u[2]=191;u[3]=191;#assert D_utf8(cast(string)u[..]);u[1]=144;#assert !D_utf8(cast(string)u[..]);
  u[0]=245;#assert !D_utf8(cast(string)u[..]);u[0]=224;u[1]=160;u[2]=128;#assert D_utf8(cast(string)u[0..<3]);
  u[1]=159;#assert !D_utf8(cast(string)u[0..<3]);u[0]=237;u[1]=159;#assert D_utf8(cast(string)u[0..<3]);u[1]=160;#assert !D_utf8(cast(string)u[0..<3]);
  u[0]=239;u[1]=191;u[2]=191;#assert D_utf8(cast(string)u[0..<3]);u[0]=241;u[1]=128;u[2]=128;u[3]=128;#assert D_utf8(cast(string)u[..]);
  u[2]=127;#assert !D_utf8(cast(string)u[..]);u[2]=192;#assert !D_utf8(cast(string)u[..]);u[2]=128;u[3]=127;#assert !D_utf8(cast(string)u[..]);u[3]=192;#assert !D_utf8(cast(string)u[..]);
}
#test "owned runtime marker substitution" {
  out:[128] u8;state:=[1] D_State.[D_State.{output=out[..]}];
  G_runtime(state[..],"PB_x PB_foo1 PB_","N_");
  #assert state[0].error=="" && cast(string)out[0..<state[0].written]=="N_x N_foo1 N_\n";
  state[0].written=0;G_runtime(state[..],"A","N_");#assert cast(string)out[0..<state[0].written]=="A\n";
}
#test "descriptor wire rejects truncation and duplicate singular metadata" {
  state:[1] D_State;
  a:=[3] u8.[8,150,1];v:=D_scan(state[..],cast(string)a[..],0);
  #assert state[0].error=="" && v.number==1 && v.wt==0 && v.raw==150 && v.next==3 && v.present;
  state[0].error="";v=D_scan(state[..],cast(string)a[0..<2],0);#assert state[0].error!="";
  state[0].error="";twice:=[4] u8.[8,1,8,2];v=D_one(state[..],cast(string)twice[..],1);#assert state[0].error!="";
}
D_storage :: (count:int,capacity:int)->bool {return count>=0 && count<capacity;}
D_type_count :: (count:int)->bool {return count>=0 && count<256;}
D_depth :: (count:int)->bool {return count>=0 && count<32;}
D_work :: (count:int,total:int)->bool {return count<=40000 && total<=400000;}
D_wire_number :: (n:u64)->bool {return n>=1 && n<=536870911;}
D_field_number :: (n:u64)->bool {return D_wire_number(n) && !(n>=19000 && n<=19999);}
G_output_room :: (written:int,size:int,capacity:int)->bool {return size<=capacity-written && size<=D_MAX_BYTES-written;}
D_input :: (count:int,prefix:string)->bool {return D_small(count,D_MAX_BYTES) && D_identifier(prefix);}
D_Capacities :: struct {types:int;fields:int;values:int;groups:int;files:int;deps:int;symbols:int;arena:int;output:int;}
D_capacities :: ()->D_Capacities #export_c {
  return D_Capacities.{types=256,fields=2048,values=8192,groups=65536,files=256,deps=8192,symbols=32768,arena=134217728,output=D_MAX_BYTES};
}
D_fail :: (s:[] D_State,message:string) {if s[0].error=="" {s[0].error=message;}}
D_alpha :: (c:u8)->bool {return (c>=65 && c<=90) || (c>=97 && c<=122) || c==95;}
D_digit :: (c:u8)->bool {return c>=48 && c<=57;}
D_identifier :: (t:string)->bool {
  if t.count==0 || t.count>256 {return false;}
  if !D_alpha(t[0]) {return false;}for c:t {if !D_alpha(c) && !D_digit(c) {return false;}}return true;
}
D_small :: (count:int,limit:int)->bool {return count>=0 && count<=limit;}
D_var :: (s:[] D_State,t:string,at:int)->D_Wire {
  pos:=at;v:u64=0;shift:=0;
  while true {
    if pos<0 || pos>=t.count {D_fail(s,"truncated descriptor varint");return D_Wire.{};}
    c:=t[pos];pos+=1;
    if shift==63 && c>1 {D_fail(s,"descriptor varint overflow");return D_Wire.{};}
    v=v | (cast(u64)(c&127)<<shift);
    if c<128 {return D_Wire.{raw=v,next=pos,present=true};}shift+=7;
  }return D_Wire.{};
}
D_scan :: (s:[] D_State,t:string,at:int)->D_Wire {
  if at==t.count {return D_Wire.{};}
  k:=D_var(s,t,at);if s[0].error!="" {return D_Wire.{};}
  number:=k.raw>>3;
  if !D_wire_number(number) {D_fail(s,"invalid descriptor field number");return D_Wire.{};}
  w:=cast(u8)(k.raw&7);f:=D_Wire.{number=cast(u32)number,wt=w,next=k.next,present=true};
  if w==0 {v:=D_var(s,t,k.next);f.raw=v.raw;f.next=v.next;return f;}
  n:u64=---;
  if w==1 {n=8;}else if w==5 {n=4;}else if w==2 {v:=D_var(s,t,k.next);n=v.raw;f.next=v.next;}
  else {D_fail(s,"descriptor groups are unsupported");return D_Wire.{};}
  if n>cast(u64)(t.count-f.next) {D_fail(s,"truncated descriptor field length");return D_Wire.{};}
  f.data=t[f.next..<f.next+cast(int)n];f.next+=cast(int)n;return f;
}
D_object :: (s:[] D_State,t:string) {
  at:=0;count:=0;
  while at<t.count && s[0].error=="" {
    f:=D_scan(s,t,at);count+=1;s[0].visited+=1;
    if !D_work(count,s[0].visited) {D_fail(s,"descriptor field/work budget");return;}at=f.next;
  }
}
D_one :: (s:[] D_State,t:string,number:u32)->D_Wire {
  at:=0;found:D_Wire;
  while at<t.count && s[0].error=="" {f:=D_scan(s,t,at);at=f.next;if f.number==number {
    if found.present {D_fail(s,"duplicate descriptor metadata field");return D_Wire.{};}found=f;
  }}return found;
}
D_integer :: (s:[] D_State,t:string,number:u32)->u64 {
  f:=D_one(s,t,number);if f.present && f.wt!=0 {D_fail(s,"descriptor integer wire type");}return f.raw;
}
D_bytes :: (s:[] D_State,f:D_Wire)->string {
  if f.present && f.wt!=2 {D_fail(s,"descriptor bytes wire type");}return f.data;
}
D_text :: (s:[] D_State,t:string)->string {
  if !D_small(t.count,16384) || !D_utf8(t) {D_fail(s,"invalid/oversized descriptor UTF-8 string");}return t;
}
D_get :: (s:[] D_State,t:string,number:u32)->string {return D_text(s,D_bytes(s,D_one(s,t,number)));}
D_ident :: (s:[] D_State,t:string,number:u32)->string {
  v:=D_get(s,t,number);if !D_identifier(v) {D_fail(s,"invalid/oversized descriptor identifier");}return v;
}
D_utf8 :: (t:string)->bool {
  at:=0;while at<t.count {a:=t[at];at+=1;if a<128 {continue;}
    n:int=---;lo:u8=128;hi:u8=191;
    if a>=194 && a<=223 {n=1;}else if a>=224 && a<=239 {n=2;if a==224 {lo=160;}if a==237 {hi=159;}}
    else if a>=240 && a<=244 {n=3;if a==240 {lo=144;}if a==244 {hi=143;}}else {return false;}
    if n>t.count-at || t[at]<lo || t[at]>hi {return false;}at+=1;
    for i:1..<n {if t[at]<128 || t[at]>191 {return false;}at+=1;}
  }return true;
}
G_cat :: (s:[] D_State,parts:[] string)->string {
  start:=s[0].used;
  for p:parts {if p.count>s[0].arena.count-s[0].used {D_fail(s,"importer scratch budget");return "";}
    for c:p {s[0].arena[s[0].used]=c;s[0].used+=1;}}
  return cast(string)s[0].arena[start..<s[0].used];
}
G_num :: (s:[] D_State,n:s64)->string {
  digits:[20] u8;at:=20;value:u64=---;
  if n<0 {value=cast(u64)(-(n+1))+1;}else {value=cast(u64)n;}
  while true {at-=1;digits[at]=cast(u8)(value%10)+48;value=value/10;if value==0 {break;}}
  if n<0 {at-=1;digits[at]=45;}parts:=[1] string.[cast(string)digits[at..<20]];return G_cat(s,parts[..]);
}
D_has_option :: (o:D_Opts,n:int)->bool {return (o.bits & (cast(u32)1<<n))!=0;}
D_options :: (s:[] D_State,wire:string,tag:u32,message:bool)->D_Opts {
  t:=D_bytes(s,D_one(s,wire,tag));D_object(s,t);at:=0;o:D_Opts;
  seen:[999] bool;
  while at<t.count && s[0].error=="" {
    f:=D_scan(s,t,at);at=f.next;
    if f.number<999 {
      if seen[f.number] {D_fail(s,"duplicate descriptor option");return o;}seen[f.number]=true;
      if f.number==7 {if f.wt!=0 {D_fail(s,"invalid numeric policy option");}o.map_entry=f.raw!=0;}continue;
    }
    if f.number<51000 || f.number>51009 {D_fail(s,"unsupported policy option");return o;}
    n:=cast(int)f.number-51000;
    allowed:=(message && (n==0 || n==1 || n==6)) || (!message && (n==2 || n==3 || n==4 || n==5 || n==7 || n==8 || n==9));
    if !allowed || D_has_option(o,n) {D_fail(s,"unsupported or duplicate descriptor option");return o;}
    o.bits=o.bits | (cast(u32)1<<n);
    if n==5 || n==6 || n==7 {o.text[n]=D_text(s,D_bytes(s,f));}
    else {if f.wt!=0 || f.raw>9223372036854775807 || (n==4 && f.raw>1) || (n==9 && f.raw>4294967295) {D_fail(s,"invalid numeric policy option");return o;}o.n[n]=f.raw;}
  }return o;
}
D_leaf :: (t:string)->string {at:=0;for c,i:t {if c==46 {at=i+1;}}return t[at..<t.count];}
D_full :: (s:[] D_State,parent:string,name:string)->string {p:=[3] string.[parent,".",name];return G_cat(s,p[..]);}
D_package :: (t:string)->bool {
  if t.count>4096 {return false;}if t.count==0 {return true;}
  at:=0;parts:=0;for c,i:t {if c==46 {if !D_identifier(t[at..<i]) {return false;}at=i+1;parts+=1;}}
  return parts<32 && D_identifier(t[at..<t.count]);
}
D_enum :: (s:[] D_State,t:string,parent:string) {
  D_object(s,t);name:=D_ident(s,t,1);e:=D_Type.{wire=t,full=D_full(s,parent,name),first=s[0].nv};at:=0;
  while at<t.count && s[0].error=="" {f:=D_scan(s,t,at);at=f.next;if f.number!=2 {continue;}
    q:=D_bytes(s,f);D_object(s,q);vn:=D_ident(s,q,1);num:=D_one(s,q,2);raw:=D_integer(s,q,2);
    if !num.present || !(raw<=2147483647 || raw>=18446744071562067968) {D_fail(s,"enum value outside int32");return;}
    value:=cast,trunc(s32)raw;
    for i:e.first..<s[0].nv {if s[0].values[i].name==vn {D_fail(s,"duplicate enum value name");return;}}
    if !D_storage(s[0].nv,s[0].values.count) {D_fail(s,"descriptor enum budget exceeded");return;}
    s[0].values[s[0].nv]=D_Value.{name=vn,number=cast(s64)value};s[0].nv+=1;e.count+=1;
  }
  if e.count==0 || s[0].values[e.first].number!=0 {D_fail(s,"proto3 enum must start at zero");return;}
  if !D_storage(s[0].ne,s[0].enums.count) {D_fail(s,"descriptor enum budget exceeded");return;}
  s[0].enums[s[0].ne]=e;s[0].ne+=1;
}
D_message :: (s:[] D_State,t:string,parent:string,depth:int) {
  if !D_depth(depth) {D_fail(s,"descriptor nesting budget");return;}D_object(s,t);
  m:=D_Type.{wire=t,full=D_full(s,parent,D_ident(s,t,1)),first=s[0].nf,groups=s[0].ng,opts=D_options(s,t,7,true)};
  if m.opts.map_entry || D_one(s,t,5).present || D_one(s,t,6).present {D_fail(s,"maps/message extensions are not supported");return;}
  at:=0;while at<t.count && s[0].error=="" {raw:=D_scan(s,t,at);at=raw.next;if raw.number==8 {
    q:=D_bytes(s,raw);D_object(s,q);name:=D_ident(s,q,1);
    if !D_type_count(m.group_count) || !D_storage(s[0].ng,s[0].groups.count) {D_fail(s,"descriptor oneof budget");return;}
    s[0].groups[s[0].ng]=name;s[0].ng+=1;m.group_count+=1;
  }}
  at=0;while at<t.count && s[0].error=="" {raw:=D_scan(s,t,at);at=raw.next;if raw.number!=2 {continue;}
    q:=D_bytes(s,raw);D_object(s,q);f:=D_Field.{name=D_ident(s,q,1),type_name=D_get(s,q,6),opts=D_options(s,q,8,false)};
    number:=D_integer(s,q,3);label:=D_integer(s,q,4);kind:=D_integer(s,q,5);optional:=D_integer(s,q,17);
    if !D_field_number(number) {D_fail(s,"invalid field number");return;}
    if !(label==1 || label==3) || kind<1 || kind>18 || kind==10 || D_one(s,q,2).present || D_one(s,q,7).present {D_fail(s,"unsupported field shape");return;}
    f.number=cast(u32)number;f.label=cast(int)label;f.kind=cast(int)kind;
    if D_has_option(f.opts,7) && f.opts.text[7]!="nonempty-no-nul" {D_fail(s,"unsupported grammar option");return;}
    g:=D_one(s,q,9);if g.present {group:=D_integer(s,q,9);if group>=cast(u64)m.group_count || f.label==3 {D_fail(s,"invalid oneof");return;}f.oneof=cast(int)group;}
    if optional>1 || (optional!=0 && f.oneof<0) {D_fail(s,"invalid proto3 optional flag");return;}f.optional=optional!=0;
    for i:m.first..<s[0].nf {if s[0].fields[i].name==f.name || s[0].fields[i].number==f.number {D_fail(s,"invalid/duplicate field");return;}}
    if !D_type_count(m.count) || !D_storage(s[0].nf,s[0].fields.count) {D_fail(s,"descriptor field budget exceeded");return;}
    s[0].fields[s[0].nf]=f;s[0].nf+=1;m.count+=1;
  }
  if !D_storage(s[0].nt,s[0].types.count) {D_fail(s,"descriptor message budget exceeded");return;}
  s[0].types[s[0].nt]=m;s[0].nt+=1;
  at=0;while at<t.count && s[0].error=="" {f:=D_scan(s,t,at);at=f.next;
    if f.number==3 {D_message(s,D_bytes(s,f),m.full,depth+1);}if f.number==4 {D_enum(s,D_bytes(s,f),m.full);}
  }
}
D_bind :: (s:[] D_State,t:D_Type,prefix:string)->string {
  leaf:=D_leaf(t.full);count:=0;
  for i:0..<s[0].nt {if D_leaf(s[0].types[i].full)==leaf {count+=1;}}
  for i:0..<s[0].ne {if D_leaf(s[0].enums[i].full)==leaf {count+=1;}}
  alias:=leaf;if count>1 {alias=t.full[1..<t.full.count];}
  start:=s[0].used;p:=[2] string.[prefix,"_"];head:=G_cat(s,p[..]);
  for c:alias {b:=[1] u8.[c];if c==46 {b[0]=95;}q:=[1] string.[cast(string)b[..]];piece:=G_cat(s,q[..]);}
  if s[0].error!="" {return "";}return cast(string)s[0].arena[start..<s[0].used];
}
D_parse :: (s:[] D_State,raw:string,prefix:string) {
  if !D_input(raw.count,prefix) {D_fail(s,"descriptor byte budget or invalid prefix");return;}
  D_object(s,raw);at:=0;
  while at<raw.count && s[0].error=="" {rf:=D_scan(s,raw,at);at=rf.next;if rf.number!=1 {continue;}
    t:=D_bytes(s,rf);D_object(s,t);name:=D_get(s,t,1);pkg:=D_get(s,t,2);
    if name.count==0 || name.count>4096 {D_fail(s,"invalid descriptor file name");return;}
    for c:name {if c==0 {D_fail(s,"invalid descriptor file name");return;}}
    for i:0..<s[0].np {if s[0].files[i]==name {D_fail(s,"duplicate descriptor file");return;}}
    if !D_storage(s[0].np,s[0].files.count) {D_fail(s,"descriptor file budget exceeded");return;}
    s[0].files[s[0].np]=name;s[0].np+=1;
    p:=0;while p<t.count && s[0].error=="" {f:=D_scan(s,t,p);p=f.next;if f.number==3 {
      if !D_storage(s[0].nd,s[0].deps.count) {D_fail(s,"descriptor dependency budget");return;}
      s[0].deps[s[0].nd]=D_text(s,D_bytes(s,f));s[0].nd+=1;
    }}
    if name=="google/protobuf/descriptor.proto" || name=="openagents/options/v1alpha1/options.proto" || name=="openagents/options/v1alpha2/numeric.proto" {continue;}
    if D_get(s,t,12)!="proto3" || D_one(s,t,7).present || D_one(s,t,14).present {D_fail(s,"only proto3 data descriptors are supported");return;}
    if !D_package(pkg) {D_fail(s,"invalid/oversized package");return;}
    parent:="";if pkg!="" {parts:=[2] string.[".",pkg];parent=G_cat(s,parts[..]);}
    p=0;while p<t.count && s[0].error=="" {f:=D_scan(s,t,p);p=f.next;if f.number==4 {D_message(s,D_bytes(s,f),parent,0);}if f.number==5 {D_enum(s,D_bytes(s,f),parent);}}
  }
  for i:0..<s[0].nd {found:=false;for j:0..<s[0].np {if s[0].deps[i]==s[0].files[j] {found=true;}}if !found {D_fail(s,"missing descriptor file dependency");return;}}
  if s[0].nt==0 {D_fail(s,"no runtime messages in descriptor");return;}
  for i:0..<s[0].nt {s[0].types[i].name=D_bind(s,s[0].types[i],prefix);}
  for i:0..<s[0].ne {s[0].enums[i].name=D_bind(s,s[0].enums[i],prefix);}
  for i:0..<s[0].nt {
    m:=s[0].types[i];D_symbol(s,m.name);
    if s[0].error!="" {return;}
    for j:m.first..<m.first+m.count {f:=s[0].fields[j];
      if f.kind==11 || f.kind==14 {
        found:=false;if f.kind==11 {for k:0..<s[0].nt {if s[0].types[k].full==f.type_name {f.target=s[0].types[k].name;found=true;}}}
        else {for k:0..<s[0].ne {if s[0].enums[k].full==f.type_name {found=true;}}}
        if !found {D_fail(s,"missing descriptor dependency or type mismatch");return;}
      }else {if f.type_name!="" {D_fail(s,"scalar has type_name");return;}}
      if ((f.opts.n[2]!=0 || f.opts.n[8]!=0) && f.kind!=9 && f.kind!=12) || (f.opts.n[3]!=0 && f.label!=3) || (D_has_option(f.opts,7) && f.kind!=9) || (D_has_option(f.opts,9) && f.kind!=13) {D_fail(s,"invalid policy option for field kind");return;}
      if f.optional {count:=0;for k:m.first..<m.first+m.count {if s[0].fields[k].oneof==f.oneof {count+=1;}}if count!=1 {D_fail(s,"invalid synthetic oneof");return;}}
      s[0].fields[j]=f;
    }
  }
  for i:0..<s[0].ne {D_symbol(s,s[0].enums[i].name);if s[0].error!="" {return;}}
  s[0].ns=0;
}
D_symbol :: (s:[] D_State,name:string) {
  for i:0..<s[0].ns {if s[0].symbols[i]==name {D_fail(s,"generated name collision; use narrower profiles");return;}}
  if !D_storage(s[0].ns,s[0].symbols.count) {D_fail(s,"generated symbol budget");return;}s[0].symbols[s[0].ns]=name;s[0].ns+=1;
}
// Template slots: 0 runtime prefix, 1 message, 2 field, 3 type, 4 option,
// 5 field number, 6 wire type, 7 decoded value, 8 oneof, 9 method/extra.
G_fmt :: (s:[] D_State,text:string,args:[] string)->string {
  start:=s[0].used;at:=0;begin:=0;
  while at<text.count && s[0].error=="" {
    if text[at]!=36 {at+=1;continue;}
    pieces:=[1] string.[text[begin..<at]];p:=G_cat(s,pieces[..]);at+=1;
    if at>=text.count || text[at]<48 || text[at]>57 {D_fail(s,"invalid source template");return "";}
    index:=cast(int)text[at]-48;if index>=args.count {D_fail(s,"missing source template argument");return "";}
    insert:=[1] string.[args[index]];q:=G_cat(s,insert[..]);at+=1;begin=at;
  }
  tail:=[1] string.[text[begin..<text.count]];q:=G_cat(s,tail[..]);
  if s[0].error!="" {return "";}return cast(string)s[0].arena[start..<s[0].used];
}
G_write :: (s:[] D_State,text:string) {
  if !G_output_room(s[0].written,text.count,s[0].output.count) {D_fail(s,"generated source budget");return;}
  for c:text {s[0].output[s[0].written]=c;s[0].written+=1;}
}
G_line :: (s:[] D_State,text:string,args:[] string) {G_write(s,G_fmt(s,text,args));G_write(s,"\n");}
G_scalar :: (kind:int)->string {
  if kind==1 {return "f64";}if kind==2 {return "f32";}if kind==3 || kind==16 || kind==18 {return "s64";}
  if kind==4 || kind==6 {return "u64";}if kind==5 || kind==14 || kind==15 || kind==17 {return "s32";}
  if kind==7 || kind==13 {return "u32";}if kind==8 {return "bool";}if kind==9 {return "string";}
  if kind==12 {return "[] u8";}return "";
}
G_option :: (kind:int)->string {
  if kind==1 {return "F64";}if kind==2 {return "F32";}if kind==3 || kind==16 || kind==18 {return "S64";}
  if kind==4 || kind==6 {return "U64";}if kind==5 || kind==14 || kind==15 || kind==17 {return "S32";}
  if kind==7 || kind==13 {return "U32";}if kind==8 {return "Bool";}if kind==9 {return "String";}
  if kind==12 {return "Bytes";}return "";
}
G_wt :: (kind:int)->int {if kind==1 || kind==6 || kind==16 {return 1;}if kind==2 || kind==7 || kind==15 {return 5;}if kind==9 || kind==11 || kind==12 {return 2;}return 0;}
G_value :: (s:[] D_State,f:D_Field,args:[] string)->string {
  if f.kind==11 {return G_fmt(s,"$3.{wire=f.payload}",args);}if f.kind==9 {return "cast(string) f.payload";}
  if f.kind==12 {return "f.payload";}if f.kind==3 || f.kind==5 || f.kind==14 || f.kind==15 || f.kind==16 {return G_fmt(s,"cast,trunc($3) f.raw",args);}
  if f.kind==4 || f.kind==6 {return "f.raw";}if f.kind==7 || f.kind==13 {return "cast(u32) f.raw";}
  if f.kind==8 {return "f.raw!=0";}if f.kind==17 || f.kind==18 {return G_fmt(s,"cast($3) $0sint(f.raw)",args);}
  if f.kind==1 {return "f64_from_bits(f.raw)";}if f.kind==2 {return "f32_from_bits(cast(u32) f.raw)";}
  D_fail(s,"unimplemented descriptor scalar");return "";
}
G_args :: (s:[] D_State,m:D_Type,f:D_Field,p:string,args:[] string) {
  args[0]=p;args[1]=m.name;args[2]=f.name;args[3]=G_scalar(f.kind);if f.kind==11 {args[3]=f.target;}
  if f.kind==11 {pieces:=[2] string.[f.target,"_Option"];args[4]=G_cat(s,pieces[..]);}
  else {pieces:=[2] string.[p,G_option(f.kind)];args[4]=G_cat(s,pieces[..]);}
  args[5]=G_num(s,cast(s64)f.number);args[6]=G_num(s,G_wt(f.kind));args[8]=G_num(s,f.oneof);args[7]=G_value(s,f,args);
}
G_constraints :: (s:[] D_State,f:D_Field,a:[] string) {
  G_line(s,"if f.wt!=$6 { $9WIRE}; }",a);
  if f.kind==7 || f.kind==13 || f.kind==17 {G_line(s,"if f.raw>4294967295 { $9VALUE}; }",a);}
  if f.kind==13 && D_has_option(f.opts,9) && f.opts.n[9]<4294967295 {a[5]=G_num(s,cast(s64)f.opts.n[9]);G_line(s,"if f.raw>$5 { $9VALUE}; }",a);}
  if f.kind==8 {G_line(s,"if canonical && f.raw>1 { $9VALUE}; }",a);}
  if f.kind==9 {G_line(s,"if !$0utf8(f.payload) { $9UTF8}; }",a);}
  if f.kind==9 || f.kind==12 {
    if f.opts.n[2]!=0 {a[5]=G_num(s,cast(s64)f.opts.n[2]);G_line(s,"if f.payload.count>$5 { $9LIMIT}; }",a);}
    if f.opts.n[8]!=0 {a[5]=G_num(s,cast(s64)f.opts.n[8]);G_line(s,"if f.payload.count!=$5 { $9LIMIT}; }",a);}
    if D_has_option(f.opts,7) {G_line(s,"if !$0no_nul(f.payload) { $9VALUE}; }",a);}
  }
  if (f.kind==1 || f.kind==2) && f.opts.text[5]=="rad" {G_line(s,"x:=$7;if x!=x || x < -3.141592653589793 || x>3.141592653589793 { $9VALUE}; }",a);}
}
G_check :: (s:[] D_State,m:D_Type,p:string) {
  a:[10] string;a[0]=p;a[1]=m.name;a[9]=G_fmt(s,"return $0Check.{error=$0",a[..]);
  G_line(s,"$1__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->$0Check {",a[..]);
  G_line(s,"if !$0message_admitted(depth,wire.count) {return $0Check.{error=$0LIMIT};}",a[..]);
  if D_tighter_message_limit(m.opts.n[1]) {a[5]=G_num(s,cast(s64)m.opts.n[1]);G_line(s,"if wire.count>$5 {return $0Check.{error=$0LIMIT};}",a[..]);}
  G_line(s,"left:=budget;pos:=0;",a[..]);
  for i:m.first..<m.first+m.count {
    f:=s[0].fields[i];a[2]=f.name;
    if f.label==3 {
      if f.opts.n[3]!=0 || f.opts.n[4]!=0 {G_line(s,"count_$2:int=0;",a[..]);}
    } else if f.oneof<0 || f.optional || f.opts.n[4]!=0 {G_line(s,"seen_$2:bool=false;",a[..]);}
  }
  for group:0..<m.group_count {
    real:=false;for i:m.first..<m.first+m.count {f:=s[0].fields[i];if f.oneof==group && !f.optional {real=true;}}
    if real {a[8]=G_num(s,group);G_line(s,"one_$8:bool=false;",a[..]);}
  }
  G_line(s,"while pos<wire.count {",a[..]);
  G_line(s,"if left<=0 {return $0Check.{error=$0LIMIT};} left-=1;",a[..]);
  G_line(s,"f:=$0scan(wire,pos);if f.error!=0 {return $0Check.{error=f.error};} if canonical && !$0minimal(wire,f) {return $0Check.{error=$0WIRE};} pos=f.next;",a[..]);
  for i:m.first..<m.first+m.count {
    f:=s[0].fields[i];G_args(s,m,f,p,a[..]);
    if i==m.first {G_line(s,"if f.number==$5 {",a[..]);}else {G_line(s,"else if f.number==$5 {",a[..]);}
    if f.label!=3 && (f.oneof<0 || f.optional) {G_line(s,"if seen_$2 {$9DUPLICATE};}",a[..]);}
    if f.oneof>=0 && !f.optional {G_line(s,"if one_$8 {$9DUPLICATE};} one_$8=true;",a[..]);}
    if f.label==3 && G_wt(f.kind)!=2 {
      G_line(s,"if f.wt==2 {packed:=f.payload;at:=0;while at<packed.count {",a[..]);
      G_line(s,"if left<=0 {$9LIMIT};} left-=1; f=$0scalar(packed,at,$6);if f.error!=0 {return $0Check.{error=f.error};}",a[..]);
      if G_wt(f.kind)==0 {G_line(s,"if canonical && $0vsize(f.raw)!=f.next-at {$9WIRE};}",a[..]);}
      G_line(s,"at=f.next;",a[..]);G_line(s,"f.wt=$6;",a[..]);G_constraints(s,f,a[..]);
      if f.opts.n[3]!=0 || f.opts.n[4]!=0 {G_line(s,"count_$2+=1;",a[..]);}
      G_line(s,"}} else {",a[..]);G_constraints(s,f,a[..]);
      if f.opts.n[3]!=0 || f.opts.n[4]!=0 {G_line(s,"count_$2+=1;",a[..]);}G_line(s,"}",a[..]);
    }else {
      G_constraints(s,f,a[..]);
      if f.label==3 {
        if f.opts.n[3]!=0 || f.opts.n[4]!=0 {G_line(s,"count_$2+=1;",a[..]);}
      }else if f.oneof<0 || f.optional || f.opts.n[4]!=0 {G_line(s,"seen_$2=true;",a[..]);}
      if f.kind==11 {G_line(s,"child:=$3__check(f.payload,depth+1,left,$0require_fields(),canonical);if child.error!=0 {return $0Check.{error=child.error};} left=child.left;",a[..]);}
    }
    // Descriptor admission rejects max_count on singular fields.
    if f.label==3 && f.opts.n[3]!=0 {a[5]=G_num(s,cast(s64)f.opts.n[3]);G_line(s,"if count_$2>$5 {$9LIMIT};}",a[..]);}
    G_line(s,"}",a[..]);
  }
  G_line(s,"}",a[..]);
  for i:m.first..<m.first+m.count {
    f:=s[0].fields[i];if f.opts.n[4]!=0 {a[2]=f.name;
      if f.label==3 {G_line(s,"if required && count_$2==0 {$9REQUIRED};}",a[..]);}
      else {G_line(s,"if required && !seen_$2 {$9REQUIRED};}",a[..]);}
    }
  }
  G_line(s,"return $0Check.{left=left};}",a[..]);
}
#test "generated message guards only tighten the shared cap" {
  arena:[8192]u8;output:[8192]u8;fields:[1]D_Field;
  for choice:0..<4 {
    state:=[1]D_State.[D_State.{fields=fields[..],arena=arena[..],output=output[..]}];
    m:=D_Type.{name="P_M"};
    if choice==1 {m.opts.n[1]=1;}
    if choice==2 {m.opts.n[1]=1048576;}
    if choice==3 {m.opts.n[1]=1048577;}
    G_check(state[..],m,"P__pb_");#assert state[0].error=="";
    needle:="if wire.count>";found:=false;
    for i:0..<state[0].written {if needle.count<=state[0].written-i && cast(string)output[i..<i+needle.count]==needle {found=true;}}
    #assert found==(choice==1);
  }
}
#test "required repeated and real oneof templates" {
  fields:[4] D_Field;
  fields[0]=D_Field.{name="texts",number=1,label=3,kind=9};fields[0].opts.n[4]=1;
  fields[1]=D_Field.{name="numbers",number=2,label=3,kind=13};fields[1].opts.n[4]=1;
  fields[2]=D_Field.{name="choice",number=3,label=1,kind=9,oneof=0};fields[2].opts.n[4]=1;
  fields[3]=D_Field.{name="blobs",number=4,label=3,kind=12};fields[3].opts.n[3]=2;
  arena:[8192]u8;output:[8192]u8;state:=[1]D_State.[D_State.{fields=fields[..],arena=arena[..],output=output[..]}];
  G_check(state[..],D_Type.{name="P_M",count=4,group_count=1},"P__pb_");
  #assert state[0].error=="" && state[0].written==1973;
  #assert cast(string)output[0..<512]=="P_M__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire.count) {return P__pb_Check.{error=P__pb_LIMIT};}\nleft:=budget;pos:=0;\ncount_texts:int=0;\ncount_numbers:int=0;\nseen_choice:bool=false;\ncount_blobs:int=0;\none_0:bool=false;\nwhile pos<wire.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.error!=0 {return P__pb_Check.{error=f.error};} if canonical && !P__pb_minimal(wire,f) {return P__";
  #assert cast(string)output[512..<1024]=="pb_Check.{error=P__pb_WIRE};} pos=f.next;\nif f.number==1 {\nif f.wt!=2 { return P__pb_Check.{error=P__pb_WIRE}; }\nif !P__pb_utf8(f.payload) { return P__pb_Check.{error=P__pb_UTF8}; }\ncount_texts+=1;\n}\nelse if f.number==2 {\nif f.wt==2 {packed:=f.payload;at:=0;while at<packed.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1; f=P__pb_scalar(packed,at,0);if f.error!=0 {return P__pb_Check.{error=f.error};}\nif canonical && P__pb_vsize(f.raw)!=f.next-at {return P__pb_Check.{error=P__pb_WIRE};}\na";
  #assert cast(string)output[1024..<1536]=="t=f.next;\nf.wt=0;\nif f.wt!=0 { return P__pb_Check.{error=P__pb_WIRE}; }\nif f.raw>4294967295 { return P__pb_Check.{error=P__pb_VALUE}; }\ncount_numbers+=1;\n}} else {\nif f.wt!=0 { return P__pb_Check.{error=P__pb_WIRE}; }\nif f.raw>4294967295 { return P__pb_Check.{error=P__pb_VALUE}; }\ncount_numbers+=1;\n}\n}\nelse if f.number==3 {\nif one_0 {return P__pb_Check.{error=P__pb_DUPLICATE};} one_0=true;\nif f.wt!=2 { return P__pb_Check.{error=P__pb_WIRE}; }\nif !P__pb_utf8(f.payload) { return P__pb_Check.{error=P__pb_UTF8}";
  #assert cast(string)output[1536..<1973]=="; }\nseen_choice=true;\n}\nelse if f.number==4 {\nif f.wt!=2 { return P__pb_Check.{error=P__pb_WIRE}; }\ncount_blobs+=1;\nif count_blobs>2 {return P__pb_Check.{error=P__pb_LIMIT};}\n}\n}\nif required && count_texts==0 {return P__pb_Check.{error=P__pb_REQUIRED};}\nif required && count_numbers==0 {return P__pb_Check.{error=P__pb_REQUIRED};}\nif required && !seen_choice {return P__pb_Check.{error=P__pb_REQUIRED};}\nreturn P__pb_Check.{left=left};}\n";
}
G_getter :: (s:[] D_State,m:D_Type,f:D_Field,p:string) {
  a:[10] string;G_args(s,m,f,p,a[..]);
  G_line(s,"$1_get_$2 :: (msg:$1)->$4 {",a[..]);
  G_line(s,"if msg.error!=0 {return $4.{error=msg.error};}checked:=$1_decode(msg.wire);if checked.error!=0 {return $4.{error=checked.error};}",a[..]);
  G_line(s,"f:=$0find(msg.wire,$5);return $4.{value=$7,present=!f.done,error=f.error};}",a[..]);
}
G_iterator :: (s:[] D_State,m:D_Type,f:D_Field,p:string) {
  a:[10] string;G_args(s,m,f,p,a[..]);a[9]=G_fmt(s,"$1_Iter_$2",a[..]);
  G_line(s,"$9 :: struct {wire:[] u8;cursor:int;packed:[] u8;packed_at:int;value:$3;has_value:bool;done:bool;error:u32;}",a[..]);
  G_line(s,"$1_iter_$2 :: (msg:$1)->$9 {if msg.error!=0 {return $9.{error=msg.error,done=true};}checked:=$1_decode(msg.wire);if checked.error!=0 {return $9.{error=checked.error,done=true};}return $1_next_$2($9.{wire=msg.wire});}",a[..]);
  G_line(s,"$1_next_$2 :: (old:$9)->$9 {iterator:=old;iterator.has_value=false;if iterator.error!=0 || iterator.done {return iterator;}while true {",a[..]);
  if G_wt(f.kind)!=2 {G_line(s,"if iterator.packed_at<iterator.packed.count {f:=$0scalar(iterator.packed,iterator.packed_at,$6);if f.error!=0 {iterator.error=f.error;iterator.done=true;return iterator;}iterator.packed_at=f.next;iterator.value=$7;iterator.has_value=true;return iterator;}",a[..]);}
  G_line(s,"f:=$0scan(iterator.wire,iterator.cursor);if f.error!=0 || f.done {iterator.error=f.error;iterator.done=true;return iterator;}iterator.cursor=f.next;if f.number!=$5 {continue;}",a[..]);
  if G_wt(f.kind)!=2 {G_line(s,"if f.wt==2 {iterator.packed=f.payload;iterator.packed_at=0;continue;}",a[..]);}
  G_line(s,"iterator.value=$7;iterator.has_value=true;return iterator;}return iterator;}",a[..]);
}
G_writer :: (s:[] D_State,m:D_Type,f:D_Field,p:string,clear:bool) {
  a:[10] string;G_args(s,m,f,p,a[..]);a[9]="set";if f.label==3 {a[9]="append";}if clear {a[9]="clear";}
  if clear {G_line(s,"$1_$9_$2 :: (msg:$1,out:[] u8)->$1 {",a[..]);}else {G_line(s,"$1_$9_$2 :: (msg:$1,value:$3,out:[] u8)->$1 {",a[..]);}
  G_line(s,"if msg.error!=0 {return $1.{error=msg.error};} checked:=$1__check(msg.wire,$0root_depth(),$0default_fields(),$0partial_fields(),false);if checked.error!=0 {return $1.{error=checked.error};}",a[..]);
  targets:[256] string;count:=0;
  if clear || !(f.label==3) {
    if clear || f.oneof<0 || f.optional {targets[0]=a[5];count=1;}
    else {for i:m.first..<m.first+m.count {x:=s[0].fields[i];if x.oneof==f.oneof {targets[count]=G_num(s,cast(s64)x.number);count+=1;}}}
  }
  if count==0 {G_line(s,"removed_fields:[] u32;",a[..]);a[9]="removed_fields";}
  else {
    a[9]=G_num(s,count);G_write(s,G_fmt(s,"removed_fields:= [$9] u32.[",a[..]));
    for i:0..<count {if i>0 {G_write(s,",");}G_write(s,targets[i]);}G_write(s,"];\n");a[9]="removed_fields[..]";
  }
  G_line(s,"data:[] u8;txt:=\"\";raw:u64=0;is_text:=false;",a[..]);
  if !clear {
    if f.kind==11 {G_line(s,"if value.error!=0 {return $1.{error=value.error};}child:=$3_decode(value.wire);if child.error!=0 {return $1.{error=child.error};} data=value.wire;",a[..]);}
    else if f.kind==9 {G_line(s,"txt=value;is_text=true;",a[..]);}else if f.kind==12 {G_line(s,"data=value;",a[..]);}
    else if f.kind==1 {G_line(s,"raw=cast(u64) f64_bits(value);",a[..]);}else if f.kind==2 {G_line(s,"raw=cast(u64) f32_bits(value);",a[..]);}
    else if f.kind==17 || f.kind==18 {G_line(s,"raw=$0zigzag(cast(s64)value);",a[..]);}
    else if f.kind==8 {G_line(s,"if value {raw=1;}",a[..]);}
    else if f.kind==3 || f.kind==5 || f.kind==14 || f.kind==15 || f.kind==16 {G_line(s,"raw=cast,trunc(u64) value;",a[..]);}
    else {G_line(s,"raw=cast(u64) value;",a[..]);}
  }
  if clear {G_line(s,"result:=$0rewrite(msg.wire,$9,$5,$6,raw,data,txt,is_text,true,out);",a[..]);}
  else {G_line(s,"result:=$0rewrite(msg.wire,$9,$5,$6,raw,data,txt,is_text,false,out);",a[..]);}
  G_line(s,"if result.error!=0 {return $1.{error=result.error};} final:=$1__check(result.wire,$0root_depth(),$0default_fields(),$0partial_fields(),false);if final.error!=0 {return $1.{error=final.error};}return $1.{wire=result.wire};}",a[..]);
}
G_api :: (s:[] D_State,m:D_Type,p:string) {
  a:[10] string;a[0]=p;a[1]=m.name;
  G_line(s,"$1_new :: ()->$1 {return $1.{};}",a[..]);
  G_line(s,"$1_decode :: (wire:[] u8)->$1 {c:=$1__check(wire,$0root_depth(),$0default_fields(),$0require_fields(),false);if c.error!=0 {return $1.{error=c.error};}return $1.{wire=wire};}",a[..]);
  G_line(s,"$1_decode_minimal :: (wire:[] u8)->$1 {c:=$1__check(wire,$0root_depth(),$0default_fields(),$0require_fields(),true);if c.error!=0 {return $1.{error=c.error};}return $1.{wire=wire};}",a[..]);
  G_line(s,"$1_encode :: (msg:$1,out:[] u8)->$0View {if msg.error!=0 {return $0View.{error=msg.error};} c:=$1__check(msg.wire,$0root_depth(),$0default_fields(),$0require_fields(),false);if c.error!=0 {return $0View.{error=c.error};} return $0copy(msg.wire,out);}",a[..]);
  for g:0..<m.group_count {
    real:=false;for i:m.first..<m.first+m.count {if s[0].fields[i].oneof==g && !s[0].fields[i].optional {real=true;}}
    if real {
      a[8]=s[0].groups[m.groups+g];G_write(s,G_fmt(s,"$1_case_$8 :: (msg:$1)->$0U32 {if msg.error!=0 {return $0U32.{error=msg.error};}checked:=$1_decode(msg.wire);if checked.error!=0 {return $0U32.{error=checked.error};}pos:=0;while pos<msg.wire.count {f:=$0scan(msg.wire,pos);pos=f.next;if ",a[..]));
      first:=true;for i:m.first..<m.first+m.count {f:=s[0].fields[i];if f.oneof==g {if !first {G_write(s," || ");}a[5]=G_num(s,cast(s64)f.number);G_write(s,G_fmt(s,"f.number==$5",a[..]));first=false;}}
      G_line(s," {return $0U32.{value=f.number,present=true};}} return $0U32.{};}",a[..]);
    }
  }
  // Build the known-field condition without runtime allocations.
  conditions:[256] string;
  for i:m.first..<m.first+m.count {a[5]=G_num(s,cast(s64)s[0].fields[i].number);conditions[i-m.first]=G_fmt(s,"f.number==$5",a[..]);}
  start:=s[0].used;
  for i:0..<m.count {if i>0 {parts:=[1] string.[" || "];piece:=G_cat(s,parts[..]);}parts:=[1] string.[conditions[i]];piece:=G_cat(s,parts[..]);}
  condition:=cast(string)s[0].arena[start..<s[0].used];if m.count==0 {condition="false";}
  a[9]=condition;G_line(s,"$1_unknown :: (msg:$1,cursor:int)->$0Field {if msg.error!=0 {return $0Field.{error=msg.error};}checked:=$1_decode(msg.wire);if checked.error!=0 {return $0Field.{error=checked.error};}pos:=cursor;while true {f:=$0scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !($9) {return f;}}return $0Field.{done=true};}",a[..]);
  for i:m.first..<m.first+m.count {f:=s[0].fields[i];G_args(s,m,f,p,a[..]);G_line(s,"$1_FIELD_$2 :: $5;",a[..]);if f.label==3 {G_iterator(s,m,f,p);}else {G_getter(s,m,f,p);}G_writer(s,m,f,p,false);G_writer(s,m,f,p,true);}
}
// PB_ is a reserved substitution marker in the owned runtime template.
// The build checks exact emitted runtime bytes; it is not user-supplied code.
G_runtime :: (s:[] D_State,text:string,prefix:string) {
  at:=0;while at<text.count && s[0].error=="" {
    if at+3<=text.count && text[at..<at+3]=="PB_" {G_write(s,prefix);at+=3;}
    else {G_write(s,text[at..<at+1]);at+=1;}
  }G_write(s,"\n");
}
G_collisions :: (s:[] D_State) {
  text:=cast(string)s[0].output[0..<s[0].written];at:=0;
  while at<text.count {
    end:=at;if D_alpha(text[at]) {while end<text.count && (D_alpha(text[end]) || D_digit(text[end])) {end+=1;}
      if end+3<=text.count && text[end..<end+3]==" ::" {D_symbol(s,text[at..<end]);}}
    while end<text.count && text[end]!=10 {end+=1;}at=end+1;
  }
}
G_generate :: (s:[] D_State,raw:string,prefix:string,runtime:string)->int #export_c {
  D_parse(s,raw,prefix);if s[0].error!="" {return 0;}
  parts:=[2] string.[prefix,"__pb_"];p:=G_cat(s,parts[..]);G_runtime(s,runtime,p);
  empty:[] string;G_line(s,"// @proto-descriptor-declarations",empty);
  for i:0..<s[0].ne {e:=s[0].enums[i];a:[10] string;a[1]=e.name;
    for j:e.first..<e.first+e.count {a[2]=s[0].values[j].name;a[5]=G_num(s,s[0].values[j].number);G_line(s,"$1_$2 :: $5;",a[..]);}
    G_write(s,G_fmt(s,"$1_known :: (value:s32)->bool {return ",a[..]));
    for j:e.first..<e.first+e.count {if j>e.first {G_write(s," || ");}a[5]=G_num(s,s[0].values[j].number);G_write(s,G_fmt(s,"value==$5",a[..]));}G_write(s,";}\n");
  }
  for i:0..<s[0].nt {m:=s[0].types[i];a:=[2] string.[m.full,m.name];G_line(s,"// $0 -> $1",a[..]);G_line(s,"$1 :: struct {wire:[] u8;error:u32;}",a[..]);G_line(s,"$1_Option :: struct {value:$1;present:bool;error:u32;}",a[..]);}
  for i:0..<s[0].nt {G_check(s,s[0].types[i],p);G_api(s,s[0].types[i],p);}
  if s[0].error!="" {return 0;}G_collisions(s);if s[0].error!="" {return 0;}return s[0].written;
}

#test "reject empty descriptor" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:[1] u8;
  D_parse(state[..],cast(string)fixture[0..<0],"P");
  #assert state[0].error=="no runtime messages in descriptor";
}
#test "reject syntax" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[44] u8.[10,42,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,98,6,112,114,111,116,111,50];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="only proto3 data descriptors are supported";
}
#test "reject file extensions" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[46] u8.[10,44,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,56,0,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="only proto3 data descriptors are supported";
}
#test "reject file editions" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[46] u8.[10,44,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,112,0,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="only proto3 data descriptors are supported";
}
#test "reject file empty name" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[34] u8.[10,32,10,0,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid descriptor file name";
}
#test "reject file nul name" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[37] u8.[10,35,10,3,97,0,98,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid descriptor file name";
}
#test "reject file name too long" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[4133] u8.[10,162,32,10,129,32,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid descriptor file name";
}
#test "reject duplicate file" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[88] u8.[10,42,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,98,6,112,114,111,116,111,51,10,42,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="duplicate descriptor file";
}
#test "reject missing dependency" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[53] u8.[10,51,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,26,7,109,105,115,115,105,110,103,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="missing descriptor file dependency";
}
#test "reject bad package 9" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[44] u8.[10,42,10,10,116,101,115,116,46,112,114,111,116,111,18,4,46,98,97,100,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/oversized package";
}
#test "reject bad package 10" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[44] u8.[10,42,10,10,116,101,115,116,46,112,114,111,116,111,18,4,98,97,100,46,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/oversized package";
}
#test "reject bad package 11" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[46] u8.[10,44,10,10,116,101,115,116,46,112,114,111,116,111,18,6,98,97,100,46,46,120,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/oversized package";
}
#test "reject bad package 12" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[105] u8.[10,103,10,10,116,101,115,116,46,112,114,111,116,111,18,65,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,46,120,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/oversized package";
}
#test "reject bad package 13" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[299] u8.[10,168,2,10,10,116,101,115,116,46,112,114,111,116,111,18,129,2,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/oversized package";
}
#test "reject bad package 14" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[4139] u8.[10,168,32,10,10,116,101,115,116,46,112,114,111,116,111,18,129,32,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/oversized package";
}
#test "reject message identifier 15" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[32] u8.[10,30,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,2,10,0,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/oversized descriptor identifier";
}
#test "reject message identifier 16" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[37] u8.[10,35,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,7,10,5,48,110,97,109,101,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/oversized descriptor identifier";
}
#test "reject message identifier 17" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[37] u8.[10,35,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,7,10,5,110,97,45,109,101,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/oversized descriptor identifier";
}
#test "reject message identifier 18" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[292] u8.[10,161,2,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,132,2,10,129,2,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/oversized descriptor identifier";
}
#test "reject unsupported message 19" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[35] u8.[10,33,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,5,10,1,77,40,0,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="maps/message extensions are not supported";
}
#test "reject unsupported message 20" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[35] u8.[10,33,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,5,10,1,77,48,0,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="maps/message extensions are not supported";
}
#test "reject unsupported message 21" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[37] u8.[10,35,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,7,10,1,77,58,2,56,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="maps/message extensions are not supported";
}
#test "reject bad field shape 22" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[44] u8.[10,42,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,0,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported field shape";
}
#test "reject bad field shape 23" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[44] u8.[10,42,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,19,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported field shape";
}
#test "reject bad field shape 24" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[44] u8.[10,42,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,10,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported field shape";
}
#test "reject bad field shape 25" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[44] u8.[10,42,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,0,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid field number";
}
#test "reject bad field shape 26" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[46] u8.[10,44,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,16,10,1,77,18,11,10,1,120,24,184,148,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid field number";
}
#test "reject bad field shape 27" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[46] u8.[10,44,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,16,10,1,77,18,11,10,1,120,24,159,156,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid field number";
}
#test "reject bad field shape 28" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[48] u8.[10,46,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,18,10,1,77,18,13,10,1,120,24,128,128,128,128,2,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid field number";
}
#test "reject bad field shape 29" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[44] u8.[10,42,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,2,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported field shape";
}
#test "reject bad field shape 30" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[44] u8.[10,42,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,0,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported field shape";
}
#test "reject bad field shape 31" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[44] u8.[10,42,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,4,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported field shape";
}
#test "reject bad field metadata 32" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[54] u8.[10,52,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,24,10,1,77,18,19,10,1,120,24,1,32,1,40,5,18,8,101,120,116,101,110,100,101,101,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported field shape";
}
#test "reject bad field metadata 33" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[53] u8.[10,51,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,23,10,1,77,18,18,10,1,120,24,1,32,1,40,5,58,7,100,101,102,97,117,108,116,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported field shape";
}
#test "reject bad field metadata 34" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[46] u8.[10,44,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,16,10,1,77,18,11,10,1,120,24,1,32,1,40,5,72,0,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid oneof";
}
#test "reject bad field metadata 35" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[47] u8.[10,45,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,17,10,1,77,18,12,10,1,120,24,1,32,1,40,5,136,1,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid proto3 optional flag";
}
#test "reject bad field metadata 36" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[47] u8.[10,45,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,17,10,1,77,18,12,10,1,120,24,1,32,1,40,5,136,1,2,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid proto3 optional flag";
}
#test "reject bad field metadata 37" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[46] u8.[10,44,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,16,10,1,77,18,11,10,1,120,24,1,32,1,40,5,32,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="duplicate descriptor metadata field";
}
#test "reject bad field metadata 38" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[50] u8.[10,48,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,20,10,1,77,18,15,10,1,120,24,1,32,1,40,5,66,4,248,243,24,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="descriptor bytes wire type";
}
#test "reject bad field metadata 39" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[55] u8.[10,53,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,25,10,1,77,18,20,10,1,120,24,1,32,1,40,5,66,9,250,243,24,5,119,114,111,110,103,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported grammar option";
}
#test "reject bad field metadata 40" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[50] u8.[10,48,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,20,10,1,77,18,15,10,1,120,24,1,32,1,40,5,66,4,208,243,24,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid policy option for field kind";
}
#test "reject bad field metadata 41" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[50] u8.[10,48,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,20,10,1,77,18,15,10,1,120,24,1,32,1,40,5,66,4,216,243,24,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid policy option for field kind";
}
#test "reject bad field metadata 42" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[65] u8.[10,63,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,35,10,1,77,18,30,10,1,120,24,1,32,1,40,5,66,19,250,243,24,15,110,111,110,101,109,112,116,121,45,110,111,45,110,117,108,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid policy option for field kind";
}
#test "reject bad field metadata 43" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[50] u8.[10,48,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,20,10,1,77,18,15,10,1,120,24,1,32,1,40,5,66,4,128,244,24,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid policy option for field kind";
}
#test "reject duplicate field number" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[55] u8.[10,53,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,25,10,1,77,18,9,10,1,120,24,1,32,1,40,5,18,9,10,1,121,24,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/duplicate field";
}
#test "reject duplicate field name" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[55] u8.[10,53,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,25,10,1,77,18,9,10,1,120,24,1,32,1,40,5,18,9,10,1,120,24,2,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/duplicate field";
}
#test "reject invalid string field name" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[46] u8.[10,44,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,16,10,1,77,18,11,10,3,97,45,98,24,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/oversized descriptor identifier";
}
#test "reject wrong type kind" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[68] u8.[10,66,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,23,10,1,77,18,18,10,1,120,24,1,32,1,40,11,50,7,46,116,101,115,116,46,69,42,13,10,1,69,18,8,10,4,90,69,82,79,16,0,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="missing descriptor dependency or type mismatch";
}
#test "reject missing type" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[59] u8.[10,57,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,29,10,1,77,18,24,10,1,120,24,1,32,1,40,11,50,13,46,116,101,115,116,46,77,105,115,115,105,110,103,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="missing descriptor dependency or type mismatch";
}
#test "reject scalar type_name" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[53] u8.[10,51,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,23,10,1,77,18,18,10,1,120,24,1,32,1,40,5,50,7,46,116,101,115,116,46,77,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="scalar has type_name";
}
#test "reject message unsupported policy 999" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[38] u8.[10,36,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,8,10,1,77,58,3,184,62,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported policy option";
}
#test "reject message unsupported policy 50999" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[39] u8.[10,37,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,9,10,1,77,58,4,184,243,24,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported policy option";
}
#test "reject message unsupported policy 51010" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[39] u8.[10,37,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,9,10,1,77,58,4,144,244,24,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported policy option";
}
#test "reject message unsupported policy 51002" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[39] u8.[10,37,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,9,10,1,77,58,4,208,243,24,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported or duplicate descriptor option";
}
#test "reject message malformed option 54" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[43] u8.[10,41,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,13,10,1,77,58,8,192,243,24,1,192,243,24,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported or duplicate descriptor option";
}
#test "reject message malformed option 55" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[40] u8.[10,38,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,10,10,1,77,58,5,202,243,24,1,120,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid numeric policy option";
}
#test "reject message malformed option 56" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[48] u8.[10,46,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,18,10,1,77,58,13,200,243,24,128,128,128,128,128,128,128,128,128,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid numeric policy option";
}
#test "reject message malformed option 57" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[40] u8.[10,38,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,10,10,1,77,58,5,242,243,24,1,255,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/oversized descriptor UTF-8 string";
}
#test "reject message malformed option 58" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[39] u8.[10,37,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,9,10,1,77,58,4,240,243,24,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="descriptor bytes wire type";
}
#test "reject message malformed option 59" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[39] u8.[10,37,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,9,10,1,77,58,4,8,1,8,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="duplicate descriptor option";
}
#test "reject field malformed option 60" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[50] u8.[10,48,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,20,10,1,77,18,15,10,1,120,24,1,32,1,40,5,66,4,224,243,24,2,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid numeric policy option";
}
#test "reject field malformed option 61" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[51] u8.[10,49,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,21,10,1,77,18,16,10,1,120,24,1,32,1,40,5,66,5,226,243,24,1,120,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid numeric policy option";
}
#test "reject field malformed option 62" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[59] u8.[10,57,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,29,10,1,77,18,24,10,1,120,24,1,32,1,40,5,66,13,224,243,24,128,128,128,128,128,128,128,128,128,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid numeric policy option";
}
#test "reject field malformed option 63" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[50] u8.[10,48,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,20,10,1,77,18,15,10,1,120,24,1,32,1,40,5,66,4,192,243,24,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="unsupported or duplicate descriptor option";
}
#test "reject field malformed option 64" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[51] u8.[10,49,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,21,10,1,77,18,16,10,1,120,24,1,32,1,40,5,66,5,234,243,24,1,255,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid/oversized descriptor UTF-8 string";
}
#test "reject field malformed option 65" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[50] u8.[10,48,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,20,10,1,77,18,15,10,1,120,24,1,32,1,40,5,66,4,232,243,24,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="descriptor bytes wire type";
}
#test "reject enum malformed 66" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[49] u8.[10,47,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,42,3,10,1,69,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="proto3 enum must start at zero";
}
#test "reject enum malformed 67" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[57] u8.[10,55,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,42,11,10,1,69,18,6,10,4,90,69,82,79,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="enum value outside int32";
}
#test "reject enum malformed 68" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[56] u8.[10,54,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,42,10,10,1,69,18,5,10,1,65,16,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="proto3 enum must start at zero";
}
#test "reject enum malformed 69" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[69] u8.[10,67,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,42,23,10,1,69,18,8,10,4,90,69,82,79,16,0,18,8,10,4,90,69,82,79,16,1,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="duplicate enum value name";
}
#test "reject enum malformed 70" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[72] u8.[10,70,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,42,26,10,1,69,18,8,10,4,90,69,82,79,16,0,18,11,10,3,66,65,68,16,128,128,128,128,8,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="enum value outside int32";
}
#test "reject duplicate type" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[60] u8.[10,58,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="generated name collision; use narrower profiles";
}
#test "reject message enum same name" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[59] u8.[10,57,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,42,13,10,1,77,18,8,10,4,90,69,82,79,16,0,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="generated name collision; use narrower profiles";
}
#test "reject duplicate enums" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[74] u8.[10,72,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,42,13,10,1,69,18,8,10,4,90,69,82,79,16,0,42,13,10,1,69,18,8,10,4,90,69,82,79,16,0,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="generated name collision; use narrower profiles";
}
#test "reject duplicate oneof metadata" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[72] u8.[10,70,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,42,10,1,77,18,14,10,1,120,24,1,32,1,40,5,72,0,136,1,1,18,11,10,1,121,24,2,32,1,40,5,72,0,66,8,10,6,99,104,111,105,99,101,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid synthetic oneof";
}
#test "reject repeated oneof" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[4096] u8;output:[1] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[56] u8.[10,54,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,26,10,1,77,18,11,10,1,120,24,1,32,3,40,5,72,0,66,8,10,6,99,104,111,105,99,101,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid oneof";
}
#test "policy templates" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[262144] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[165] u8.[10,162,1,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,133,1,10,1,77,58,19,192,243,24,1,200,243,24,128,8,242,243,24,6,112,111,108,105,99,121,18,46,10,5,108,97,98,101,108,24,1,32,1,40,9,66,31,208,243,24,32,224,243,24,1,250,243,24,15,110,111,110,101,109,112,116,121,45,110,111,45,110,117,108,128,244,24,4,18,23,10,2,120,115,24,2,32,3,40,1,66,11,216,243,24,3,234,243,24,3,114,97,100,18,19,10,2,121,115,24,3,32,3,40,2,66,7,234,243,24,3,114,97,100,18,13,10,5,110,97,109,101,115,24,4,32,3,40,9,98,6,112,114,111,116,111,51];
  n:=G_generate(state[..],cast(string)fixture[..],"P","");
  #assert state[0].error=="" && n==12696;
  #assert cast(string)output[0..<512]=="\n// @proto-descriptor-declarations\n// .test.M -> P_M\nP_M :: struct {wire:[] u8;error:u32;}\nP_M_Option :: struct {value:P_M;present:bool;error:u32;}\nP_M__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire.count) {return P__pb_Check.{error=P__pb_LIMIT};}\nif wire.count>1024 {return P__pb_Check.{error=P__pb_LIMIT};}\nleft:=budget;pos:=0;\nseen_label:bool=false;\ncount_xs:int=0;\nwhile pos<wire.count {\nif left<=0 {return P__pb_Check.{error=P__";
  #assert cast(string)output[512..<1024]=="pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.error!=0 {return P__pb_Check.{error=f.error};} if canonical && !P__pb_minimal(wire,f) {return P__pb_Check.{error=P__pb_WIRE};} pos=f.next;\nif f.number==1 {\nif seen_label {return P__pb_Check.{error=P__pb_DUPLICATE};}\nif f.wt!=2 { return P__pb_Check.{error=P__pb_WIRE}; }\nif !P__pb_utf8(f.payload) { return P__pb_Check.{error=P__pb_UTF8}; }\nif f.payload.count>32 { return P__pb_Check.{error=P__pb_LIMIT}; }\nif f.payload.count!=4 { return P__pb_Check.{error=P__pb_L";
  #assert cast(string)output[1024..<1536]=="IMIT}; }\nif !P__pb_no_nul(f.payload) { return P__pb_Check.{error=P__pb_VALUE}; }\nseen_label=true;\n}\nelse if f.number==2 {\nif f.wt==2 {packed:=f.payload;at:=0;while at<packed.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1; f=P__pb_scalar(packed,at,1);if f.error!=0 {return P__pb_Check.{error=f.error};}\nat=f.next;\nf.wt=1;\nif f.wt!=1 { return P__pb_Check.{error=P__pb_WIRE}; }\nx:=f64_from_bits(f.raw);if x!=x || x < -3.141592653589793 || x>3.141592653589793 { return P__pb_Check.{error=P__pb_";
  #assert cast(string)output[1536..<2048]=="VALUE}; }\ncount_xs+=1;\n}} else {\nif f.wt!=1 { return P__pb_Check.{error=P__pb_WIRE}; }\nx:=f64_from_bits(f.raw);if x!=x || x < -3.141592653589793 || x>3.141592653589793 { return P__pb_Check.{error=P__pb_VALUE}; }\ncount_xs+=1;\n}\nif count_xs>3 {return P__pb_Check.{error=P__pb_LIMIT};}\n}\nelse if f.number==3 {\nif f.wt==2 {packed:=f.payload;at:=0;while at<packed.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1; f=P__pb_scalar(packed,at,5);if f.error!=0 {return P__pb_Check.{error=f.error};}\nat=";
  #assert cast(string)output[2048..<2560]=="f.next;\nf.wt=5;\nif f.wt!=5 { return P__pb_Check.{error=P__pb_WIRE}; }\nx:=f32_from_bits(cast(u32) f.raw);if x!=x || x < -3.141592653589793 || x>3.141592653589793 { return P__pb_Check.{error=P__pb_VALUE}; }\n}} else {\nif f.wt!=5 { return P__pb_Check.{error=P__pb_WIRE}; }\nx:=f32_from_bits(cast(u32) f.raw);if x!=x || x < -3.141592653589793 || x>3.141592653589793 { return P__pb_Check.{error=P__pb_VALUE}; }\n}\n}\nelse if f.number==4 {\nif f.wt!=2 { return P__pb_Check.{error=P__pb_WIRE}; }\nif !P__pb_utf8(f.payload) { ";
  #assert cast(string)output[2560..<3072]=="return P__pb_Check.{error=P__pb_UTF8}; }\n}\n}\nif required && !seen_label {return P__pb_Check.{error=P__pb_REQUIRED};}\nreturn P__pb_Check.{left=left};}\nP_M_new :: ()->P_M {return P_M.{};}\nP_M_decode :: (wire:[] u8)->P_M {c:=P_M__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P_M.{error=c.error};}return P_M.{wire=wire};}\nP_M_decode_minimal :: (wire:[] u8)->P_M {c:=P_M__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),true);if ";
  #assert cast(string)output[3072..<3584]=="c.error!=0 {return P_M.{error=c.error};}return P_M.{wire=wire};}\nP_M_encode :: (msg:P_M,out:[] u8)->P__pb_View {if msg.error!=0 {return P__pb_View.{error=msg.error};} c:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P__pb_View.{error=c.error};} return P__pb_copy(msg.wire,out);}\nP_M_unknown :: (msg:P_M,cursor:int)->P__pb_Field {if msg.error!=0 {return P__pb_Field.{error=msg.error};}checked:=P_M_decode(msg.wire);if checked.error!=0 {return P_";
  #assert cast(string)output[3584..<4096]=="_pb_Field.{error=checked.error};}pos:=cursor;while true {f:=P__pb_scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(f.number==1 || f.number==2 || f.number==3 || f.number==4) {return f;}}return P__pb_Field.{done=true};}\nP_M_FIELD_label :: 1;\nP_M_get_label :: (msg:P_M)->P__pb_String {\nif msg.error!=0 {return P__pb_String.{error=msg.error};}checked:=P_M_decode(msg.wire);if checked.error!=0 {return P__pb_String.{error=checked.error};}\nf:=P__pb_find(msg.wire,1);return P__pb_String.{value=cast";
  #assert cast(string)output[4096..<4608]=="(string) f.payload,present=!f.done,error=f.error};}\nP_M_set_label :: (msg:P_M,value:string,out:[] u8)->P_M {\nif msg.error!=0 {return P_M.{error=msg.error};} checked:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_M.{error=checked.error};}\nremoved_fields:= [1] u32.[1];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\ntxt=value;is_text=true;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],1,2,raw,data,txt,is_text,false,out);\nif result.er";
  #assert cast(string)output[4608..<5120]=="ror!=0 {return P_M.{error=result.error};} final:=P_M__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_M.{error=final.error};}return P_M.{wire=result.wire};}\nP_M_clear_label :: (msg:P_M,out:[] u8)->P_M {\nif msg.error!=0 {return P_M.{error=msg.error};} checked:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_M.{error=checked.error};}\nremoved_fields:= [1] u32.[1];\ndata";
  #assert cast(string)output[5120..<5632]==":[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],1,2,raw,data,txt,is_text,true,out);\nif result.error!=0 {return P_M.{error=result.error};} final:=P_M__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_M.{error=final.error};}return P_M.{wire=result.wire};}\nP_M_FIELD_xs :: 2;\nP_M_Iter_xs :: struct {wire:[] u8;cursor:int;packed:[] u8;packed_at:int;value:f64;has_value:bool;done:bool;error:u32;}\nP_M_iter";
  #assert cast(string)output[5632..<6144]=="_xs :: (msg:P_M)->P_M_Iter_xs {if msg.error!=0 {return P_M_Iter_xs.{error=msg.error,done=true};}checked:=P_M_decode(msg.wire);if checked.error!=0 {return P_M_Iter_xs.{error=checked.error,done=true};}return P_M_next_xs(P_M_Iter_xs.{wire=msg.wire});}\nP_M_next_xs :: (old:P_M_Iter_xs)->P_M_Iter_xs {iterator:=old;iterator.has_value=false;if iterator.error!=0 || iterator.done {return iterator;}while true {\nif iterator.packed_at<iterator.packed.count {f:=P__pb_scalar(iterator.packed,iterator.packed_at,1);if f.erro";
  #assert cast(string)output[6144..<6656]=="r!=0 {iterator.error=f.error;iterator.done=true;return iterator;}iterator.packed_at=f.next;iterator.value=f64_from_bits(f.raw);iterator.has_value=true;return iterator;}\nf:=P__pb_scan(iterator.wire,iterator.cursor);if f.error!=0 || f.done {iterator.error=f.error;iterator.done=true;return iterator;}iterator.cursor=f.next;if f.number!=2 {continue;}\nif f.wt==2 {iterator.packed=f.payload;iterator.packed_at=0;continue;}\niterator.value=f64_from_bits(f.raw);iterator.has_value=true;return iterator;}return iterator;}";
  #assert cast(string)output[6656..<7168]=="\nP_M_append_xs :: (msg:P_M,value:f64,out:[] u8)->P_M {\nif msg.error!=0 {return P_M.{error=msg.error};} checked:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_M.{error=checked.error};}\nremoved_fields:[] u32;\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast(u64) f64_bits(value);\nresult:=P__pb_rewrite(msg.wire,removed_fields,2,1,raw,data,txt,is_text,false,out);\nif result.error!=0 {return P_M.{error=result.error};} final:=P_M__chec";
  #assert cast(string)output[7168..<7680]=="k(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_M.{error=final.error};}return P_M.{wire=result.wire};}\nP_M_clear_xs :: (msg:P_M,out:[] u8)->P_M {\nif msg.error!=0 {return P_M.{error=msg.error};} checked:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_M.{error=checked.error};}\nremoved_fields:= [1] u32.[2];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=P__pb_rewrit";
  #assert cast(string)output[7680..<8192]=="e(msg.wire,removed_fields[..],2,1,raw,data,txt,is_text,true,out);\nif result.error!=0 {return P_M.{error=result.error};} final:=P_M__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_M.{error=final.error};}return P_M.{wire=result.wire};}\nP_M_FIELD_ys :: 3;\nP_M_Iter_ys :: struct {wire:[] u8;cursor:int;packed:[] u8;packed_at:int;value:f32;has_value:bool;done:bool;error:u32;}\nP_M_iter_ys :: (msg:P_M)->P_M_Iter_ys {if msg.error!=0 {return P_M_It";
  #assert cast(string)output[8192..<8704]=="er_ys.{error=msg.error,done=true};}checked:=P_M_decode(msg.wire);if checked.error!=0 {return P_M_Iter_ys.{error=checked.error,done=true};}return P_M_next_ys(P_M_Iter_ys.{wire=msg.wire});}\nP_M_next_ys :: (old:P_M_Iter_ys)->P_M_Iter_ys {iterator:=old;iterator.has_value=false;if iterator.error!=0 || iterator.done {return iterator;}while true {\nif iterator.packed_at<iterator.packed.count {f:=P__pb_scalar(iterator.packed,iterator.packed_at,5);if f.error!=0 {iterator.error=f.error;iterator.done=true;return iterat";
  #assert cast(string)output[8704..<9216]=="or;}iterator.packed_at=f.next;iterator.value=f32_from_bits(cast(u32) f.raw);iterator.has_value=true;return iterator;}\nf:=P__pb_scan(iterator.wire,iterator.cursor);if f.error!=0 || f.done {iterator.error=f.error;iterator.done=true;return iterator;}iterator.cursor=f.next;if f.number!=3 {continue;}\nif f.wt==2 {iterator.packed=f.payload;iterator.packed_at=0;continue;}\niterator.value=f32_from_bits(cast(u32) f.raw);iterator.has_value=true;return iterator;}return iterator;}\nP_M_append_ys :: (msg:P_M,value:f32,out:";
  #assert cast(string)output[9216..<9728]=="[] u8)->P_M {\nif msg.error!=0 {return P_M.{error=msg.error};} checked:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_M.{error=checked.error};}\nremoved_fields:[] u32;\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast(u64) f32_bits(value);\nresult:=P__pb_rewrite(msg.wire,removed_fields,3,5,raw,data,txt,is_text,false,out);\nif result.error!=0 {return P_M.{error=result.error};} final:=P_M__check(result.wire,P__pb_root_depth(),P__pb_de";
  #assert cast(string)output[9728..<10240]=="fault_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_M.{error=final.error};}return P_M.{wire=result.wire};}\nP_M_clear_ys :: (msg:P_M,out:[] u8)->P_M {\nif msg.error!=0 {return P_M.{error=msg.error};} checked:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_M.{error=checked.error};}\nremoved_fields:= [1] u32.[3];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],3,5,raw,dat";
  #assert cast(string)output[10240..<10752]=="a,txt,is_text,true,out);\nif result.error!=0 {return P_M.{error=result.error};} final:=P_M__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_M.{error=final.error};}return P_M.{wire=result.wire};}\nP_M_FIELD_names :: 4;\nP_M_Iter_names :: struct {wire:[] u8;cursor:int;packed:[] u8;packed_at:int;value:string;has_value:bool;done:bool;error:u32;}\nP_M_iter_names :: (msg:P_M)->P_M_Iter_names {if msg.error!=0 {return P_M_Iter_names.{error=msg.error,";
  #assert cast(string)output[10752..<11264]=="done=true};}checked:=P_M_decode(msg.wire);if checked.error!=0 {return P_M_Iter_names.{error=checked.error,done=true};}return P_M_next_names(P_M_Iter_names.{wire=msg.wire});}\nP_M_next_names :: (old:P_M_Iter_names)->P_M_Iter_names {iterator:=old;iterator.has_value=false;if iterator.error!=0 || iterator.done {return iterator;}while true {\nf:=P__pb_scan(iterator.wire,iterator.cursor);if f.error!=0 || f.done {iterator.error=f.error;iterator.done=true;return iterator;}iterator.cursor=f.next;if f.number!=4 {contin";
  #assert cast(string)output[11264..<11776]=="ue;}\niterator.value=cast(string) f.payload;iterator.has_value=true;return iterator;}return iterator;}\nP_M_append_names :: (msg:P_M,value:string,out:[] u8)->P_M {\nif msg.error!=0 {return P_M.{error=msg.error};} checked:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_M.{error=checked.error};}\nremoved_fields:[] u32;\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\ntxt=value;is_text=true;\nresult:=P__pb_rewrite(msg.wire,removed_fields,4,2,raw";
  #assert cast(string)output[11776..<12288]==",data,txt,is_text,false,out);\nif result.error!=0 {return P_M.{error=result.error};} final:=P_M__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_M.{error=final.error};}return P_M.{wire=result.wire};}\nP_M_clear_names :: (msg:P_M,out:[] u8)->P_M {\nif msg.error!=0 {return P_M.{error=msg.error};} checked:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_M.{error=checked.e";
  #assert cast(string)output[12288..<12696]=="rror};}\nremoved_fields:= [1] u32.[4];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],4,2,raw,data,txt,is_text,true,out);\nif result.error!=0 {return P_M.{error=result.error};} final:=P_M__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_M.{error=final.error};}return P_M.{wire=result.wire};}\n";
}
#test "empty message" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[262144] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[33] u8.[10,31,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,3,10,1,77,98,6,112,114,111,116,111,51];
  n:=G_generate(state[..],cast(string)fixture[..],"P","");
  #assert state[0].error=="" && n==1698;
  #assert cast(string)output[0..<512]=="\n// @proto-descriptor-declarations\n// .test.M -> P_M\nP_M :: struct {wire:[] u8;error:u32;}\nP_M_Option :: struct {value:P_M;present:bool;error:u32;}\nP_M__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire.count) {return P__pb_Check.{error=P__pb_LIMIT};}\nleft:=budget;pos:=0;\nwhile pos<wire.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.error!=0 {return P__pb_Check.{error=f.error};} if ";
  #assert cast(string)output[512..<1024]=="canonical && !P__pb_minimal(wire,f) {return P__pb_Check.{error=P__pb_WIRE};} pos=f.next;\n}\nreturn P__pb_Check.{left=left};}\nP_M_new :: ()->P_M {return P_M.{};}\nP_M_decode :: (wire:[] u8)->P_M {c:=P_M__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P_M.{error=c.error};}return P_M.{wire=wire};}\nP_M_decode_minimal :: (wire:[] u8)->P_M {c:=P_M__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),true);if c.error!=0 {return P_M.{er";
  #assert cast(string)output[1024..<1536]=="ror=c.error};}return P_M.{wire=wire};}\nP_M_encode :: (msg:P_M,out:[] u8)->P__pb_View {if msg.error!=0 {return P__pb_View.{error=msg.error};} c:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P__pb_View.{error=c.error};} return P__pb_copy(msg.wire,out);}\nP_M_unknown :: (msg:P_M,cursor:int)->P__pb_Field {if msg.error!=0 {return P__pb_Field.{error=msg.error};}checked:=P_M_decode(msg.wire);if checked.error!=0 {return P__pb_Field.{error=checked.e";
  #assert cast(string)output[1536..<1698]=="rror};}pos:=cursor;while true {f:=P__pb_scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(false) {return f;}}return P__pb_Field.{done=true};}\n";
}
#test "namespaced collisions" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[262144] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[52] u8.[10,24,10,1,97,18,3,111,110,101,34,6,10,4,73,116,101,109,98,6,112,114,111,116,111,51,10,24,10,1,98,18,3,116,119,111,34,6,10,4,73,116,101,109,98,6,112,114,111,116,111,51];
  n:=G_generate(state[..],cast(string)fixture[..],"P","");
  #assert state[0].error=="" && n==3701;
  #assert cast(string)output[0..<512]=="\n// @proto-descriptor-declarations\n// .one.Item -> P_one_Item\nP_one_Item :: struct {wire:[] u8;error:u32;}\nP_one_Item_Option :: struct {value:P_one_Item;present:bool;error:u32;}\n// .two.Item -> P_two_Item\nP_two_Item :: struct {wire:[] u8;error:u32;}\nP_two_Item_Option :: struct {value:P_two_Item;present:bool;error:u32;}\nP_one_Item__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire.count) {return P__pb_Check.{error=P__pb_LIMIT};}\nleft:";
  #assert cast(string)output[512..<1024]=="=budget;pos:=0;\nwhile pos<wire.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.error!=0 {return P__pb_Check.{error=f.error};} if canonical && !P__pb_minimal(wire,f) {return P__pb_Check.{error=P__pb_WIRE};} pos=f.next;\n}\nreturn P__pb_Check.{left=left};}\nP_one_Item_new :: ()->P_one_Item {return P_one_Item.{};}\nP_one_Item_decode :: (wire:[] u8)->P_one_Item {c:=P_one_Item__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.e";
  #assert cast(string)output[1024..<1536]=="rror!=0 {return P_one_Item.{error=c.error};}return P_one_Item.{wire=wire};}\nP_one_Item_decode_minimal :: (wire:[] u8)->P_one_Item {c:=P_one_Item__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),true);if c.error!=0 {return P_one_Item.{error=c.error};}return P_one_Item.{wire=wire};}\nP_one_Item_encode :: (msg:P_one_Item,out:[] u8)->P__pb_View {if msg.error!=0 {return P__pb_View.{error=msg.error};} c:=P_one_Item__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_";
  #assert cast(string)output[1536..<2048]=="fields(),false);if c.error!=0 {return P__pb_View.{error=c.error};} return P__pb_copy(msg.wire,out);}\nP_one_Item_unknown :: (msg:P_one_Item,cursor:int)->P__pb_Field {if msg.error!=0 {return P__pb_Field.{error=msg.error};}checked:=P_one_Item_decode(msg.wire);if checked.error!=0 {return P__pb_Field.{error=checked.error};}pos:=cursor;while true {f:=P__pb_scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(false) {return f;}}return P__pb_Field.{done=true};}\nP_two_Item__check :: (wire:[] u8,dept";
  #assert cast(string)output[2048..<2560]=="h:int,budget:int,required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire.count) {return P__pb_Check.{error=P__pb_LIMIT};}\nleft:=budget;pos:=0;\nwhile pos<wire.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.error!=0 {return P__pb_Check.{error=f.error};} if canonical && !P__pb_minimal(wire,f) {return P__pb_Check.{error=P__pb_WIRE};} pos=f.next;\n}\nreturn P__pb_Check.{left=left};}\nP_two_Item_new :: ()->P_two_Item {return P_two_Item.{}";
  #assert cast(string)output[2560..<3072]==";}\nP_two_Item_decode :: (wire:[] u8)->P_two_Item {c:=P_two_Item__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P_two_Item.{error=c.error};}return P_two_Item.{wire=wire};}\nP_two_Item_decode_minimal :: (wire:[] u8)->P_two_Item {c:=P_two_Item__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),true);if c.error!=0 {return P_two_Item.{error=c.error};}return P_two_Item.{wire=wire};}\nP_two_Item_encode :: (msg:P_two_Item,out:[] u8)-";
  #assert cast(string)output[3072..<3584]==">P__pb_View {if msg.error!=0 {return P__pb_View.{error=msg.error};} c:=P_two_Item__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P__pb_View.{error=c.error};} return P__pb_copy(msg.wire,out);}\nP_two_Item_unknown :: (msg:P_two_Item,cursor:int)->P__pb_Field {if msg.error!=0 {return P__pb_Field.{error=msg.error};}checked:=P_two_Item_decode(msg.wire);if checked.error!=0 {return P__pb_Field.{error=checked.error};}pos:=cursor;while true {f:=P__pb_scan(";
  #assert cast(string)output[3584..<3701]=="msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(false) {return f;}}return P__pb_Field.{done=true};}\n";
}

#test "bytes and grammar policies" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[262144] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[96] u8.[10,94,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,66,10,1,77,18,23,10,5,98,121,116,101,115,24,1,32,1,40,12,66,8,208,243,24,33,128,244,24,6,18,36,10,7,103,114,97,109,109,97,114,24,2,32,1,40,9,66,19,250,243,24,15,110,111,110,101,109,112,116,121,45,110,111,45,110,117,108,98,6,112,114,111,116,111,51];
  n:=G_generate(state[..],cast(string)fixture[..],"P","");
  #assert state[0].error=="" && n==5653;
  #assert cast(string)output[0..<512]=="\n// @proto-descriptor-declarations\n// .test.M -> P_M\nP_M :: struct {wire:[] u8;error:u32;}\nP_M_Option :: struct {value:P_M;present:bool;error:u32;}\nP_M__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire.count) {return P__pb_Check.{error=P__pb_LIMIT};}\nleft:=budget;pos:=0;\nseen_bytes:bool=false;\nseen_grammar:bool=false;\nwhile pos<wire.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.er";
  #assert cast(string)output[512..<1024]=="ror!=0 {return P__pb_Check.{error=f.error};} if canonical && !P__pb_minimal(wire,f) {return P__pb_Check.{error=P__pb_WIRE};} pos=f.next;\nif f.number==1 {\nif seen_bytes {return P__pb_Check.{error=P__pb_DUPLICATE};}\nif f.wt!=2 { return P__pb_Check.{error=P__pb_WIRE}; }\nif f.payload.count>33 { return P__pb_Check.{error=P__pb_LIMIT}; }\nif f.payload.count!=6 { return P__pb_Check.{error=P__pb_LIMIT}; }\nseen_bytes=true;\n}\nelse if f.number==2 {\nif seen_grammar {return P__pb_Check.{error=P__pb_DUPLICATE};}\nif f.wt!=";
  #assert cast(string)output[1024..<1536]=="2 { return P__pb_Check.{error=P__pb_WIRE}; }\nif !P__pb_utf8(f.payload) { return P__pb_Check.{error=P__pb_UTF8}; }\nif !P__pb_no_nul(f.payload) { return P__pb_Check.{error=P__pb_VALUE}; }\nseen_grammar=true;\n}\n}\nreturn P__pb_Check.{left=left};}\nP_M_new :: ()->P_M {return P_M.{};}\nP_M_decode :: (wire:[] u8)->P_M {c:=P_M__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P_M.{error=c.error};}return P_M.{wire=wire};}\nP_M_decode_minimal :: (wire:[] u8)->P_M {c";
  #assert cast(string)output[1536..<2048]==":=P_M__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),true);if c.error!=0 {return P_M.{error=c.error};}return P_M.{wire=wire};}\nP_M_encode :: (msg:P_M,out:[] u8)->P__pb_View {if msg.error!=0 {return P__pb_View.{error=msg.error};} c:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P__pb_View.{error=c.error};} return P__pb_copy(msg.wire,out);}\nP_M_unknown :: (msg:P_M,cursor:int)->P__pb_Field {if msg.error!=0 {return";
  #assert cast(string)output[2048..<2560]==" P__pb_Field.{error=msg.error};}checked:=P_M_decode(msg.wire);if checked.error!=0 {return P__pb_Field.{error=checked.error};}pos:=cursor;while true {f:=P__pb_scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(f.number==1 || f.number==2) {return f;}}return P__pb_Field.{done=true};}\nP_M_FIELD_bytes :: 1;\nP_M_get_bytes :: (msg:P_M)->P__pb_Bytes {\nif msg.error!=0 {return P__pb_Bytes.{error=msg.error};}checked:=P_M_decode(msg.wire);if checked.error!=0 {return P__pb_Bytes.{error=checked.error};";
  #assert cast(string)output[2560..<3072]=="}\nf:=P__pb_find(msg.wire,1);return P__pb_Bytes.{value=f.payload,present=!f.done,error=f.error};}\nP_M_set_bytes :: (msg:P_M,value:[] u8,out:[] u8)->P_M {\nif msg.error!=0 {return P_M.{error=msg.error};} checked:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_M.{error=checked.error};}\nremoved_fields:= [1] u32.[1];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\ndata=value;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],1,2,raw,data,txt,";
  #assert cast(string)output[3072..<3584]=="is_text,false,out);\nif result.error!=0 {return P_M.{error=result.error};} final:=P_M__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_M.{error=final.error};}return P_M.{wire=result.wire};}\nP_M_clear_bytes :: (msg:P_M,out:[] u8)->P_M {\nif msg.error!=0 {return P_M.{error=msg.error};} checked:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_M.{error=checked.error};}\nre";
  #assert cast(string)output[3584..<4096]=="moved_fields:= [1] u32.[1];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],1,2,raw,data,txt,is_text,true,out);\nif result.error!=0 {return P_M.{error=result.error};} final:=P_M__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_M.{error=final.error};}return P_M.{wire=result.wire};}\nP_M_FIELD_grammar :: 2;\nP_M_get_grammar :: (msg:P_M)->P__pb_String {\nif msg.error!=0 {return P__pb_String.{error=m";
  #assert cast(string)output[4096..<4608]=="sg.error};}checked:=P_M_decode(msg.wire);if checked.error!=0 {return P__pb_String.{error=checked.error};}\nf:=P__pb_find(msg.wire,2);return P__pb_String.{value=cast(string) f.payload,present=!f.done,error=f.error};}\nP_M_set_grammar :: (msg:P_M,value:string,out:[] u8)->P_M {\nif msg.error!=0 {return P_M.{error=msg.error};} checked:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_M.{error=checked.error};}\nremoved_fields:= [1] u32.[2];\ndat";
  #assert cast(string)output[4608..<5120]=="a:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\ntxt=value;is_text=true;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],2,2,raw,data,txt,is_text,false,out);\nif result.error!=0 {return P_M.{error=result.error};} final:=P_M__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_M.{error=final.error};}return P_M.{wire=result.wire};}\nP_M_clear_grammar :: (msg:P_M,out:[] u8)->P_M {\nif msg.error!=0 {return P_M.{error=msg.error};} checked:=P_M__check(msg.";
  #assert cast(string)output[5120..<5632]=="wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_M.{error=checked.error};}\nremoved_fields:= [1] u32.[2];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],2,2,raw,data,txt,is_text,true,out);\nif result.error!=0 {return P_M.{error=result.error};} final:=P_M__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_M.{error=final.error};}return P_M.";
  #assert cast(string)output[5632..<5653]=="{wire=result.wire};}\n";
}
#test "multiple message oneof offsets" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[262144] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[105] u8.[10,103,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,25,10,1,65,18,11,10,1,97,24,1,32,1,40,5,72,0,66,7,10,5,102,105,114,115,116,34,48,10,1,66,18,11,10,1,98,24,1,32,1,40,5,72,0,18,11,10,1,99,24,2,32,1,40,5,72,1,66,8,10,6,115,101,99,111,110,100,66,7,10,5,116,104,105,114,100,98,6,112,114,111,116,111,51];
  n:=G_generate(state[..],cast(string)fixture[..],"P","");
  #assert state[0].error=="" && n==9802;
  #assert cast(string)output[0..<512]=="\n// @proto-descriptor-declarations\n// .test.A -> P_A\nP_A :: struct {wire:[] u8;error:u32;}\nP_A_Option :: struct {value:P_A;present:bool;error:u32;}\n// .test.B -> P_B\nP_B :: struct {wire:[] u8;error:u32;}\nP_B_Option :: struct {value:P_B;present:bool;error:u32;}\nP_A__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire.count) {return P__pb_Check.{error=P__pb_LIMIT};}\nleft:=budget;pos:=0;\none_0:bool=false;\nwhile pos<wire.count {\nif left<=0";
  #assert cast(string)output[512..<1024]==" {return P__pb_Check.{error=P__pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.error!=0 {return P__pb_Check.{error=f.error};} if canonical && !P__pb_minimal(wire,f) {return P__pb_Check.{error=P__pb_WIRE};} pos=f.next;\nif f.number==1 {\nif one_0 {return P__pb_Check.{error=P__pb_DUPLICATE};} one_0=true;\nif f.wt!=0 { return P__pb_Check.{error=P__pb_WIRE}; }\n}\n}\nreturn P__pb_Check.{left=left};}\nP_A_new :: ()->P_A {return P_A.{};}\nP_A_decode :: (wire:[] u8)->P_A {c:=P_A__check(wire,P__pb_root_depth(),P__pb_defa";
  #assert cast(string)output[1024..<1536]=="ult_fields(),P__pb_require_fields(),false);if c.error!=0 {return P_A.{error=c.error};}return P_A.{wire=wire};}\nP_A_decode_minimal :: (wire:[] u8)->P_A {c:=P_A__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),true);if c.error!=0 {return P_A.{error=c.error};}return P_A.{wire=wire};}\nP_A_encode :: (msg:P_A,out:[] u8)->P__pb_View {if msg.error!=0 {return P__pb_View.{error=msg.error};} c:=P_A__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.";
  #assert cast(string)output[1536..<2048]=="error!=0 {return P__pb_View.{error=c.error};} return P__pb_copy(msg.wire,out);}\nP_A_case_first :: (msg:P_A)->P__pb_U32 {if msg.error!=0 {return P__pb_U32.{error=msg.error};}checked:=P_A_decode(msg.wire);if checked.error!=0 {return P__pb_U32.{error=checked.error};}pos:=0;while pos<msg.wire.count {f:=P__pb_scan(msg.wire,pos);pos=f.next;if f.number==1 {return P__pb_U32.{value=f.number,present=true};}} return P__pb_U32.{};}\nP_A_unknown :: (msg:P_A,cursor:int)->P__pb_Field {if msg.error!=0 {return P__pb_Field.{e";
  #assert cast(string)output[2048..<2560]=="rror=msg.error};}checked:=P_A_decode(msg.wire);if checked.error!=0 {return P__pb_Field.{error=checked.error};}pos:=cursor;while true {f:=P__pb_scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(f.number==1) {return f;}}return P__pb_Field.{done=true};}\nP_A_FIELD_a :: 1;\nP_A_get_a :: (msg:P_A)->P__pb_S32 {\nif msg.error!=0 {return P__pb_S32.{error=msg.error};}checked:=P_A_decode(msg.wire);if checked.error!=0 {return P__pb_S32.{error=checked.error};}\nf:=P__pb_find(msg.wire,1);return P__pb_S32";
  #assert cast(string)output[2560..<3072]==".{value=cast,trunc(s32) f.raw,present=!f.done,error=f.error};}\nP_A_set_a :: (msg:P_A,value:s32,out:[] u8)->P_A {\nif msg.error!=0 {return P_A.{error=msg.error};} checked:=P_A__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_A.{error=checked.error};}\nremoved_fields:= [1] u32.[1];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast,trunc(u64) value;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],1,0,raw,data,txt,is_text,false,out);\nif re";
  #assert cast(string)output[3072..<3584]=="sult.error!=0 {return P_A.{error=result.error};} final:=P_A__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_A.{error=final.error};}return P_A.{wire=result.wire};}\nP_A_clear_a :: (msg:P_A,out:[] u8)->P_A {\nif msg.error!=0 {return P_A.{error=msg.error};} checked:=P_A__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_A.{error=checked.error};}\nremoved_fields:= [1] u32.[1];\nd";
  #assert cast(string)output[3584..<4096]=="ata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],1,0,raw,data,txt,is_text,true,out);\nif result.error!=0 {return P_A.{error=result.error};} final:=P_A__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_A.{error=final.error};}return P_A.{wire=result.wire};}\nP_B__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire.count) {retur";
  #assert cast(string)output[4096..<4608]=="n P__pb_Check.{error=P__pb_LIMIT};}\nleft:=budget;pos:=0;\none_0:bool=false;\none_1:bool=false;\nwhile pos<wire.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.error!=0 {return P__pb_Check.{error=f.error};} if canonical && !P__pb_minimal(wire,f) {return P__pb_Check.{error=P__pb_WIRE};} pos=f.next;\nif f.number==1 {\nif one_0 {return P__pb_Check.{error=P__pb_DUPLICATE};} one_0=true;\nif f.wt!=0 { return P__pb_Check.{error=P__pb_WIRE}; }\n}\nelse if f.number==2 {\nif o";
  #assert cast(string)output[4608..<5120]=="ne_1 {return P__pb_Check.{error=P__pb_DUPLICATE};} one_1=true;\nif f.wt!=0 { return P__pb_Check.{error=P__pb_WIRE}; }\n}\n}\nreturn P__pb_Check.{left=left};}\nP_B_new :: ()->P_B {return P_B.{};}\nP_B_decode :: (wire:[] u8)->P_B {c:=P_B__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P_B.{error=c.error};}return P_B.{wire=wire};}\nP_B_decode_minimal :: (wire:[] u8)->P_B {c:=P_B__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),true)";
  #assert cast(string)output[5120..<5632]==";if c.error!=0 {return P_B.{error=c.error};}return P_B.{wire=wire};}\nP_B_encode :: (msg:P_B,out:[] u8)->P__pb_View {if msg.error!=0 {return P__pb_View.{error=msg.error};} c:=P_B__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P__pb_View.{error=c.error};} return P__pb_copy(msg.wire,out);}\nP_B_case_second :: (msg:P_B)->P__pb_U32 {if msg.error!=0 {return P__pb_U32.{error=msg.error};}checked:=P_B_decode(msg.wire);if checked.error!=0 {return P__pb_U32";
  #assert cast(string)output[5632..<6144]==".{error=checked.error};}pos:=0;while pos<msg.wire.count {f:=P__pb_scan(msg.wire,pos);pos=f.next;if f.number==1 {return P__pb_U32.{value=f.number,present=true};}} return P__pb_U32.{};}\nP_B_case_third :: (msg:P_B)->P__pb_U32 {if msg.error!=0 {return P__pb_U32.{error=msg.error};}checked:=P_B_decode(msg.wire);if checked.error!=0 {return P__pb_U32.{error=checked.error};}pos:=0;while pos<msg.wire.count {f:=P__pb_scan(msg.wire,pos);pos=f.next;if f.number==2 {return P__pb_U32.{value=f.number,present=true};}} return";
  #assert cast(string)output[6144..<6656]==" P__pb_U32.{};}\nP_B_unknown :: (msg:P_B,cursor:int)->P__pb_Field {if msg.error!=0 {return P__pb_Field.{error=msg.error};}checked:=P_B_decode(msg.wire);if checked.error!=0 {return P__pb_Field.{error=checked.error};}pos:=cursor;while true {f:=P__pb_scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(f.number==1 || f.number==2) {return f;}}return P__pb_Field.{done=true};}\nP_B_FIELD_b :: 1;\nP_B_get_b :: (msg:P_B)->P__pb_S32 {\nif msg.error!=0 {return P__pb_S32.{error=msg.error};}checked:=P_B_de";
  #assert cast(string)output[6656..<7168]=="code(msg.wire);if checked.error!=0 {return P__pb_S32.{error=checked.error};}\nf:=P__pb_find(msg.wire,1);return P__pb_S32.{value=cast,trunc(s32) f.raw,present=!f.done,error=f.error};}\nP_B_set_b :: (msg:P_B,value:s32,out:[] u8)->P_B {\nif msg.error!=0 {return P_B.{error=msg.error};} checked:=P_B__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_B.{error=checked.error};}\nremoved_fields:= [1] u32.[1];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\n";
  #assert cast(string)output[7168..<7680]=="raw=cast,trunc(u64) value;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],1,0,raw,data,txt,is_text,false,out);\nif result.error!=0 {return P_B.{error=result.error};} final:=P_B__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_B.{error=final.error};}return P_B.{wire=result.wire};}\nP_B_clear_b :: (msg:P_B,out:[] u8)->P_B {\nif msg.error!=0 {return P_B.{error=msg.error};} checked:=P_B__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(";
  #assert cast(string)output[7680..<8192]=="),P__pb_partial_fields(),false);if checked.error!=0 {return P_B.{error=checked.error};}\nremoved_fields:= [1] u32.[1];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],1,0,raw,data,txt,is_text,true,out);\nif result.error!=0 {return P_B.{error=result.error};} final:=P_B__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_B.{error=final.error};}return P_B.{wire=result.wire};}\nP_B_FIELD_c :: 2;\nP_B_ge";
  #assert cast(string)output[8192..<8704]=="t_c :: (msg:P_B)->P__pb_S32 {\nif msg.error!=0 {return P__pb_S32.{error=msg.error};}checked:=P_B_decode(msg.wire);if checked.error!=0 {return P__pb_S32.{error=checked.error};}\nf:=P__pb_find(msg.wire,2);return P__pb_S32.{value=cast,trunc(s32) f.raw,present=!f.done,error=f.error};}\nP_B_set_c :: (msg:P_B,value:s32,out:[] u8)->P_B {\nif msg.error!=0 {return P_B.{error=msg.error};} checked:=P_B__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_B.{";
  #assert cast(string)output[8704..<9216]=="error=checked.error};}\nremoved_fields:= [1] u32.[2];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast,trunc(u64) value;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],2,0,raw,data,txt,is_text,false,out);\nif result.error!=0 {return P_B.{error=result.error};} final:=P_B__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_B.{error=final.error};}return P_B.{wire=result.wire};}\nP_B_clear_c :: (msg:P_B,out:[] u8)->P_B {\nif msg.error!=0 {re";
  #assert cast(string)output[9216..<9728]=="turn P_B.{error=msg.error};} checked:=P_B__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_B.{error=checked.error};}\nremoved_fields:= [1] u32.[2];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],2,0,raw,data,txt,is_text,true,out);\nif result.error!=0 {return P_B.{error=result.error};} final:=P_B__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.";
  #assert cast(string)output[9728..<9802]=="error!=0 {return P_B.{error=final.error};}return P_B.{wire=result.wire};}\n";
}
#test "nested messages and enum" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[262144] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[62] u8.[10,60,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,32,10,6,80,97,114,101,110,116,26,7,10,5,67,104,105,108,100,34,13,10,1,69,18,8,10,4,90,69,82,79,16,0,98,6,112,114,111,116,111,51];
  n:=G_generate(state[..],cast(string)fixture[..],"P","");
  #assert state[0].error=="" && n==3658;
  #assert cast(string)output[0..<512]=="\n// @proto-descriptor-declarations\nP_E_ZERO :: 0;\nP_E_known :: (value:s32)->bool {return value==0;}\n// .test.Parent -> P_Parent\nP_Parent :: struct {wire:[] u8;error:u32;}\nP_Parent_Option :: struct {value:P_Parent;present:bool;error:u32;}\n// .test.Parent.Child -> P_Child\nP_Child :: struct {wire:[] u8;error:u32;}\nP_Child_Option :: struct {value:P_Child;present:bool;error:u32;}\nP_Parent__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire";
  #assert cast(string)output[512..<1024]==".count) {return P__pb_Check.{error=P__pb_LIMIT};}\nleft:=budget;pos:=0;\nwhile pos<wire.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.error!=0 {return P__pb_Check.{error=f.error};} if canonical && !P__pb_minimal(wire,f) {return P__pb_Check.{error=P__pb_WIRE};} pos=f.next;\n}\nreturn P__pb_Check.{left=left};}\nP_Parent_new :: ()->P_Parent {return P_Parent.{};}\nP_Parent_decode :: (wire:[] u8)->P_Parent {c:=P_Parent__check(wire,P__pb_root_depth(),P__pb_default_fi";
  #assert cast(string)output[1024..<1536]=="elds(),P__pb_require_fields(),false);if c.error!=0 {return P_Parent.{error=c.error};}return P_Parent.{wire=wire};}\nP_Parent_decode_minimal :: (wire:[] u8)->P_Parent {c:=P_Parent__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),true);if c.error!=0 {return P_Parent.{error=c.error};}return P_Parent.{wire=wire};}\nP_Parent_encode :: (msg:P_Parent,out:[] u8)->P__pb_View {if msg.error!=0 {return P__pb_View.{error=msg.error};} c:=P_Parent__check(msg.wire,P__pb_root_depth(),P__pb_default_";
  #assert cast(string)output[1536..<2048]=="fields(),P__pb_require_fields(),false);if c.error!=0 {return P__pb_View.{error=c.error};} return P__pb_copy(msg.wire,out);}\nP_Parent_unknown :: (msg:P_Parent,cursor:int)->P__pb_Field {if msg.error!=0 {return P__pb_Field.{error=msg.error};}checked:=P_Parent_decode(msg.wire);if checked.error!=0 {return P__pb_Field.{error=checked.error};}pos:=cursor;while true {f:=P__pb_scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(false) {return f;}}return P__pb_Field.{done=true};}\nP_Child__check :: (w";
  #assert cast(string)output[2048..<2560]=="ire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire.count) {return P__pb_Check.{error=P__pb_LIMIT};}\nleft:=budget;pos:=0;\nwhile pos<wire.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.error!=0 {return P__pb_Check.{error=f.error};} if canonical && !P__pb_minimal(wire,f) {return P__pb_Check.{error=P__pb_WIRE};} pos=f.next;\n}\nreturn P__pb_Check.{left=left};}\nP_Child_new :: ()->P_Child {return P_Chi";
  #assert cast(string)output[2560..<3072]=="ld.{};}\nP_Child_decode :: (wire:[] u8)->P_Child {c:=P_Child__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P_Child.{error=c.error};}return P_Child.{wire=wire};}\nP_Child_decode_minimal :: (wire:[] u8)->P_Child {c:=P_Child__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),true);if c.error!=0 {return P_Child.{error=c.error};}return P_Child.{wire=wire};}\nP_Child_encode :: (msg:P_Child,out:[] u8)->P__pb_View {if msg.error!=0 {r";
  #assert cast(string)output[3072..<3584]=="eturn P__pb_View.{error=msg.error};} c:=P_Child__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P__pb_View.{error=c.error};} return P__pb_copy(msg.wire,out);}\nP_Child_unknown :: (msg:P_Child,cursor:int)->P__pb_Field {if msg.error!=0 {return P__pb_Field.{error=msg.error};}checked:=P_Child_decode(msg.wire);if checked.error!=0 {return P__pb_Field.{error=checked.error};}pos:=cursor;while true {f:=P__pb_scan(msg.wire,pos);if f.error!=0 || f.done {retu";
  #assert cast(string)output[3584..<3658]=="rn f;}pos=f.next;if !(false) {return f;}}return P__pb_Field.{done=true};}\n";
}
#test "enum and message leaf collisions" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[262144] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[81] u8.[10,29,10,9,111,110,101,46,112,114,111,116,111,18,3,111,110,101,34,3,10,1,69,98,6,112,114,111,116,111,51,10,48,10,9,116,119,111,46,112,114,111,116,111,18,3,116,119,111,34,7,10,5,79,116,104,101,114,42,13,10,1,69,18,8,10,4,90,69,82,79,16,0,98,6,112,114,111,116,111,51];
  n:=G_generate(state[..],cast(string)fixture[..],"P","");
  #assert state[0].error=="" && n==3628;
  #assert cast(string)output[0..<512]=="\n// @proto-descriptor-declarations\nP_two_E_ZERO :: 0;\nP_two_E_known :: (value:s32)->bool {return value==0;}\n// .one.E -> P_one_E\nP_one_E :: struct {wire:[] u8;error:u32;}\nP_one_E_Option :: struct {value:P_one_E;present:bool;error:u32;}\n// .two.Other -> P_Other\nP_Other :: struct {wire:[] u8;error:u32;}\nP_Other_Option :: struct {value:P_Other;present:bool;error:u32;}\nP_one_E__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire.count) {re";
  #assert cast(string)output[512..<1024]=="turn P__pb_Check.{error=P__pb_LIMIT};}\nleft:=budget;pos:=0;\nwhile pos<wire.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.error!=0 {return P__pb_Check.{error=f.error};} if canonical && !P__pb_minimal(wire,f) {return P__pb_Check.{error=P__pb_WIRE};} pos=f.next;\n}\nreturn P__pb_Check.{left=left};}\nP_one_E_new :: ()->P_one_E {return P_one_E.{};}\nP_one_E_decode :: (wire:[] u8)->P_one_E {c:=P_one_E__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_requ";
  #assert cast(string)output[1024..<1536]=="ire_fields(),false);if c.error!=0 {return P_one_E.{error=c.error};}return P_one_E.{wire=wire};}\nP_one_E_decode_minimal :: (wire:[] u8)->P_one_E {c:=P_one_E__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),true);if c.error!=0 {return P_one_E.{error=c.error};}return P_one_E.{wire=wire};}\nP_one_E_encode :: (msg:P_one_E,out:[] u8)->P__pb_View {if msg.error!=0 {return P__pb_View.{error=msg.error};} c:=P_one_E__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fiel";
  #assert cast(string)output[1536..<2048]=="ds(),false);if c.error!=0 {return P__pb_View.{error=c.error};} return P__pb_copy(msg.wire,out);}\nP_one_E_unknown :: (msg:P_one_E,cursor:int)->P__pb_Field {if msg.error!=0 {return P__pb_Field.{error=msg.error};}checked:=P_one_E_decode(msg.wire);if checked.error!=0 {return P__pb_Field.{error=checked.error};}pos:=cursor;while true {f:=P__pb_scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(false) {return f;}}return P__pb_Field.{done=true};}\nP_Other__check :: (wire:[] u8,depth:int,budget:int";
  #assert cast(string)output[2048..<2560]==",required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire.count) {return P__pb_Check.{error=P__pb_LIMIT};}\nleft:=budget;pos:=0;\nwhile pos<wire.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.error!=0 {return P__pb_Check.{error=f.error};} if canonical && !P__pb_minimal(wire,f) {return P__pb_Check.{error=P__pb_WIRE};} pos=f.next;\n}\nreturn P__pb_Check.{left=left};}\nP_Other_new :: ()->P_Other {return P_Other.{};}\nP_Other_decode :: (wir";
  #assert cast(string)output[2560..<3072]=="e:[] u8)->P_Other {c:=P_Other__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P_Other.{error=c.error};}return P_Other.{wire=wire};}\nP_Other_decode_minimal :: (wire:[] u8)->P_Other {c:=P_Other__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),true);if c.error!=0 {return P_Other.{error=c.error};}return P_Other.{wire=wire};}\nP_Other_encode :: (msg:P_Other,out:[] u8)->P__pb_View {if msg.error!=0 {return P__pb_View.{error=msg.er";
  #assert cast(string)output[3072..<3584]=="ror};} c:=P_Other__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P__pb_View.{error=c.error};} return P__pb_copy(msg.wire,out);}\nP_Other_unknown :: (msg:P_Other,cursor:int)->P__pb_Field {if msg.error!=0 {return P__pb_Field.{error=msg.error};}checked:=P_Other_decode(msg.wire);if checked.error!=0 {return P__pb_Field.{error=checked.error};}pos:=cursor;while true {f:=P__pb_scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(false) {";
  #assert cast(string)output[3584..<3628]=="return f;}}return P__pb_Field.{done=true};}\n";
}
#test "enum signed endpoints" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[262144] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[79] u8.[10,77,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,3,10,1,77,42,44,10,1,69,18,8,10,4,90,69,82,79,16,0,18,11,10,3,77,65,88,16,255,255,255,255,7,18,16,10,3,77,73,78,16,128,128,128,128,248,255,255,255,255,1,98,6,112,114,111,116,111,51];
  n:=G_generate(state[..],cast(string)fixture[..],"P","");
  #assert state[0].error=="" && n==1853;
  #assert cast(string)output[0..<512]=="\n// @proto-descriptor-declarations\nP_E_ZERO :: 0;\nP_E_MAX :: 2147483647;\nP_E_MIN :: -2147483648;\nP_E_known :: (value:s32)->bool {return value==0 || value==2147483647 || value==-2147483648;}\n// .test.M -> P_M\nP_M :: struct {wire:[] u8;error:u32;}\nP_M_Option :: struct {value:P_M;present:bool;error:u32;}\nP_M__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire.count) {return P__pb_Check.{error=P__pb_LIMIT};}\nleft:=budget;pos:=0;\nwhile pos";
  #assert cast(string)output[512..<1024]=="<wire.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.error!=0 {return P__pb_Check.{error=f.error};} if canonical && !P__pb_minimal(wire,f) {return P__pb_Check.{error=P__pb_WIRE};} pos=f.next;\n}\nreturn P__pb_Check.{left=left};}\nP_M_new :: ()->P_M {return P_M.{};}\nP_M_decode :: (wire:[] u8)->P_M {c:=P_M__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P_M.{error=c.error};}return P_M.{wire=wire};}\nP_M_d";
  #assert cast(string)output[1024..<1536]=="ecode_minimal :: (wire:[] u8)->P_M {c:=P_M__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),true);if c.error!=0 {return P_M.{error=c.error};}return P_M.{wire=wire};}\nP_M_encode :: (msg:P_M,out:[] u8)->P__pb_View {if msg.error!=0 {return P__pb_View.{error=msg.error};} c:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P__pb_View.{error=c.error};} return P__pb_copy(msg.wire,out);}\nP_M_unknown :: (msg:P_M,cursor:int)-";
  #assert cast(string)output[1536..<1853]==">P__pb_Field {if msg.error!=0 {return P__pb_Field.{error=msg.error};}checked:=P_M_decode(msg.wire);if checked.error!=0 {return P__pb_Field.{error=checked.error};}pos:=cursor;while true {f:=P__pb_scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(false) {return f;}}return P__pb_Field.{done=true};}\n";
}
#test "explicit dependencies and skipped builtin files" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[262144] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[191] u8.[10,34,10,32,103,111,111,103,108,101,47,112,114,111,116,111,98,117,102,47,100,101,115,99,114,105,112,116,111,114,46,112,114,111,116,111,10,43,10,41,111,112,101,110,97,103,101,110,116,115,47,111,112,116,105,111,110,115,47,118,49,97,108,112,104,97,49,47,111,112,116,105,111,110,115,46,112,114,111,116,111,10,108,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,3,10,1,77,26,32,103,111,111,103,108,101,47,112,114,111,116,111,98,117,102,47,100,101,115,99,114,105,112,116,111,114,46,112,114,111,116,111,26,41,111,112,101,110,97,103,101,110,116,115,47,111,112,116,105,111,110,115,47,118,49,97,108,112,104,97,49,47,111,112,116,105,111,110,115,46,112,114,111,116,111,98,6,112,114,111,116,111,51];
  n:=G_generate(state[..],cast(string)fixture[..],"P","");
  #assert state[0].error=="" && n==1698;
  #assert cast(string)output[0..<512]=="\n// @proto-descriptor-declarations\n// .test.M -> P_M\nP_M :: struct {wire:[] u8;error:u32;}\nP_M_Option :: struct {value:P_M;present:bool;error:u32;}\nP_M__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire.count) {return P__pb_Check.{error=P__pb_LIMIT};}\nleft:=budget;pos:=0;\nwhile pos<wire.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.error!=0 {return P__pb_Check.{error=f.error};} if ";
  #assert cast(string)output[512..<1024]=="canonical && !P__pb_minimal(wire,f) {return P__pb_Check.{error=P__pb_WIRE};} pos=f.next;\n}\nreturn P__pb_Check.{left=left};}\nP_M_new :: ()->P_M {return P_M.{};}\nP_M_decode :: (wire:[] u8)->P_M {c:=P_M__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P_M.{error=c.error};}return P_M.{wire=wire};}\nP_M_decode_minimal :: (wire:[] u8)->P_M {c:=P_M__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),true);if c.error!=0 {return P_M.{er";
  #assert cast(string)output[1024..<1536]=="ror=c.error};}return P_M.{wire=wire};}\nP_M_encode :: (msg:P_M,out:[] u8)->P__pb_View {if msg.error!=0 {return P__pb_View.{error=msg.error};} c:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P__pb_View.{error=c.error};} return P__pb_copy(msg.wire,out);}\nP_M_unknown :: (msg:P_M,cursor:int)->P__pb_Field {if msg.error!=0 {return P__pb_Field.{error=msg.error};}checked:=P_M_decode(msg.wire);if checked.error!=0 {return P__pb_Field.{error=checked.e";
  #assert cast(string)output[1536..<1698]=="rror};}pos:=cursor;while true {f:=P__pb_scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(false) {return f;}}return P__pb_Field.{done=true};}\n";
}
#test "standard option preservation" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[262144] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[54] u8.[10,52,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,24,10,1,77,58,4,56,0,8,1,18,13,10,1,120,24,1,32,1,40,5,66,2,8,1,98,6,112,114,111,116,111,51];
  n:=G_generate(state[..],cast(string)fixture[..],"P","");
  #assert state[0].error=="" && n==3499;
  #assert cast(string)output[0..<512]=="\n// @proto-descriptor-declarations\n// .test.M -> P_M\nP_M :: struct {wire:[] u8;error:u32;}\nP_M_Option :: struct {value:P_M;present:bool;error:u32;}\nP_M__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->P__pb_Check {\nif !P__pb_message_admitted(depth,wire.count) {return P__pb_Check.{error=P__pb_LIMIT};}\nleft:=budget;pos:=0;\nseen_x:bool=false;\nwhile pos<wire.count {\nif left<=0 {return P__pb_Check.{error=P__pb_LIMIT};} left-=1;\nf:=P__pb_scan(wire,pos);if f.error!=0 {return P__pb_Check.{e";
  #assert cast(string)output[512..<1024]=="rror=f.error};} if canonical && !P__pb_minimal(wire,f) {return P__pb_Check.{error=P__pb_WIRE};} pos=f.next;\nif f.number==1 {\nif seen_x {return P__pb_Check.{error=P__pb_DUPLICATE};}\nif f.wt!=0 { return P__pb_Check.{error=P__pb_WIRE}; }\nseen_x=true;\n}\n}\nreturn P__pb_Check.{left=left};}\nP_M_new :: ()->P_M {return P_M.{};}\nP_M_decode :: (wire:[] u8)->P_M {c:=P_M__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P_M.{error=c.error};}return P_M.{wire=wire};}";
  #assert cast(string)output[1024..<1536]=="\nP_M_decode_minimal :: (wire:[] u8)->P_M {c:=P_M__check(wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),true);if c.error!=0 {return P_M.{error=c.error};}return P_M.{wire=wire};}\nP_M_encode :: (msg:P_M,out:[] u8)->P__pb_View {if msg.error!=0 {return P__pb_View.{error=msg.error};} c:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_require_fields(),false);if c.error!=0 {return P__pb_View.{error=c.error};} return P__pb_copy(msg.wire,out);}\nP_M_unknown :: (msg:P_M,cursor";
  #assert cast(string)output[1536..<2048]==":int)->P__pb_Field {if msg.error!=0 {return P__pb_Field.{error=msg.error};}checked:=P_M_decode(msg.wire);if checked.error!=0 {return P__pb_Field.{error=checked.error};}pos:=cursor;while true {f:=P__pb_scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(f.number==1) {return f;}}return P__pb_Field.{done=true};}\nP_M_FIELD_x :: 1;\nP_M_get_x :: (msg:P_M)->P__pb_S32 {\nif msg.error!=0 {return P__pb_S32.{error=msg.error};}checked:=P_M_decode(msg.wire);if checked.error!=0 {return P__pb_S32.{error=c";
  #assert cast(string)output[2048..<2560]=="hecked.error};}\nf:=P__pb_find(msg.wire,1);return P__pb_S32.{value=cast,trunc(s32) f.raw,present=!f.done,error=f.error};}\nP_M_set_x :: (msg:P_M,value:s32,out:[] u8)->P_M {\nif msg.error!=0 {return P_M.{error=msg.error};} checked:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P_M.{error=checked.error};}\nremoved_fields:= [1] u32.[1];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast,trunc(u64) value;\nresult:=P__pb_rewrite(msg.wire,rem";
  #assert cast(string)output[2560..<3072]=="oved_fields[..],1,0,raw,data,txt,is_text,false,out);\nif result.error!=0 {return P_M.{error=result.error};} final:=P_M__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_M.{error=final.error};}return P_M.{wire=result.wire};}\nP_M_clear_x :: (msg:P_M,out:[] u8)->P_M {\nif msg.error!=0 {return P_M.{error=msg.error};} checked:=P_M__check(msg.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if checked.error!=0 {return P";
  #assert cast(string)output[3072..<3499]=="_M.{error=checked.error};}\nremoved_fields:= [1] u32.[1];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=P__pb_rewrite(msg.wire,removed_fields[..],1,0,raw,data,txt,is_text,true,out);\nif result.error!=0 {return P_M.{error=result.error};} final:=P_M__check(result.wire,P__pb_root_depth(),P__pb_default_fields(),P__pb_partial_fields(),false);if final.error!=0 {return P_M.{error=final.error};}return P_M.{wire=result.wire};}\n";
}
#test "object work accounting" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[262144] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[44] u8.[10,42,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,14,10,1,77,18,9,10,1,120,24,1,32,1,40,5,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="" && state[0].visited==11 && state[0].np==1;
}
#test "enum work accounting" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[262144] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[48] u8.[10,46,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,3,10,1,77,42,13,10,1,69,18,8,10,4,90,69,82,79,16,0,98,6,112,114,111,116,111,51];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="" && state[0].visited==11 && state[0].np==1;
}
#test "generated helper collision is an import error" {
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[262144] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[51] u8.[10,49,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,6,10,4,73,116,101,109,34,13,10,11,73,116,101,109,95,79,112,116,105,111,110,98,6,112,114,111,116,111,51];
  n:=G_generate(state[..],cast(string)fixture[..],"P","");
  #assert state[0].error!="" && n==0;
}
#test "enum value storage" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[13] u8.[10,1,69,18,8,10,4,90,69,82,79,16,0];
  state[0].values=values[0..<0];
  D_enum(state[..],cast(string)fixture[..],".p");
  #assert state[0].error=="descriptor enum budget exceeded";
}
#test "enum type storage" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[13] u8.[10,1,69,18,8,10,4,90,69,82,79,16,0];
  state[0].enums=enums[0..<0];
  D_enum(state[..],cast(string)fixture[..],".p");
  #assert state[0].error=="descriptor enum budget exceeded";
}
#test "message storage" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[3] u8.[10,1,77];
  state[0].types=types[0..<0];
  D_message(state[..],cast(string)fixture[..],".p",0);
  #assert state[0].error=="descriptor message budget exceeded";
}
#test "field storage" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[14] u8.[10,1,77,18,9,10,1,120,24,1,32,1,40,5];
  state[0].fields=fields[0..<0];
  D_message(state[..],cast(string)fixture[..],".p",0);
  #assert state[0].error=="descriptor field budget exceeded";
}
#test "group storage" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[8] u8.[10,1,77,66,3,10,1,103];
  state[0].groups=groups[0..<0];
  D_message(state[..],cast(string)fixture[..],".p",0);
  #assert state[0].error=="descriptor oneof budget";
}
#test "file storage" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[33] u8.[10,31,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,3,10,1,77,98,6,112,114,111,116,111,51];
  state[0].files=files[0..<0];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="descriptor file budget exceeded";
}
#test "dependency storage" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[38] u8.[10,36,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,3,10,1,77,26,3,100,101,112,98,6,112,114,111,116,111,51];
  state[0].deps=deps[0..<0];
  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="descriptor dependency budget";
}
#test "per message groups" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[1949] u8.[10,1,77,66,4,10,2,103,48,66,4,10,2,103,49,66,4,10,2,103,50,66,4,10,2,103,51,66,4,10,2,103,52,66,4,10,2,103,53,66,4,10,2,103,54,66,4,10,2,103,55,66,4,10,2,103,56,66,4,10,2,103,57,66,5,10,3,103,49,48,66,5,10,3,103,49,49,66,5,10,3,103,49,50,66,5,10,3,103,49,51,66,5,10,3,103,49,52,66,5,10,3,103,49,53,66,5,10,3,103,49,54,66,5,10,3,103,49,55,66,5,10,3,103,49,56,66,5,10,3,103,49,57,66,5,10,3,103,50,48,66,5,10,3,103,50,49,66,5,10,3,103,50,50,66,5,10,3,103,50,51,66,5,10,3,103,50,52,66,5,10,3,103,50,53,66,5,10,3,103,50,54,66,5,10,3,103,50,55,66,5,10,3,103,50,56,66,5,10,3,103,50,57,66,5,10,3,103,51,48,66,5,10,3,103,51,49,66,5,10,3,103,51,50,66,5,10,3,103,51,51,66,5,10,3,103,51,52,66,5,10,3,103,51,53,66,5,10,3,103,51,54,66,5,10,3,103,51,55,66,5,10,3,103,51,56,66,5,10,3,103,51,57,66,5,10,3,103,52,48,66,5,10,3,103,52,49,66,5,10,3,103,52,50,66,5,10,3,103,52,51,66,5,10,3,103,52,52,66,5,10,3,103,52,53,66,5,10,3,103,52,54,66,5,10,3,103,52,55,66,5,10,3,103,52,56,66,5,10,3,103,52,57,66,5,10,3,103,53,48,66,5,10,3,103,53,49,66,5,10,3,103,53,50,66,5,10,3,103,53,51,66,5,10,3,103,53,52,66,5,10,3,103,53,53,66,5,10,3,103,53,54,66,5,10,3,103,53,55,66,5,10,3,103,53,56,66,5,10,3,103,53,57,66,5,10,3,103,54,48,66,5,10,3,103,54,49,66,5,10,3,103,54,50,66,5,10,3,103,54,51,66,5,10,3,103,54,52,66,5,10,3,103,54,53,66,5,10,3,103,54,54,66,5,10,3,103,54,55,66,5,10,3,103,54,56,66,5,10,3,103,54,57,66,5,10,3,103,55,48,66,5,10,3,103,55,49,66,5,10,3,103,55,50,66,5,10,3,103,55,51,66,5,10,3,103,55,52,66,5,10,3,103,55,53,66,5,10,3,103,55,54,66,5,10,3,103,55,55,66,5,10,3,103,55,56,66,5,10,3,103,55,57,66,5,10,3,103,56,48,66,5,10,3,103,56,49,66,5,10,3,103,56,50,66,5,10,3,103,56,51,66,5,10,3,103,56,52,66,5,10,3,103,56,53,66,5,10,3,103,56,54,66,5,10,3,103,56,55,66,5,10,3,103,56,56,66,5,10,3,103,56,57,66,5,10,3,103,57,48,66,5,10,3,103,57,49,66,5,10,3,103,57,50,66,5,10,3,103,57,51,66,5,10,3,103,57,52,66,5,10,3,103,57,53,66,5,10,3,103,57,54,66,5,10,3,103,57,55,66,5,10,3,103,57,56,66,5,10,3,103,57,57,66,6,10,4,103,49,48,48,66,6,10,4,103,49,48,49,66,6,10,4,103,49,48,50,66,6,10,4,103,49,48,51,66,6,10,4,103,49,48,52,66,6,10,4,103,49,48,53,66,6,10,4,103,49,48,54,66,6,10,4,103,49,48,55,66,6,10,4,103,49,48,56,66,6,10,4,103,49,48,57,66,6,10,4,103,49,49,48,66,6,10,4,103,49,49,49,66,6,10,4,103,49,49,50,66,6,10,4,103,49,49,51,66,6,10,4,103,49,49,52,66,6,10,4,103,49,49,53,66,6,10,4,103,49,49,54,66,6,10,4,103,49,49,55,66,6,10,4,103,49,49,56,66,6,10,4,103,49,49,57,66,6,10,4,103,49,50,48,66,6,10,4,103,49,50,49,66,6,10,4,103,49,50,50,66,6,10,4,103,49,50,51,66,6,10,4,103,49,50,52,66,6,10,4,103,49,50,53,66,6,10,4,103,49,50,54,66,6,10,4,103,49,50,55,66,6,10,4,103,49,50,56,66,6,10,4,103,49,50,57,66,6,10,4,103,49,51,48,66,6,10,4,103,49,51,49,66,6,10,4,103,49,51,50,66,6,10,4,103,49,51,51,66,6,10,4,103,49,51,52,66,6,10,4,103,49,51,53,66,6,10,4,103,49,51,54,66,6,10,4,103,49,51,55,66,6,10,4,103,49,51,56,66,6,10,4,103,49,51,57,66,6,10,4,103,49,52,48,66,6,10,4,103,49,52,49,66,6,10,4,103,49,52,50,66,6,10,4,103,49,52,51,66,6,10,4,103,49,52,52,66,6,10,4,103,49,52,53,66,6,10,4,103,49,52,54,66,6,10,4,103,49,52,55,66,6,10,4,103,49,52,56,66,6,10,4,103,49,52,57,66,6,10,4,103,49,53,48,66,6,10,4,103,49,53,49,66,6,10,4,103,49,53,50,66,6,10,4,103,49,53,51,66,6,10,4,103,49,53,52,66,6,10,4,103,49,53,53,66,6,10,4,103,49,53,54,66,6,10,4,103,49,53,55,66,6,10,4,103,49,53,56,66,6,10,4,103,49,53,57,66,6,10,4,103,49,54,48,66,6,10,4,103,49,54,49,66,6,10,4,103,49,54,50,66,6,10,4,103,49,54,51,66,6,10,4,103,49,54,52,66,6,10,4,103,49,54,53,66,6,10,4,103,49,54,54,66,6,10,4,103,49,54,55,66,6,10,4,103,49,54,56,66,6,10,4,103,49,54,57,66,6,10,4,103,49,55,48,66,6,10,4,103,49,55,49,66,6,10,4,103,49,55,50,66,6,10,4,103,49,55,51,66,6,10,4,103,49,55,52,66,6,10,4,103,49,55,53,66,6,10,4,103,49,55,54,66,6,10,4,103,49,55,55,66,6,10,4,103,49,55,56,66,6,10,4,103,49,55,57,66,6,10,4,103,49,56,48,66,6,10,4,103,49,56,49,66,6,10,4,103,49,56,50,66,6,10,4,103,49,56,51,66,6,10,4,103,49,56,52,66,6,10,4,103,49,56,53,66,6,10,4,103,49,56,54,66,6,10,4,103,49,56,55,66,6,10,4,103,49,56,56,66,6,10,4,103,49,56,57,66,6,10,4,103,49,57,48,66,6,10,4,103,49,57,49,66,6,10,4,103,49,57,50,66,6,10,4,103,49,57,51,66,6,10,4,103,49,57,52,66,6,10,4,103,49,57,53,66,6,10,4,103,49,57,54,66,6,10,4,103,49,57,55,66,6,10,4,103,49,57,56,66,6,10,4,103,49,57,57,66,6,10,4,103,50,48,48,66,6,10,4,103,50,48,49,66,6,10,4,103,50,48,50,66,6,10,4,103,50,48,51,66,6,10,4,103,50,48,52,66,6,10,4,103,50,48,53,66,6,10,4,103,50,48,54,66,6,10,4,103,50,48,55,66,6,10,4,103,50,48,56,66,6,10,4,103,50,48,57,66,6,10,4,103,50,49,48,66,6,10,4,103,50,49,49,66,6,10,4,103,50,49,50,66,6,10,4,103,50,49,51,66,6,10,4,103,50,49,52,66,6,10,4,103,50,49,53,66,6,10,4,103,50,49,54,66,6,10,4,103,50,49,55,66,6,10,4,103,50,49,56,66,6,10,4,103,50,49,57,66,6,10,4,103,50,50,48,66,6,10,4,103,50,50,49,66,6,10,4,103,50,50,50,66,6,10,4,103,50,50,51,66,6,10,4,103,50,50,52,66,6,10,4,103,50,50,53,66,6,10,4,103,50,50,54,66,6,10,4,103,50,50,55,66,6,10,4,103,50,50,56,66,6,10,4,103,50,50,57,66,6,10,4,103,50,51,48,66,6,10,4,103,50,51,49,66,6,10,4,103,50,51,50,66,6,10,4,103,50,51,51,66,6,10,4,103,50,51,52,66,6,10,4,103,50,51,53,66,6,10,4,103,50,51,54,66,6,10,4,103,50,51,55,66,6,10,4,103,50,51,56,66,6,10,4,103,50,51,57,66,6,10,4,103,50,52,48,66,6,10,4,103,50,52,49,66,6,10,4,103,50,52,50,66,6,10,4,103,50,52,51,66,6,10,4,103,50,52,52,66,6,10,4,103,50,52,53,66,6,10,4,103,50,52,54,66,6,10,4,103,50,52,55,66,6,10,4,103,50,52,56,66,6,10,4,103,50,52,57,66,6,10,4,103,50,53,48,66,6,10,4,103,50,53,49,66,6,10,4,103,50,53,50,66,6,10,4,103,50,53,51,66,6,10,4,103,50,53,52,66,6,10,4,103,50,53,53,66,6,10,4,103,50,53,54];

  D_message(state[..],cast(string)fixture[..],".p",0);
  #assert state[0].error=="descriptor oneof budget";
}
#test "per message fields" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[3621] u8.[10,1,77,18,10,10,2,120,48,24,1,32,1,40,5,18,10,10,2,120,49,24,2,32,1,40,5,18,10,10,2,120,50,24,3,32,1,40,5,18,10,10,2,120,51,24,4,32,1,40,5,18,10,10,2,120,52,24,5,32,1,40,5,18,10,10,2,120,53,24,6,32,1,40,5,18,10,10,2,120,54,24,7,32,1,40,5,18,10,10,2,120,55,24,8,32,1,40,5,18,10,10,2,120,56,24,9,32,1,40,5,18,10,10,2,120,57,24,10,32,1,40,5,18,11,10,3,120,49,48,24,11,32,1,40,5,18,11,10,3,120,49,49,24,12,32,1,40,5,18,11,10,3,120,49,50,24,13,32,1,40,5,18,11,10,3,120,49,51,24,14,32,1,40,5,18,11,10,3,120,49,52,24,15,32,1,40,5,18,11,10,3,120,49,53,24,16,32,1,40,5,18,11,10,3,120,49,54,24,17,32,1,40,5,18,11,10,3,120,49,55,24,18,32,1,40,5,18,11,10,3,120,49,56,24,19,32,1,40,5,18,11,10,3,120,49,57,24,20,32,1,40,5,18,11,10,3,120,50,48,24,21,32,1,40,5,18,11,10,3,120,50,49,24,22,32,1,40,5,18,11,10,3,120,50,50,24,23,32,1,40,5,18,11,10,3,120,50,51,24,24,32,1,40,5,18,11,10,3,120,50,52,24,25,32,1,40,5,18,11,10,3,120,50,53,24,26,32,1,40,5,18,11,10,3,120,50,54,24,27,32,1,40,5,18,11,10,3,120,50,55,24,28,32,1,40,5,18,11,10,3,120,50,56,24,29,32,1,40,5,18,11,10,3,120,50,57,24,30,32,1,40,5,18,11,10,3,120,51,48,24,31,32,1,40,5,18,11,10,3,120,51,49,24,32,32,1,40,5,18,11,10,3,120,51,50,24,33,32,1,40,5,18,11,10,3,120,51,51,24,34,32,1,40,5,18,11,10,3,120,51,52,24,35,32,1,40,5,18,11,10,3,120,51,53,24,36,32,1,40,5,18,11,10,3,120,51,54,24,37,32,1,40,5,18,11,10,3,120,51,55,24,38,32,1,40,5,18,11,10,3,120,51,56,24,39,32,1,40,5,18,11,10,3,120,51,57,24,40,32,1,40,5,18,11,10,3,120,52,48,24,41,32,1,40,5,18,11,10,3,120,52,49,24,42,32,1,40,5,18,11,10,3,120,52,50,24,43,32,1,40,5,18,11,10,3,120,52,51,24,44,32,1,40,5,18,11,10,3,120,52,52,24,45,32,1,40,5,18,11,10,3,120,52,53,24,46,32,1,40,5,18,11,10,3,120,52,54,24,47,32,1,40,5,18,11,10,3,120,52,55,24,48,32,1,40,5,18,11,10,3,120,52,56,24,49,32,1,40,5,18,11,10,3,120,52,57,24,50,32,1,40,5,18,11,10,3,120,53,48,24,51,32,1,40,5,18,11,10,3,120,53,49,24,52,32,1,40,5,18,11,10,3,120,53,50,24,53,32,1,40,5,18,11,10,3,120,53,51,24,54,32,1,40,5,18,11,10,3,120,53,52,24,55,32,1,40,5,18,11,10,3,120,53,53,24,56,32,1,40,5,18,11,10,3,120,53,54,24,57,32,1,40,5,18,11,10,3,120,53,55,24,58,32,1,40,5,18,11,10,3,120,53,56,24,59,32,1,40,5,18,11,10,3,120,53,57,24,60,32,1,40,5,18,11,10,3,120,54,48,24,61,32,1,40,5,18,11,10,3,120,54,49,24,62,32,1,40,5,18,11,10,3,120,54,50,24,63,32,1,40,5,18,11,10,3,120,54,51,24,64,32,1,40,5,18,11,10,3,120,54,52,24,65,32,1,40,5,18,11,10,3,120,54,53,24,66,32,1,40,5,18,11,10,3,120,54,54,24,67,32,1,40,5,18,11,10,3,120,54,55,24,68,32,1,40,5,18,11,10,3,120,54,56,24,69,32,1,40,5,18,11,10,3,120,54,57,24,70,32,1,40,5,18,11,10,3,120,55,48,24,71,32,1,40,5,18,11,10,3,120,55,49,24,72,32,1,40,5,18,11,10,3,120,55,50,24,73,32,1,40,5,18,11,10,3,120,55,51,24,74,32,1,40,5,18,11,10,3,120,55,52,24,75,32,1,40,5,18,11,10,3,120,55,53,24,76,32,1,40,5,18,11,10,3,120,55,54,24,77,32,1,40,5,18,11,10,3,120,55,55,24,78,32,1,40,5,18,11,10,3,120,55,56,24,79,32,1,40,5,18,11,10,3,120,55,57,24,80,32,1,40,5,18,11,10,3,120,56,48,24,81,32,1,40,5,18,11,10,3,120,56,49,24,82,32,1,40,5,18,11,10,3,120,56,50,24,83,32,1,40,5,18,11,10,3,120,56,51,24,84,32,1,40,5,18,11,10,3,120,56,52,24,85,32,1,40,5,18,11,10,3,120,56,53,24,86,32,1,40,5,18,11,10,3,120,56,54,24,87,32,1,40,5,18,11,10,3,120,56,55,24,88,32,1,40,5,18,11,10,3,120,56,56,24,89,32,1,40,5,18,11,10,3,120,56,57,24,90,32,1,40,5,18,11,10,3,120,57,48,24,91,32,1,40,5,18,11,10,3,120,57,49,24,92,32,1,40,5,18,11,10,3,120,57,50,24,93,32,1,40,5,18,11,10,3,120,57,51,24,94,32,1,40,5,18,11,10,3,120,57,52,24,95,32,1,40,5,18,11,10,3,120,57,53,24,96,32,1,40,5,18,11,10,3,120,57,54,24,97,32,1,40,5,18,11,10,3,120,57,55,24,98,32,1,40,5,18,11,10,3,120,57,56,24,99,32,1,40,5,18,11,10,3,120,57,57,24,100,32,1,40,5,18,12,10,4,120,49,48,48,24,101,32,1,40,5,18,12,10,4,120,49,48,49,24,102,32,1,40,5,18,12,10,4,120,49,48,50,24,103,32,1,40,5,18,12,10,4,120,49,48,51,24,104,32,1,40,5,18,12,10,4,120,49,48,52,24,105,32,1,40,5,18,12,10,4,120,49,48,53,24,106,32,1,40,5,18,12,10,4,120,49,48,54,24,107,32,1,40,5,18,12,10,4,120,49,48,55,24,108,32,1,40,5,18,12,10,4,120,49,48,56,24,109,32,1,40,5,18,12,10,4,120,49,48,57,24,110,32,1,40,5,18,12,10,4,120,49,49,48,24,111,32,1,40,5,18,12,10,4,120,49,49,49,24,112,32,1,40,5,18,12,10,4,120,49,49,50,24,113,32,1,40,5,18,12,10,4,120,49,49,51,24,114,32,1,40,5,18,12,10,4,120,49,49,52,24,115,32,1,40,5,18,12,10,4,120,49,49,53,24,116,32,1,40,5,18,12,10,4,120,49,49,54,24,117,32,1,40,5,18,12,10,4,120,49,49,55,24,118,32,1,40,5,18,12,10,4,120,49,49,56,24,119,32,1,40,5,18,12,10,4,120,49,49,57,24,120,32,1,40,5,18,12,10,4,120,49,50,48,24,121,32,1,40,5,18,12,10,4,120,49,50,49,24,122,32,1,40,5,18,12,10,4,120,49,50,50,24,123,32,1,40,5,18,12,10,4,120,49,50,51,24,124,32,1,40,5,18,12,10,4,120,49,50,52,24,125,32,1,40,5,18,12,10,4,120,49,50,53,24,126,32,1,40,5,18,12,10,4,120,49,50,54,24,127,32,1,40,5,18,13,10,4,120,49,50,55,24,128,1,32,1,40,5,18,13,10,4,120,49,50,56,24,129,1,32,1,40,5,18,13,10,4,120,49,50,57,24,130,1,32,1,40,5,18,13,10,4,120,49,51,48,24,131,1,32,1,40,5,18,13,10,4,120,49,51,49,24,132,1,32,1,40,5,18,13,10,4,120,49,51,50,24,133,1,32,1,40,5,18,13,10,4,120,49,51,51,24,134,1,32,1,40,5,18,13,10,4,120,49,51,52,24,135,1,32,1,40,5,18,13,10,4,120,49,51,53,24,136,1,32,1,40,5,18,13,10,4,120,49,51,54,24,137,1,32,1,40,5,18,13,10,4,120,49,51,55,24,138,1,32,1,40,5,18,13,10,4,120,49,51,56,24,139,1,32,1,40,5,18,13,10,4,120,49,51,57,24,140,1,32,1,40,5,18,13,10,4,120,49,52,48,24,141,1,32,1,40,5,18,13,10,4,120,49,52,49,24,142,1,32,1,40,5,18,13,10,4,120,49,52,50,24,143,1,32,1,40,5,18,13,10,4,120,49,52,51,24,144,1,32,1,40,5,18,13,10,4,120,49,52,52,24,145,1,32,1,40,5,18,13,10,4,120,49,52,53,24,146,1,32,1,40,5,18,13,10,4,120,49,52,54,24,147,1,32,1,40,5,18,13,10,4,120,49,52,55,24,148,1,32,1,40,5,18,13,10,4,120,49,52,56,24,149,1,32,1,40,5,18,13,10,4,120,49,52,57,24,150,1,32,1,40,5,18,13,10,4,120,49,53,48,24,151,1,32,1,40,5,18,13,10,4,120,49,53,49,24,152,1,32,1,40,5,18,13,10,4,120,49,53,50,24,153,1,32,1,40,5,18,13,10,4,120,49,53,51,24,154,1,32,1,40,5,18,13,10,4,120,49,53,52,24,155,1,32,1,40,5,18,13,10,4,120,49,53,53,24,156,1,32,1,40,5,18,13,10,4,120,49,53,54,24,157,1,32,1,40,5,18,13,10,4,120,49,53,55,24,158,1,32,1,40,5,18,13,10,4,120,49,53,56,24,159,1,32,1,40,5,18,13,10,4,120,49,53,57,24,160,1,32,1,40,5,18,13,10,4,120,49,54,48,24,161,1,32,1,40,5,18,13,10,4,120,49,54,49,24,162,1,32,1,40,5,18,13,10,4,120,49,54,50,24,163,1,32,1,40,5,18,13,10,4,120,49,54,51,24,164,1,32,1,40,5,18,13,10,4,120,49,54,52,24,165,1,32,1,40,5,18,13,10,4,120,49,54,53,24,166,1,32,1,40,5,18,13,10,4,120,49,54,54,24,167,1,32,1,40,5,18,13,10,4,120,49,54,55,24,168,1,32,1,40,5,18,13,10,4,120,49,54,56,24,169,1,32,1,40,5,18,13,10,4,120,49,54,57,24,170,1,32,1,40,5,18,13,10,4,120,49,55,48,24,171,1,32,1,40,5,18,13,10,4,120,49,55,49,24,172,1,32,1,40,5,18,13,10,4,120,49,55,50,24,173,1,32,1,40,5,18,13,10,4,120,49,55,51,24,174,1,32,1,40,5,18,13,10,4,120,49,55,52,24,175,1,32,1,40,5,18,13,10,4,120,49,55,53,24,176,1,32,1,40,5,18,13,10,4,120,49,55,54,24,177,1,32,1,40,5,18,13,10,4,120,49,55,55,24,178,1,32,1,40,5,18,13,10,4,120,49,55,56,24,179,1,32,1,40,5,18,13,10,4,120,49,55,57,24,180,1,32,1,40,5,18,13,10,4,120,49,56,48,24,181,1,32,1,40,5,18,13,10,4,120,49,56,49,24,182,1,32,1,40,5,18,13,10,4,120,49,56,50,24,183,1,32,1,40,5,18,13,10,4,120,49,56,51,24,184,1,32,1,40,5,18,13,10,4,120,49,56,52,24,185,1,32,1,40,5,18,13,10,4,120,49,56,53,24,186,1,32,1,40,5,18,13,10,4,120,49,56,54,24,187,1,32,1,40,5,18,13,10,4,120,49,56,55,24,188,1,32,1,40,5,18,13,10,4,120,49,56,56,24,189,1,32,1,40,5,18,13,10,4,120,49,56,57,24,190,1,32,1,40,5,18,13,10,4,120,49,57,48,24,191,1,32,1,40,5,18,13,10,4,120,49,57,49,24,192,1,32,1,40,5,18,13,10,4,120,49,57,50,24,193,1,32,1,40,5,18,13,10,4,120,49,57,51,24,194,1,32,1,40,5,18,13,10,4,120,49,57,52,24,195,1,32,1,40,5,18,13,10,4,120,49,57,53,24,196,1,32,1,40,5,18,13,10,4,120,49,57,54,24,197,1,32,1,40,5,18,13,10,4,120,49,57,55,24,198,1,32,1,40,5,18,13,10,4,120,49,57,56,24,199,1,32,1,40,5,18,13,10,4,120,49,57,57,24,200,1,32,1,40,5,18,13,10,4,120,50,48,48,24,201,1,32,1,40,5,18,13,10,4,120,50,48,49,24,202,1,32,1,40,5,18,13,10,4,120,50,48,50,24,203,1,32,1,40,5,18,13,10,4,120,50,48,51,24,204,1,32,1,40,5,18,13,10,4,120,50,48,52,24,205,1,32,1,40,5,18,13,10,4,120,50,48,53,24,206,1,32,1,40,5,18,13,10,4,120,50,48,54,24,207,1,32,1,40,5,18,13,10,4,120,50,48,55,24,208,1,32,1,40,5,18,13,10,4,120,50,48,56,24,209,1,32,1,40,5,18,13,10,4,120,50,48,57,24,210,1,32,1,40,5,18,13,10,4,120,50,49,48,24,211,1,32,1,40,5,18,13,10,4,120,50,49,49,24,212,1,32,1,40,5,18,13,10,4,120,50,49,50,24,213,1,32,1,40,5,18,13,10,4,120,50,49,51,24,214,1,32,1,40,5,18,13,10,4,120,50,49,52,24,215,1,32,1,40,5,18,13,10,4,120,50,49,53,24,216,1,32,1,40,5,18,13,10,4,120,50,49,54,24,217,1,32,1,40,5,18,13,10,4,120,50,49,55,24,218,1,32,1,40,5,18,13,10,4,120,50,49,56,24,219,1,32,1,40,5,18,13,10,4,120,50,49,57,24,220,1,32,1,40,5,18,13,10,4,120,50,50,48,24,221,1,32,1,40,5,18,13,10,4,120,50,50,49,24,222,1,32,1,40,5,18,13,10,4,120,50,50,50,24,223,1,32,1,40,5,18,13,10,4,120,50,50,51,24,224,1,32,1,40,5,18,13,10,4,120,50,50,52,24,225,1,32,1,40,5,18,13,10,4,120,50,50,53,24,226,1,32,1,40,5,18,13,10,4,120,50,50,54,24,227,1,32,1,40,5,18,13,10,4,120,50,50,55,24,228,1,32,1,40,5,18,13,10,4,120,50,50,56,24,229,1,32,1,40,5,18,13,10,4,120,50,50,57,24,230,1,32,1,40,5,18,13,10,4,120,50,51,48,24,231,1,32,1,40,5,18,13,10,4,120,50,51,49,24,232,1,32,1,40,5,18,13,10,4,120,50,51,50,24,233,1,32,1,40,5,18,13,10,4,120,50,51,51,24,234,1,32,1,40,5,18,13,10,4,120,50,51,52,24,235,1,32,1,40,5,18,13,10,4,120,50,51,53,24,236,1,32,1,40,5,18,13,10,4,120,50,51,54,24,237,1,32,1,40,5,18,13,10,4,120,50,51,55,24,238,1,32,1,40,5,18,13,10,4,120,50,51,56,24,239,1,32,1,40,5,18,13,10,4,120,50,51,57,24,240,1,32,1,40,5,18,13,10,4,120,50,52,48,24,241,1,32,1,40,5,18,13,10,4,120,50,52,49,24,242,1,32,1,40,5,18,13,10,4,120,50,52,50,24,243,1,32,1,40,5,18,13,10,4,120,50,52,51,24,244,1,32,1,40,5,18,13,10,4,120,50,52,52,24,245,1,32,1,40,5,18,13,10,4,120,50,52,53,24,246,1,32,1,40,5,18,13,10,4,120,50,52,54,24,247,1,32,1,40,5,18,13,10,4,120,50,52,55,24,248,1,32,1,40,5,18,13,10,4,120,50,52,56,24,249,1,32,1,40,5,18,13,10,4,120,50,52,57,24,250,1,32,1,40,5,18,13,10,4,120,50,53,48,24,251,1,32,1,40,5,18,13,10,4,120,50,53,49,24,252,1,32,1,40,5,18,13,10,4,120,50,53,50,24,253,1,32,1,40,5,18,13,10,4,120,50,53,51,24,254,1,32,1,40,5,18,13,10,4,120,50,53,52,24,255,1,32,1,40,5,18,13,10,4,120,50,53,53,24,128,2,32,1,40,5,18,13,10,4,120,50,53,54,24,129,2,32,1,40,5];

  D_message(state[..],cast(string)fixture[..],".p",0);
  #assert state[0].error=="descriptor field budget exceeded";
}
#test "initial depth admission" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[3] u8.[10,1,77];

  D_message(state[..],cast(string)fixture[..],".p",32);
  #assert state[0].error=="descriptor nesting budget";
}
#test "all thirty two nesting levels" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[257] u8.[10,254,1,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,225,1,10,2,78,48,26,218,1,10,2,78,49,26,211,1,10,2,78,50,26,204,1,10,2,78,51,26,197,1,10,2,78,52,26,190,1,10,2,78,53,26,183,1,10,2,78,54,26,176,1,10,2,78,55,26,169,1,10,2,78,56,26,162,1,10,2,78,57,26,155,1,10,3,78,49,48,26,147,1,10,3,78,49,49,26,139,1,10,3,78,49,50,26,131,1,10,3,78,49,51,26,124,10,3,78,49,52,26,117,10,3,78,49,53,26,110,10,3,78,49,54,26,103,10,3,78,49,55,26,96,10,3,78,49,56,26,89,10,3,78,49,57,26,82,10,3,78,50,48,26,75,10,3,78,50,49,26,68,10,3,78,50,50,26,61,10,3,78,50,51,26,54,10,3,78,50,52,26,47,10,3,78,50,53,26,40,10,3,78,50,54,26,33,10,3,78,50,55,26,26,10,3,78,50,56,26,19,10,3,78,50,57,26,12,10,3,78,51,48,26,5,10,3,78,51,49,98,6,112,114,111,116,111,51];

  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="" && state[0].nt==32;
}
#test "file name admits 4096 bytes" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[4121] u8.[10,150,32,10,128,32,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,120,18,4,116,101,115,116,34,3,10,1,77,98,6,112,114,111,116,111,51];

  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="" && state[0].np==1;
}
#test "optional flag on first group" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[54] u8.[10,52,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,24,10,1,77,18,14,10,1,120,24,1,32,1,40,5,72,0,136,1,1,66,3,10,1,103,98,6,112,114,111,116,111,51];

  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="" && types[0].group_count==1 && fields[0].optional && state[0].ng==1 && state[0].visited==15;
}
#test "reject optional two in a valid group" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[54] u8.[10,52,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,24,10,1,77,18,14,10,1,120,24,1,32,1,40,5,72,0,136,1,2,66,3,10,1,103,98,6,112,114,111,116,111,51];

  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid proto3 optional flag";
}
#test "reject oneof at group count" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[51] u8.[10,49,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,21,10,1,77,18,11,10,1,120,24,1,32,1,40,5,72,1,66,3,10,1,103,98,6,112,114,111,116,111,51];

  D_parse(state[..],cast(string)fixture[..],"P");
  #assert state[0].error=="invalid oneof";
}
#test "maximum policy integer" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[14] u8.[58,12,200,243,24,255,255,255,255,255,255,255,255,127];

  o:=D_options(state[..],cast(string)fixture[..],7,true);
  #assert state[0].error=="" && o.n[1]==9223372036854775807 && state[0].visited==1;
}
#test "map flag wire type" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[5] u8.[58,3,58,1,120];

  o:=D_options(state[..],cast(string)fixture[..],7,true);
  #assert state[0].error=="invalid numeric policy option";
}
#test "generate reports parsing failure" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[33] u8.[10,31,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,3,10,1,77,98,6,112,114,111,116,111,51];

  n:=G_generate(state[..],cast(string)fixture[..],"bad-","");
  #assert state[0].error=="descriptor byte budget or invalid prefix" && n==0;
}
#test "generate reports output exhaustion" {
  types:[40] D_Type;enums:[4] D_Type;fields:[300] D_Field;values:[8] D_Value;groups:[300] string;files:[8] string;deps:[8] string;
  arena:[16384] u8;output:[65536] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  fixture:=[33] u8.[10,31,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,3,10,1,77,98,6,112,114,111,116,111,51];
  state[0].output=output[0..<0];
  n:=G_generate(state[..],cast(string)fixture[..],"P","");
  #assert state[0].error=="generated source budget" && n==0;
}
#test "scalar fixture descriptor parser and generator" {
  fixture:=[761] u8.[10,246,5,10,11,116,121,112,101,115,46,112,114,111,116,111,18,9,99,111,97,120,46,116,101,115,116,34,27,10,5,73,110,110,101,114,18,18,10,4,116,101,120,116,24,1,32,1,40,9,82,4,116,101,120,116,34,254,4,10,7,83,99,97,108,97,114,115,18,22,10,6,114,101,97,108,54,52,24,1,32,1,40,1,82,6,114,101,97,108,54,52,18,22,10,6,114,101,97,108,51,50,24,2,32,1,40,2,82,6,114,101,97,108,51,50,18,16,10,3,105,54,52,24,3,32,1,40,3,82,3,105,54,52,18,16,10,3,117,54,52,24,4,32,1,40,4,82,3,117,54,52,18,16,10,3,105,51,50,24,5,32,1,40,5,82,3,105,51,50,18,35,10,13,102,105,120,101,100,54,52,95,118,97,108,117,101,24,6,32,1,40,6,82,12,102,105,120,101,100,54,52,86,97,108,117,101,18,35,10,13,102,105,120,101,100,51,50,95,118,97,108,117,101,24,7,32,1,40,7,82,12,102,105,120,101,100,51,50,86,97,108,117,101,18,24,10,7,101,110,97,98,108,101,100,24,8,32,1,40,8,82,7,101,110,97,98,108,101,100,18,18,10,4,116,101,120,116,24,9,32,1,40,9,82,4,116,101,120,116,18,18,10,4,98,108,111,98,24,10,32,1,40,12,82,4,98,108,111,98,18,16,10,3,117,51,50,24,11,32,1,40,13,82,3,117,51,50,18,38,10,5,115,116,97,116,101,24,12,32,1,40,14,50,16,46,99,111,97,120,46,116,101,115,116,46,83,116,97,116,101,82,5,115,116,97,116,101,18,18,10,4,115,102,51,50,24,13,32,1,40,15,82,4,115,102,51,50,18,18,10,4,115,102,54,52,24,14,32,1,40,16,82,4,115,102,54,52,18,20,10,5,122,105,103,51,50,24,15,32,1,40,17,82,5,122,105,103,51,50,18,20,10,5,122,105,103,54,52,24,16,32,1,40,18,82,5,122,105,103,54,52,18,24,10,7,110,117,109,98,101,114,115,24,17,32,3,40,17,82,7,110,117,109,98,101,114,115,18,38,10,5,99,104,105,108,100,24,18,32,1,40,11,50,16,46,99,111,97,120,46,116,101,115,116,46,73,110,110,101,114,82,5,99,104,105,108,100,18,44,10,8,99,104,105,108,100,114,101,110,24,19,32,3,40,11,50,16,46,99,111,97,120,46,116,101,115,116,46,73,110,110,101,114,82,8,99,104,105,108,100,114,101,110,18,22,10,5,116,105,116,108,101,24,20,32,1,40,9,72,0,82,5,116,105,116,108,101,18,20,10,4,114,97,110,107,24,21,32,1,40,4,72,0,82,4,114,97,110,107,18,21,10,3,111,112,116,24,22,32,1,40,13,72,1,82,3,111,112,116,136,1,1,18,44,10,7,114,101,99,117,114,115,101,24,23,32,1,40,11,50,18,46,99,111,97,120,46,116,101,115,116,46,83,99,97,108,97,114,115,82,7,114,101,99,117,114,115,101,66,8,10,6,99,104,111,105,99,101,66,6,10,4,95,111,112,116,42,54,10,5,83,116,97,116,101,18,11,10,7,85,78,75,78,79,87,78,16,0,18,9,10,5,82,69,65,68,89,16,1,18,21,10,8,78,69,71,65,84,73,86,69,16,255,255,255,255,255,255,255,255,255,1,98,6,112,114,111,116,111,51];
  types:[8] D_Type;enums:[8] D_Type;fields:[64] D_Field;values:[16] D_Value;groups:[16] string;files:[8] string;deps:[8] string;
  arena:[262144] u8;output:[131072] u8;symbols:[256] string;
  state:=[1] D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  n:=G_generate(state[..],cast(string)fixture[..],"T","");
  #assert state[0].error=="" && n==55231;
  #assert cast(string)output[0..<512]=="\n// @proto-descriptor-declarations\nT_State_UNKNOWN :: 0;\nT_State_READY :: 1;\nT_State_NEGATIVE :: -1;\nT_State_known :: (value:s32)->bool {return value==0 || value==1 || value==-1;}\n// .coax.test.Inner -> T_Inner\nT_Inner :: struct {wire:[] u8;error:u32;}\nT_Inner_Option :: struct {value:T_Inner;present:bool;error:u32;}\n// .coax.test.Scalars -> T_Scalars\nT_Scalars :: struct {wire:[] u8;error:u32;}\nT_Scalars_Option :: struct {value:T_Scalars;present:bool;error:u32;}\nT_Inner__check :: (wire:[] u8,depth:int,budget";
  #assert cast(string)output[512..<1024]==":int,required:bool,canonical:bool)->T__pb_Check {\nif !T__pb_message_admitted(depth,wire.count) {return T__pb_Check.{error=T__pb_LIMIT};}\nleft:=budget;pos:=0;\nseen_text:bool=false;\nwhile pos<wire.count {\nif left<=0 {return T__pb_Check.{error=T__pb_LIMIT};} left-=1;\nf:=T__pb_scan(wire,pos);if f.error!=0 {return T__pb_Check.{error=f.error};} if canonical && !T__pb_minimal(wire,f) {return T__pb_Check.{error=T__pb_WIRE};} pos=f.next;\nif f.number==1 {\nif seen_text {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif ";
  #assert cast(string)output[1024..<1536]=="f.wt!=2 { return T__pb_Check.{error=T__pb_WIRE}; }\nif !T__pb_utf8(f.payload) { return T__pb_Check.{error=T__pb_UTF8}; }\nseen_text=true;\n}\n}\nreturn T__pb_Check.{left=left};}\nT_Inner_new :: ()->T_Inner {return T_Inner.{};}\nT_Inner_decode :: (wire:[] u8)->T_Inner {c:=T_Inner__check(wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_require_fields(),false);if c.error!=0 {return T_Inner.{error=c.error};}return T_Inner.{wire=wire};}\nT_Inner_decode_minimal :: (wire:[] u8)->T_Inner {c:=T_Inner__check(wire,T__pb_r";
  #assert cast(string)output[1536..<2048]=="oot_depth(),T__pb_default_fields(),T__pb_require_fields(),true);if c.error!=0 {return T_Inner.{error=c.error};}return T_Inner.{wire=wire};}\nT_Inner_encode :: (msg:T_Inner,out:[] u8)->T__pb_View {if msg.error!=0 {return T__pb_View.{error=msg.error};} c:=T_Inner__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_require_fields(),false);if c.error!=0 {return T__pb_View.{error=c.error};} return T__pb_copy(msg.wire,out);}\nT_Inner_unknown :: (msg:T_Inner,cursor:int)->T__pb_Field {if msg.error!=0 {ret";
  #assert cast(string)output[2048..<2560]=="urn T__pb_Field.{error=msg.error};}checked:=T_Inner_decode(msg.wire);if checked.error!=0 {return T__pb_Field.{error=checked.error};}pos:=cursor;while true {f:=T__pb_scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(f.number==1) {return f;}}return T__pb_Field.{done=true};}\nT_Inner_FIELD_text :: 1;\nT_Inner_get_text :: (msg:T_Inner)->T__pb_String {\nif msg.error!=0 {return T__pb_String.{error=msg.error};}checked:=T_Inner_decode(msg.wire);if checked.error!=0 {return T__pb_String.{error=checke";
  #assert cast(string)output[2560..<3072]=="d.error};}\nf:=T__pb_find(msg.wire,1);return T__pb_String.{value=cast(string) f.payload,present=!f.done,error=f.error};}\nT_Inner_set_text :: (msg:T_Inner,value:string,out:[] u8)->T_Inner {\nif msg.error!=0 {return T_Inner.{error=msg.error};} checked:=T_Inner__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Inner.{error=checked.error};}\nremoved_fields:= [1] u32.[1];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\ntxt=value;is_text=true;\nresult:=";
  #assert cast(string)output[3072..<3584]=="T__pb_rewrite(msg.wire,removed_fields[..],1,2,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Inner.{error=result.error};} final:=T_Inner__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Inner.{error=final.error};}return T_Inner.{wire=result.wire};}\nT_Inner_clear_text :: (msg:T_Inner,out:[] u8)->T_Inner {\nif msg.error!=0 {return T_Inner.{error=msg.error};} checked:=T_Inner__check(msg.wire,T__pb_root_depth(),T__pb_default_fie";
  #assert cast(string)output[3584..<4096]=="lds(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Inner.{error=checked.error};}\nremoved_fields:= [1] u32.[1];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],1,2,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Inner.{error=result.error};} final:=T_Inner__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Inner.{error=final.error};}return T_Inner.{wire=result.wire};}\n";
  #assert cast(string)output[4096..<4608]=="T_Scalars__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->T__pb_Check {\nif !T__pb_message_admitted(depth,wire.count) {return T__pb_Check.{error=T__pb_LIMIT};}\nleft:=budget;pos:=0;\nseen_real64:bool=false;\nseen_real32:bool=false;\nseen_i64:bool=false;\nseen_u64:bool=false;\nseen_i32:bool=false;\nseen_fixed64_value:bool=false;\nseen_fixed32_value:bool=false;\nseen_enabled:bool=false;\nseen_text:bool=false;\nseen_blob:bool=false;\nseen_u32:bool=false;\nseen_state:bool=false;\nseen_sf32:bool=false";
  #assert cast(string)output[4608..<5120]==";\nseen_sf64:bool=false;\nseen_zig32:bool=false;\nseen_zig64:bool=false;\nseen_child:bool=false;\nseen_opt:bool=false;\nseen_recurse:bool=false;\none_0:bool=false;\nwhile pos<wire.count {\nif left<=0 {return T__pb_Check.{error=T__pb_LIMIT};} left-=1;\nf:=T__pb_scan(wire,pos);if f.error!=0 {return T__pb_Check.{error=f.error};} if canonical && !T__pb_minimal(wire,f) {return T__pb_Check.{error=T__pb_WIRE};} pos=f.next;\nif f.number==1 {\nif seen_real64 {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=1 { return T__p";
  #assert cast(string)output[5120..<5632]=="b_Check.{error=T__pb_WIRE}; }\nseen_real64=true;\n}\nelse if f.number==2 {\nif seen_real32 {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=5 { return T__pb_Check.{error=T__pb_WIRE}; }\nseen_real32=true;\n}\nelse if f.number==3 {\nif seen_i64 {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=0 { return T__pb_Check.{error=T__pb_WIRE}; }\nseen_i64=true;\n}\nelse if f.number==4 {\nif seen_u64 {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=0 { return T__pb_Check.{error=T__pb_WIRE}; }\nseen_u64=true;\n}\nels";
  #assert cast(string)output[5632..<6144]=="e if f.number==5 {\nif seen_i32 {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=0 { return T__pb_Check.{error=T__pb_WIRE}; }\nseen_i32=true;\n}\nelse if f.number==6 {\nif seen_fixed64_value {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=1 { return T__pb_Check.{error=T__pb_WIRE}; }\nseen_fixed64_value=true;\n}\nelse if f.number==7 {\nif seen_fixed32_value {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=5 { return T__pb_Check.{error=T__pb_WIRE}; }\nif f.raw>4294967295 { return T__pb_Check.{error=T";
  #assert cast(string)output[6144..<6656]=="__pb_VALUE}; }\nseen_fixed32_value=true;\n}\nelse if f.number==8 {\nif seen_enabled {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=0 { return T__pb_Check.{error=T__pb_WIRE}; }\nif canonical && f.raw>1 { return T__pb_Check.{error=T__pb_VALUE}; }\nseen_enabled=true;\n}\nelse if f.number==9 {\nif seen_text {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=2 { return T__pb_Check.{error=T__pb_WIRE}; }\nif !T__pb_utf8(f.payload) { return T__pb_Check.{error=T__pb_UTF8}; }\nseen_text=true;\n}\nelse if f.number==10 ";
  #assert cast(string)output[6656..<7168]=="{\nif seen_blob {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=2 { return T__pb_Check.{error=T__pb_WIRE}; }\nseen_blob=true;\n}\nelse if f.number==11 {\nif seen_u32 {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=0 { return T__pb_Check.{error=T__pb_WIRE}; }\nif f.raw>4294967295 { return T__pb_Check.{error=T__pb_VALUE}; }\nseen_u32=true;\n}\nelse if f.number==12 {\nif seen_state {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=0 { return T__pb_Check.{error=T__pb_WIRE}; }\nseen_state=true;\n}\nelse if";
  #assert cast(string)output[7168..<7680]==" f.number==13 {\nif seen_sf32 {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=5 { return T__pb_Check.{error=T__pb_WIRE}; }\nseen_sf32=true;\n}\nelse if f.number==14 {\nif seen_sf64 {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=1 { return T__pb_Check.{error=T__pb_WIRE}; }\nseen_sf64=true;\n}\nelse if f.number==15 {\nif seen_zig32 {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=0 { return T__pb_Check.{error=T__pb_WIRE}; }\nif f.raw>4294967295 { return T__pb_Check.{error=T__pb_VALUE}; }\nseen_zig32";
  #assert cast(string)output[7680..<8192]=="=true;\n}\nelse if f.number==16 {\nif seen_zig64 {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=0 { return T__pb_Check.{error=T__pb_WIRE}; }\nseen_zig64=true;\n}\nelse if f.number==17 {\nif f.wt==2 {packed:=f.payload;at:=0;while at<packed.count {\nif left<=0 {return T__pb_Check.{error=T__pb_LIMIT};} left-=1; f=T__pb_scalar(packed,at,0);if f.error!=0 {return T__pb_Check.{error=f.error};}\nif canonical && T__pb_vsize(f.raw)!=f.next-at {return T__pb_Check.{error=T__pb_WIRE};}\nat=f.next;\nf.wt=0;\nif f.wt!=0 { ret";
  #assert cast(string)output[8192..<8704]=="urn T__pb_Check.{error=T__pb_WIRE}; }\nif f.raw>4294967295 { return T__pb_Check.{error=T__pb_VALUE}; }\n}} else {\nif f.wt!=0 { return T__pb_Check.{error=T__pb_WIRE}; }\nif f.raw>4294967295 { return T__pb_Check.{error=T__pb_VALUE}; }\n}\n}\nelse if f.number==18 {\nif seen_child {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=2 { return T__pb_Check.{error=T__pb_WIRE}; }\nseen_child=true;\nchild:=T_Inner__check(f.payload,depth+1,left,T__pb_require_fields(),canonical);if child.error!=0 {return T__pb_Check.{error=";
  #assert cast(string)output[8704..<9216]=="child.error};} left=child.left;\n}\nelse if f.number==19 {\nif f.wt!=2 { return T__pb_Check.{error=T__pb_WIRE}; }\nchild:=T_Inner__check(f.payload,depth+1,left,T__pb_require_fields(),canonical);if child.error!=0 {return T__pb_Check.{error=child.error};} left=child.left;\n}\nelse if f.number==20 {\nif one_0 {return T__pb_Check.{error=T__pb_DUPLICATE};} one_0=true;\nif f.wt!=2 { return T__pb_Check.{error=T__pb_WIRE}; }\nif !T__pb_utf8(f.payload) { return T__pb_Check.{error=T__pb_UTF8}; }\n}\nelse if f.number==21 {\nif on";
  #assert cast(string)output[9216..<9728]=="e_0 {return T__pb_Check.{error=T__pb_DUPLICATE};} one_0=true;\nif f.wt!=0 { return T__pb_Check.{error=T__pb_WIRE}; }\n}\nelse if f.number==22 {\nif seen_opt {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=0 { return T__pb_Check.{error=T__pb_WIRE}; }\nif f.raw>4294967295 { return T__pb_Check.{error=T__pb_VALUE}; }\nseen_opt=true;\n}\nelse if f.number==23 {\nif seen_recurse {return T__pb_Check.{error=T__pb_DUPLICATE};}\nif f.wt!=2 { return T__pb_Check.{error=T__pb_WIRE}; }\nseen_recurse=true;\nchild:=T_Scalars__ch";
  #assert cast(string)output[9728..<10240]=="eck(f.payload,depth+1,left,T__pb_require_fields(),canonical);if child.error!=0 {return T__pb_Check.{error=child.error};} left=child.left;\n}\n}\nreturn T__pb_Check.{left=left};}\nT_Scalars_new :: ()->T_Scalars {return T_Scalars.{};}\nT_Scalars_decode :: (wire:[] u8)->T_Scalars {c:=T_Scalars__check(wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_require_fields(),false);if c.error!=0 {return T_Scalars.{error=c.error};}return T_Scalars.{wire=wire};}\nT_Scalars_decode_minimal :: (wire:[] u8)->T_Scalars {c:=T_Sca";
  #assert cast(string)output[10240..<10752]=="lars__check(wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_require_fields(),true);if c.error!=0 {return T_Scalars.{error=c.error};}return T_Scalars.{wire=wire};}\nT_Scalars_encode :: (msg:T_Scalars,out:[] u8)->T__pb_View {if msg.error!=0 {return T__pb_View.{error=msg.error};} c:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_require_fields(),false);if c.error!=0 {return T__pb_View.{error=c.error};} return T__pb_copy(msg.wire,out);}\nT_Scalars_case_choice :: (msg:T_Scalars)->T_";
  #assert cast(string)output[10752..<11264]=="_pb_U32 {if msg.error!=0 {return T__pb_U32.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_U32.{error=checked.error};}pos:=0;while pos<msg.wire.count {f:=T__pb_scan(msg.wire,pos);pos=f.next;if f.number==20 || f.number==21 {return T__pb_U32.{value=f.number,present=true};}} return T__pb_U32.{};}\nT_Scalars_unknown :: (msg:T_Scalars,cursor:int)->T__pb_Field {if msg.error!=0 {return T__pb_Field.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {re";
  #assert cast(string)output[11264..<11776]=="turn T__pb_Field.{error=checked.error};}pos:=cursor;while true {f:=T__pb_scan(msg.wire,pos);if f.error!=0 || f.done {return f;}pos=f.next;if !(f.number==1 || f.number==2 || f.number==3 || f.number==4 || f.number==5 || f.number==6 || f.number==7 || f.number==8 || f.number==9 || f.number==10 || f.number==11 || f.number==12 || f.number==13 || f.number==14 || f.number==15 || f.number==16 || f.number==17 || f.number==18 || f.number==19 || f.number==20 || f.number==21 || f.number==22 || f.number==23) {return f;}}";
  #assert cast(string)output[11776..<12288]=="return T__pb_Field.{done=true};}\nT_Scalars_FIELD_real64 :: 1;\nT_Scalars_get_real64 :: (msg:T_Scalars)->T__pb_F64 {\nif msg.error!=0 {return T__pb_F64.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_F64.{error=checked.error};}\nf:=T__pb_find(msg.wire,1);return T__pb_F64.{value=f64_from_bits(f.raw),present=!f.done,error=f.error};}\nT_Scalars_set_real64 :: (msg:T_Scalars,value:f64,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scal";
  #assert cast(string)output[12288..<12800]=="ars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[1];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast(u64) f64_bits(value);\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],1,1,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),fals";
  #assert cast(string)output[12800..<13312]=="e);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_real64 :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[1];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],1";
  #assert cast(string)output[13312..<13824]==",1,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_real32 :: 2;\nT_Scalars_get_real32 :: (msg:T_Scalars)->T__pb_F32 {\nif msg.error!=0 {return T__pb_F32.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_F32.{erro";
  #assert cast(string)output[13824..<14336]=="r=checked.error};}\nf:=T__pb_find(msg.wire,2);return T__pb_F32.{value=f32_from_bits(cast(u32) f.raw),present=!f.done,error=f.error};}\nT_Scalars_set_real32 :: (msg:T_Scalars,value:f32,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[2];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast";
  #assert cast(string)output[14336..<14848]=="(u64) f32_bits(value);\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],2,5,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_real32 :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__";
  #assert cast(string)output[14848..<15360]=="check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[2];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],2,5,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalar";
  #assert cast(string)output[15360..<15872]=="s.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_i64 :: 3;\nT_Scalars_get_i64 :: (msg:T_Scalars)->T__pb_S64 {\nif msg.error!=0 {return T__pb_S64.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_S64.{error=checked.error};}\nf:=T__pb_find(msg.wire,3);return T__pb_S64.{value=cast,trunc(s64) f.raw,present=!f.done,error=f.error};}\nT_Scalars_set_i64 :: (msg:T_Scalars,value:s64,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.erro";
  #assert cast(string)output[15872..<16384]=="r};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[3];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast,trunc(u64) value;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],3,0,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_parti";
  #assert cast(string)output[16384..<16896]=="al_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_i64 :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[3];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed";
  #assert cast(string)output[16896..<17408]=="_fields[..],3,0,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_u64 :: 4;\nT_Scalars_get_u64 :: (msg:T_Scalars)->T__pb_U64 {\nif msg.error!=0 {return T__pb_U64.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_U6";
  #assert cast(string)output[17408..<17920]=="4.{error=checked.error};}\nf:=T__pb_find(msg.wire,4);return T__pb_U64.{value=f.raw,present=!f.done,error=f.error};}\nT_Scalars_set_u64 :: (msg:T_Scalars,value:u64,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[4];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast(u64) value;\nresult:=";
  #assert cast(string)output[17920..<18432]=="T__pb_rewrite(msg.wire,removed_fields[..],4,0,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_u64 :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),";
  #assert cast(string)output[18432..<18944]=="T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[4];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],4,0,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Sc";
  #assert cast(string)output[18944..<19456]=="alars.{wire=result.wire};}\nT_Scalars_FIELD_i32 :: 5;\nT_Scalars_get_i32 :: (msg:T_Scalars)->T__pb_S32 {\nif msg.error!=0 {return T__pb_S32.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_S32.{error=checked.error};}\nf:=T__pb_find(msg.wire,5);return T__pb_S32.{value=cast,trunc(s32) f.raw,present=!f.done,error=f.error};}\nT_Scalars_set_i32 :: (msg:T_Scalars,value:s32,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg";
  #assert cast(string)output[19456..<19968]==".wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[5];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast,trunc(u64) value;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],5,0,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!";
  #assert cast(string)output[19968..<20480]=="=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_i32 :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[5];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],5,0,raw,data,txt,is_te";
  #assert cast(string)output[20480..<20992]=="xt,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_fixed64_value :: 6;\nT_Scalars_get_fixed64_value :: (msg:T_Scalars)->T__pb_U64 {\nif msg.error!=0 {return T__pb_U64.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_U64.{error=check";
  #assert cast(string)output[20992..<21504]=="ed.error};}\nf:=T__pb_find(msg.wire,6);return T__pb_U64.{value=f.raw,present=!f.done,error=f.error};}\nT_Scalars_set_fixed64_value :: (msg:T_Scalars,value:u64,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[6];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast(u64) value;\nresult:=T__p";
  #assert cast(string)output[21504..<22016]=="b_rewrite(msg.wire,removed_fields[..],6,1,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_fixed64_value :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_de";
  #assert cast(string)output[22016..<22528]=="pth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[6];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],6,1,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}retur";
  #assert cast(string)output[22528..<23040]=="n T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_fixed32_value :: 7;\nT_Scalars_get_fixed32_value :: (msg:T_Scalars)->T__pb_U32 {\nif msg.error!=0 {return T__pb_U32.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_U32.{error=checked.error};}\nf:=T__pb_find(msg.wire,7);return T__pb_U32.{value=cast(u32) f.raw,present=!f.done,error=f.error};}\nT_Scalars_set_fixed32_value :: (msg:T_Scalars,value:u32,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};}";
  #assert cast(string)output[23040..<23552]==" checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[7];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast(u64) value;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],7,5,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(";
  #assert cast(string)output[23552..<24064]=="),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_fixed32_value :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[7];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed";
  #assert cast(string)output[24064..<24576]=="_fields[..],7,5,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_enabled :: 8;\nT_Scalars_get_enabled :: (msg:T_Scalars)->T__pb_Bool {\nif msg.error!=0 {return T__pb_Bool.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {retur";
  #assert cast(string)output[24576..<25088]=="n T__pb_Bool.{error=checked.error};}\nf:=T__pb_find(msg.wire,8);return T__pb_Bool.{value=f.raw!=0,present=!f.done,error=f.error};}\nT_Scalars_set_enabled :: (msg:T_Scalars,value:bool,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[8];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nif value ";
  #assert cast(string)output[25088..<25600]=="{raw=1;}\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],8,0,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_enabled :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wir";
  #assert cast(string)output[25600..<26112]=="e,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[8];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],8,0,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=fina";
  #assert cast(string)output[26112..<26624]=="l.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_text :: 9;\nT_Scalars_get_text :: (msg:T_Scalars)->T__pb_String {\nif msg.error!=0 {return T__pb_String.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_String.{error=checked.error};}\nf:=T__pb_find(msg.wire,9);return T__pb_String.{value=cast(string) f.payload,present=!f.done,error=f.error};}\nT_Scalars_set_text :: (msg:T_Scalars,value:string,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=ms";
  #assert cast(string)output[26624..<27136]=="g.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[9];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\ntxt=value;is_text=true;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],9,2,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_pa";
  #assert cast(string)output[27136..<27648]=="rtial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_text :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[9];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,rem";
  #assert cast(string)output[27648..<28160]=="oved_fields[..],9,2,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_blob :: 10;\nT_Scalars_get_blob :: (msg:T_Scalars)->T__pb_Bytes {\nif msg.error!=0 {return T__pb_Bytes.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {retu";
  #assert cast(string)output[28160..<28672]=="rn T__pb_Bytes.{error=checked.error};}\nf:=T__pb_find(msg.wire,10);return T__pb_Bytes.{value=f.payload,present=!f.done,error=f.error};}\nT_Scalars_set_blob :: (msg:T_Scalars,value:[] u8,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[10];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\ndata=";
  #assert cast(string)output[28672..<29184]=="value;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],10,2,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_blob :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T_";
  #assert cast(string)output[29184..<29696]=="_pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[10];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],10,2,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.";
  #assert cast(string)output[29696..<30208]=="error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_u32 :: 11;\nT_Scalars_get_u32 :: (msg:T_Scalars)->T__pb_U32 {\nif msg.error!=0 {return T__pb_U32.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_U32.{error=checked.error};}\nf:=T__pb_find(msg.wire,11);return T__pb_U32.{value=cast(u32) f.raw,present=!f.done,error=f.error};}\nT_Scalars_set_u32 :: (msg:T_Scalars,value:u32,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Sca";
  #assert cast(string)output[30208..<30720]=="lars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[11];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast(u64) value;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],11,0,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if f";
  #assert cast(string)output[30720..<31232]=="inal.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_u32 :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[11];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],11,0,raw,d";
  #assert cast(string)output[31232..<31744]=="ata,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_state :: 12;\nT_Scalars_get_state :: (msg:T_Scalars)->T__pb_S32 {\nif msg.error!=0 {return T__pb_S32.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_S32.{error=checked";
  #assert cast(string)output[31744..<32256]==".error};}\nf:=T__pb_find(msg.wire,12);return T__pb_S32.{value=cast,trunc(s32) f.raw,present=!f.done,error=f.error};}\nT_Scalars_set_state :: (msg:T_Scalars,value:s32,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[12];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast,trunc(u64) value";
  #assert cast(string)output[32256..<32768]==";\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],12,0,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_state :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_";
  #assert cast(string)output[32768..<33280]=="root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[12];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],12,0,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.erro";
  #assert cast(string)output[33280..<33792]=="r};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_sf32 :: 13;\nT_Scalars_get_sf32 :: (msg:T_Scalars)->T__pb_S32 {\nif msg.error!=0 {return T__pb_S32.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_S32.{error=checked.error};}\nf:=T__pb_find(msg.wire,13);return T__pb_S32.{value=cast,trunc(s32) f.raw,present=!f.done,error=f.error};}\nT_Scalars_set_sf32 :: (msg:T_Scalars,value:s32,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=";
  #assert cast(string)output[33792..<34304]=="T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[13];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast,trunc(u64) value;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],13,5,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),";
  #assert cast(string)output[34304..<34816]=="false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_sf32 :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[13];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..";
  #assert cast(string)output[34816..<35328]=="],13,5,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_sf64 :: 14;\nT_Scalars_get_sf64 :: (msg:T_Scalars)->T__pb_S64 {\nif msg.error!=0 {return T__pb_S64.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_S64.{err";
  #assert cast(string)output[35328..<35840]=="or=checked.error};}\nf:=T__pb_find(msg.wire,14);return T__pb_S64.{value=cast,trunc(s64) f.raw,present=!f.done,error=f.error};}\nT_Scalars_set_sf64 :: (msg:T_Scalars,value:s64,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[14];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast,trunc(u";
  #assert cast(string)output[35840..<36352]=="64) value;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],14,1,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_sf64 :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wir";
  #assert cast(string)output[36352..<36864]=="e,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[14];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],14,1,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=fi";
  #assert cast(string)output[36864..<37376]=="nal.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_zig32 :: 15;\nT_Scalars_get_zig32 :: (msg:T_Scalars)->T__pb_S32 {\nif msg.error!=0 {return T__pb_S32.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_S32.{error=checked.error};}\nf:=T__pb_find(msg.wire,15);return T__pb_S32.{value=cast(s32) T__pb_sint(f.raw),present=!f.done,error=f.error};}\nT_Scalars_set_zig32 :: (msg:T_Scalars,value:s32,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.e";
  #assert cast(string)output[37376..<37888]=="rror};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[15];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=T__pb_zigzag(cast(s64)value);\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],15,0,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields()";
  #assert cast(string)output[37888..<38400]==",T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_zig32 :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[15];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(m";
  #assert cast(string)output[38400..<38912]=="sg.wire,removed_fields[..],15,0,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_zig64 :: 16;\nT_Scalars_get_zig64 :: (msg:T_Scalars)->T__pb_S64 {\nif msg.error!=0 {return T__pb_S64.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.erro";
  #assert cast(string)output[38912..<39424]=="r!=0 {return T__pb_S64.{error=checked.error};}\nf:=T__pb_find(msg.wire,16);return T__pb_S64.{value=cast(s64) T__pb_sint(f.raw),present=!f.done,error=f.error};}\nT_Scalars_set_zig64 :: (msg:T_Scalars,value:s64,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[16];\ndata:[] u8;txt:=\"\";raw:u64=";
  #assert cast(string)output[39424..<39936]=="0;is_text:=false;\nraw=T__pb_zigzag(cast(s64)value);\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],16,0,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_zig64 :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.";
  #assert cast(string)output[39936..<40448]=="error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[16];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],16,0,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if ";
  #assert cast(string)output[40448..<40960]=="final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_numbers :: 17;\nT_Scalars_Iter_numbers :: struct {wire:[] u8;cursor:int;packed:[] u8;packed_at:int;value:s32;has_value:bool;done:bool;error:u32;}\nT_Scalars_iter_numbers :: (msg:T_Scalars)->T_Scalars_Iter_numbers {if msg.error!=0 {return T_Scalars_Iter_numbers.{error=msg.error,done=true};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T_Scalars_Iter_numbers.{error=checked.error,done=tru";
  #assert cast(string)output[40960..<41472]=="e};}return T_Scalars_next_numbers(T_Scalars_Iter_numbers.{wire=msg.wire});}\nT_Scalars_next_numbers :: (old:T_Scalars_Iter_numbers)->T_Scalars_Iter_numbers {iterator:=old;iterator.has_value=false;if iterator.error!=0 || iterator.done {return iterator;}while true {\nif iterator.packed_at<iterator.packed.count {f:=T__pb_scalar(iterator.packed,iterator.packed_at,0);if f.error!=0 {iterator.error=f.error;iterator.done=true;return iterator;}iterator.packed_at=f.next;iterator.value=cast(s32) T__pb_sint(f.raw);iterat";
  #assert cast(string)output[41472..<41984]=="or.has_value=true;return iterator;}\nf:=T__pb_scan(iterator.wire,iterator.cursor);if f.error!=0 || f.done {iterator.error=f.error;iterator.done=true;return iterator;}iterator.cursor=f.next;if f.number!=17 {continue;}\nif f.wt==2 {iterator.packed=f.payload;iterator.packed_at=0;continue;}\niterator.value=cast(s32) T__pb_sint(f.raw);iterator.has_value=true;return iterator;}return iterator;}\nT_Scalars_append_numbers :: (msg:T_Scalars,value:s32,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.er";
  #assert cast(string)output[41984..<42496]=="ror};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:[] u32;\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=T__pb_zigzag(cast(s64)value);\nresult:=T__pb_rewrite(msg.wire,removed_fields,17,0,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partia";
  #assert cast(string)output[42496..<43008]=="l_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_numbers :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[17];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,rem";
  #assert cast(string)output[43008..<43520]=="oved_fields[..],17,0,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_child :: 18;\nT_Scalars_get_child :: (msg:T_Scalars)->T_Inner_Option {\nif msg.error!=0 {return T_Inner_Option.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error";
  #assert cast(string)output[43520..<44032]=="!=0 {return T_Inner_Option.{error=checked.error};}\nf:=T__pb_find(msg.wire,18);return T_Inner_Option.{value=T_Inner.{wire=f.payload},present=!f.done,error=f.error};}\nT_Scalars_set_child :: (msg:T_Scalars,value:T_Inner,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[18];\ndata:[] u8;txt:=\"";
  #assert cast(string)output[44032..<44544]=="\";raw:u64=0;is_text:=false;\nif value.error!=0 {return T_Scalars.{error=value.error};}child:=T_Inner_decode(value.wire);if child.error!=0 {return T_Scalars.{error=child.error};} data=value.wire;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],18,2,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}retur";
  #assert cast(string)output[44544..<45056]=="n T_Scalars.{wire=result.wire};}\nT_Scalars_clear_child :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[18];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],18,2,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_";
  #assert cast(string)output[45056..<45568]=="Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_children :: 19;\nT_Scalars_Iter_children :: struct {wire:[] u8;cursor:int;packed:[] u8;packed_at:int;value:T_Inner;has_value:bool;done:bool;error:u32;}\nT_Scalars_iter_children :: (msg:T_Scalars)->T_Scalars_Iter_children {if msg.error!=0 {return T_Scalars_Iter_chi";
  #assert cast(string)output[45568..<46080]=="ldren.{error=msg.error,done=true};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T_Scalars_Iter_children.{error=checked.error,done=true};}return T_Scalars_next_children(T_Scalars_Iter_children.{wire=msg.wire});}\nT_Scalars_next_children :: (old:T_Scalars_Iter_children)->T_Scalars_Iter_children {iterator:=old;iterator.has_value=false;if iterator.error!=0 || iterator.done {return iterator;}while true {\nf:=T__pb_scan(iterator.wire,iterator.cursor);if f.error!=0 || f.done {iterator.error=f.erro";
  #assert cast(string)output[46080..<46592]=="r;iterator.done=true;return iterator;}iterator.cursor=f.next;if f.number!=19 {continue;}\niterator.value=T_Inner.{wire=f.payload};iterator.has_value=true;return iterator;}return iterator;}\nT_Scalars_append_children :: (msg:T_Scalars,value:T_Inner,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:[] u";
  #assert cast(string)output[46592..<47104]=="32;\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nif value.error!=0 {return T_Scalars.{error=value.error};}child:=T_Inner_decode(value.wire);if child.error!=0 {return T_Scalars.{error=child.error};} data=value.wire;\nresult:=T__pb_rewrite(msg.wire,removed_fields,19,2,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=fi";
  #assert cast(string)output[47104..<47616]=="nal.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_children :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[19];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],19,2,raw,data,txt,is_text,true,out);\nif result";
  #assert cast(string)output[47616..<48128]==".error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_title :: 20;\nT_Scalars_get_title :: (msg:T_Scalars)->T__pb_String {\nif msg.error!=0 {return T__pb_String.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_String.{error=checked.error};}\nf:=T__pb_find(msg";
  #assert cast(string)output[48128..<48640]==".wire,20);return T__pb_String.{value=cast(string) f.payload,present=!f.done,error=f.error};}\nT_Scalars_set_title :: (msg:T_Scalars,value:string,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [2] u32.[20,21];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\ntxt=value;is_text=true;\nresult:=T__pb_rewr";
  #assert cast(string)output[48640..<49152]=="ite(msg.wire,removed_fields[..],20,2,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_title :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_d";
  #assert cast(string)output[49152..<49664]=="efault_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[20];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],20,2,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars";
  #assert cast(string)output[49664..<50176]==".{wire=result.wire};}\nT_Scalars_FIELD_rank :: 21;\nT_Scalars_get_rank :: (msg:T_Scalars)->T__pb_U64 {\nif msg.error!=0 {return T__pb_U64.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_U64.{error=checked.error};}\nf:=T__pb_find(msg.wire,21);return T__pb_U64.{value=f.raw,present=!f.done,error=f.error};}\nT_Scalars_set_rank :: (msg:T_Scalars,value:u64,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root";
  #assert cast(string)output[50176..<50688]=="_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [2] u32.[20,21];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast(u64) value;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],21,0,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scal";
  #assert cast(string)output[50688..<51200]=="ars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_rank :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[21];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],21,0,raw,data,txt,is_text,true,out);\n";
  #assert cast(string)output[51200..<51712]=="if result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_FIELD_opt :: 22;\nT_Scalars_get_opt :: (msg:T_Scalars)->T__pb_U32 {\nif msg.error!=0 {return T__pb_U32.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T__pb_U32.{error=checked.error};}\nf:=T__pb_find(msg.wir";
  #assert cast(string)output[51712..<52224]=="e,22);return T__pb_U32.{value=cast(u32) f.raw,present=!f.done,error=f.error};}\nT_Scalars_set_opt :: (msg:T_Scalars,value:u32,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[22];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nraw=cast(u64) value;\nresult:=T__pb_rewrite(msg.wire,removed_fiel";
  #assert cast(string)output[52224..<52736]=="ds[..],22,0,raw,data,txt,is_text,false,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_opt :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_parti";
  #assert cast(string)output[52736..<53248]=="al_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[22];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],22,0,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Sca";
  #assert cast(string)output[53248..<53760]=="lars_FIELD_recurse :: 23;\nT_Scalars_get_recurse :: (msg:T_Scalars)->T_Scalars_Option {\nif msg.error!=0 {return T_Scalars_Option.{error=msg.error};}checked:=T_Scalars_decode(msg.wire);if checked.error!=0 {return T_Scalars_Option.{error=checked.error};}\nf:=T__pb_find(msg.wire,23);return T_Scalars_Option.{value=T_Scalars.{wire=f.payload},present=!f.done,error=f.error};}\nT_Scalars_set_recurse :: (msg:T_Scalars,value:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:";
  #assert cast(string)output[53760..<54272]=="=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{error=checked.error};}\nremoved_fields:= [1] u32.[23];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nif value.error!=0 {return T_Scalars.{error=value.error};}child:=T_Scalars_decode(value.wire);if child.error!=0 {return T_Scalars.{error=child.error};} data=value.wire;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],23,2,raw,data,txt,is_text,false,out);\nif result.error!=0 ";
  #assert cast(string)output[54272..<54784]=="{return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\nT_Scalars_clear_recurse :: (msg:T_Scalars,out:[] u8)->T_Scalars {\nif msg.error!=0 {return T_Scalars.{error=msg.error};} checked:=T_Scalars__check(msg.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if checked.error!=0 {return T_Scalars.{er";
  #assert cast(string)output[54784..<55231]=="ror=checked.error};}\nremoved_fields:= [1] u32.[23];\ndata:[] u8;txt:=\"\";raw:u64=0;is_text:=false;\nresult:=T__pb_rewrite(msg.wire,removed_fields[..],23,2,raw,data,txt,is_text,true,out);\nif result.error!=0 {return T_Scalars.{error=result.error};} final:=T_Scalars__check(result.wire,T__pb_root_depth(),T__pb_default_fields(),T__pb_partial_fields(),false);if final.error!=0 {return T_Scalars.{error=final.error};}return T_Scalars.{wire=result.wire};}\n";
  #assert state[0].nt==2 && state[0].ne==1 && state[0].nf==24 && state[0].nv==3;
  #assert types[0].name=="T_Inner" && types[1].name=="T_Scalars";
  #assert fields[1].kind==1 && fields[23].target=="T_Scalars";
}

#test "object count limit is exact" {
  state:[1] D_State;wire:[80002] u8;for i:0..<80002 {wire[i]=8;}
  D_object(state[..],cast(string)wire[0..<80000]);#assert state[0].error=="" && state[0].visited==40000;
  state[0].visited=0;D_object(state[..],cast(string)wire[..]);#assert state[0].error=="descriptor field/work budget";
}
#test "package component and length ceilings are exact" {
  bytes:[4097] u8;for i:0..<4097 {bytes[i]=97;}for i:0..<15 {bytes[(i+1)*257-1]=46;}
  #assert D_package(cast(string)bytes[0..<4096]) && !D_package(cast(string)bytes[..]);
  parts:[65] u8;for i:0..<65 {parts[i]=97;if i%2==1 {parts[i]=46;}}
  #assert D_package(cast(string)parts[0..<63]) && !D_package(cast(string)parts[..]);
}

#forall "every two-byte descriptor UTF8 input" bits:u16 {
  a:=cast,trunc(u8)bits;b:=cast(u8)(bits>>8);bytes:=[2] u8.[a,b];
  expected:=(a<128 && b<128) || (a>=194 && a<=223 && b>=128 && b<=191);
  #assert D_utf8(cast(string)bytes[..])==expected;
}

#test "present uint32 maximum distinguishes zero and rejects malformed options" {
 state:[1]D_State;
 zero:=[6]u8.[66,4,136,244,24,0];o:=D_options(state[..],cast(string)zero[..],8,false);
 #assert state[0].error=="" && D_has_option(o,9) && o.n[9]==0;
 max:=[8]u8.[66,6,136,244,24,255,255,3];o=D_options(state[..],cast(string)max[..],8,false);
 #assert state[0].error=="" && D_has_option(o,9) && o.n[9]==65535;
 full:=[10]u8.[66,8,136,244,24,255,255,255,255,15];o=D_options(state[..],cast(string)full[..],8,false);
 #assert state[0].error=="" && D_has_option(o,9) && o.n[9]==4294967295;
 absent:=[2]u8.[66,0];o=D_options(state[..],cast(string)absent[..],8,false);
 #assert state[0].error=="" && !D_has_option(o,9);
 overflow:=[10]u8.[66,8,136,244,24,128,128,128,128,16];o=D_options(state[..],cast(string)overflow[..],8,false);
 #assert state[0].error!="";state[0].error="";
 duplicate:=[10]u8.[66,8,136,244,24,0,136,244,24,1];o=D_options(state[..],cast(string)duplicate[..],8,false);
 #assert state[0].error!="";state[0].error="";
 wrong:=[7]u8.[66,5,138,244,24,1,0];o=D_options(state[..],cast(string)wrong[..],8,false);
 #assert state[0].error!="";state[0].error="";
 future:=[6]u8.[66,4,144,244,24,1];o=D_options(state[..],cast(string)future[..],8,false);
 #assert state[0].error!="";state[0].error="";
 message:=[6]u8.[58,4,136,244,24,0];o=D_options(state[..],cast(string)message[..],7,true);
 #assert state[0].error!="";
}
#test "uint32 maximum rejects wrong descriptor types" {
 fixture:=[53]u8.[10,51,10,10,116,101,115,116,46,112,114,111,116,111,18,4,116,101,115,116,34,23,10,1,77,18,18,10,4,107,105,110,100,24,1,32,1,40,13,66,4,136,244,24,0,98,6,112,114,111,116,111,51];
 kinds:=[6]u8.[4,5,7,8,9,13];
 for i:0..<kinds.count {
  fixture[38]=kinds[i];types:[8]D_Type;enums:[8]D_Type;fields:[64]D_Field;values:[16]D_Value;groups:[16]string;files:[8]string;deps:[8]string;
  arena:[4096]u8;output:[1]u8;symbols:[256]string;
  state:=[1]D_State.[D_State.{types=types[..],enums=enums[..],fields=fields[..],values=values[..],groups=groups[..],files=files[..],deps=deps[..],arena=arena[..],output=output[..],symbols=symbols[..]}];
  D_parse(state[..],cast(string)fixture[..],"P");
  if kinds[i]==13 {#assert state[0].error=="";}else {#assert state[0].error!="";}
 }
}
#test "generated uint32 maximum checks zero and omits redundant ceiling" {
 arena:[4096]u8;output:[1024]u8;args:[10]string;args[0]="P__pb_";args[6]="0";args[9]="return P__pb_Check.{error=P__pb_";
 for choice:0..<4 {
  state:=[1]D_State.[D_State.{arena=arena[..],output=output[..]}];f:=D_Field.{kind=13};
  if choice>0 {f.opts.bits=cast(u32)512;}
  if choice==2 {f.opts.n[9]=65535;}if choice==3 {f.opts.n[9]=4294967295;}
  G_constraints(state[..],f,args[..]);
  code:=cast(string)output[0..<state[0].written];
  if choice==0 || choice==3 {#assert code=="if f.wt!=0 { return P__pb_Check.{error=P__pb_WIRE}; }\nif f.raw>4294967295 { return P__pb_Check.{error=P__pb_VALUE}; }\n";}
  if choice==1 {#assert code=="if f.wt!=0 { return P__pb_Check.{error=P__pb_WIRE}; }\nif f.raw>4294967295 { return P__pb_Check.{error=P__pb_VALUE}; }\nif f.raw>0 { return P__pb_Check.{error=P__pb_VALUE}; }\n";}
  if choice==2 {#assert code=="if f.wt!=0 { return P__pb_Check.{error=P__pb_WIRE}; }\nif f.raw>4294967295 { return P__pb_Check.{error=P__pb_VALUE}; }\nif f.raw>65535 { return P__pb_Check.{error=P__pb_VALUE}; }\n";}
 }
}
)COAX_IMPORTER";
static constexpr char PROTO_FINGERPRINT_SOURCE[] = R"COAX_FP(// SHA-256 (FIPS 180-4): the compression function, padding, the initial
// hash words and a one-shot hash over a whole string. No host calls,
// allocation or input-sized padding buffer. Additions reduce widened sums
// modulo 2^32 in both checked and native modes. The streaming API in
// sha256.coax and the compiler fingerprint in ../proto/fingerprint.coax both
// load this file.
#authority;
S_Digest :: struct {words:[8] u32;error:u32;}
#test "published SHA256 empty and abc vectors" {
  out:[64] u8;
  #assert S_hex(S_hash(""),out[..]);
  #assert cast(string)out[..]=="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
  #assert S_hex(S_hash("abc"),out[..]);
  #assert cast(string)out[..]=="ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
}
#test "SHA256 length and padding boundaries" {
  #assert S_length(0) && S_length(2305843009213693951) && !S_length(-1) && !S_length(2305843009213693952);
  #assert S_blocks(0)==1 && S_blocks(55)==1 && S_blocks(56)==2 && S_blocks(63)==2 && S_blocks(64)==2;
  #assert S_blocks(119)==2 && S_blocks(120)==3;
  #assert S_seed(-1).error==1 && S_seed(2305843009213693952).error==1 && S_seed(0).error==0;
  #assert S_padded("abc",0,64)==97 && S_padded("abc",2,64)==99 && S_padded("abc",3,64)==128;
  #assert S_padded("abc",4,64)==0 && S_padded("abc",55,64)==0 && S_padded("abc",56,64)==0 && S_padded("abc",63,64)==24;
  bad:=S_Digest.{error=1};out:[65] u8;out[0]=99;
  #assert !S_hex(bad,out[..]) && out[0]==99;
  #assert !S_hex(S_hash(""),out[0..<63]) && out[0]==99;
  #assert S_hex(S_hash("abc"),out[0..<64]) && out[64]==0;
}
// book:begin padding
// The padded message is the input, one 0x80 byte, zero bytes, and the input's
// bit length as a big-endian u64 in the last 8 bytes. It needs one block per
// 64 input bytes plus one, and one more when the last block has fewer than 9
// bytes free (the remainder is 56 to 63).
S_length :: (n: int) -> bool { return n >= 0 && n <= 2305843009213693951; }
S_blocks :: (n: int) -> int {
  blocks := n / 64 + 1;
  if n % 64 >= 56 { blocks += 1; }
  return blocks;
}
S_padding :: (count: int, at: int, total: int) -> u8 {
  if at == count { return 128; }
  if at >= total - 8 { return cast,trunc(u8) ((cast(u64) count * 8) >> ((total - 1 - at) * 8)); }
  return 0;
}
S_padded :: (input: string, at: int, total: int) -> u8 {
  if at < input.count { return input[at]; }
  return S_padding(input.count, at, total);
}
// book:end padding
// book:begin words
// Word arithmetic. Sums widen to u64 and keep the low 32 bits, so they wrap
// the same way in checked and native builds.
S_rotr :: (x: u32, n: int) -> u32 {
  if n == 0 { return x; }
  return (x >> n) | (x << (32 - n));
}
S_sum2 :: (a: u32, b: u32) -> u32 { return cast,trunc(u32) (cast(u64) a + cast(u64) b); }
S_sum4 :: (a: u32, b: u32, c: u32, d: u32) -> u32 { return cast,trunc(u32) (cast(u64) a + cast(u64) b + cast(u64) c + cast(u64) d); }
S_sum5 :: (a: u32, b: u32, c: u32, d: u32, e: u32) -> u32 { return cast,trunc(u32) (cast(u64) a + cast(u64) b + cast(u64) c + cast(u64) d + cast(u64) e); }
// book:end words
// book:begin compress
// One 64-byte block of the padded message, starting at `at`, into the eight
// hash words `h`.
S_compress :: (h: [] u32, input: string, at: int, total: int) {
  k:=[64] u32.[
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2];
  // The message schedule: 16 big-endian words from the block, then 48 more.
  w: [64] u32;
  for i: 0..<16 {
    p := at + i * 4;
    w[i] = (cast(u32) S_padded(input, p, total) << 24) | (cast(u32) S_padded(input, p + 1, total) << 16) | (cast(u32) S_padded(input, p + 2, total) << 8) | cast(u32) S_padded(input, p + 3, total);
  }
  for i: 16..<64 {
    x := w[i - 15]; y := w[i - 2];
    s0 := S_rotr(x, 7) ^ S_rotr(x, 18) ^ (x >> 3);
    s1 := S_rotr(y, 17) ^ S_rotr(y, 19) ^ (y >> 10);
    w[i] = S_sum4(w[i - 16], s0, w[i - 7], s1);
  }
  // 64 rounds over the eight working variables. FIPS 180-4 calls the last
  // one h; here it is z, because h is the hash.
  a := h[0]; b := h[1]; c := h[2]; d := h[3]; e := h[4]; f := h[5]; g := h[6]; z := h[7];
  for i: 0..<64 {
    s1 := S_rotr(e, 6) ^ S_rotr(e, 11) ^ S_rotr(e, 25);
    choose := (e & f) ^ ((e ^ 4294967295) & g);
    t1 := S_sum5(z, s1, choose, k[i], w[i]);
    s0 := S_rotr(a, 2) ^ S_rotr(a, 13) ^ S_rotr(a, 22);
    majority := (a & b) ^ (a & c) ^ (b & c);
    t2 := S_sum2(s0, majority);
    z = g; g = f; f = e; e = S_sum2(d, t1); d = c; c = b; b = a; a = S_sum2(t1, t2);
  }
  h[0] = S_sum2(h[0], a); h[1] = S_sum2(h[1], b); h[2] = S_sum2(h[2], c); h[3] = S_sum2(h[3], d);
  h[4] = S_sum2(h[4], e); h[5] = S_sum2(h[5], f); h[6] = S_sum2(h[6], g); h[7] = S_sum2(h[7], z);
}
// book:end compress
// book:begin hash
// The initial hash words: the first 32 bits of the fractional parts of the
// square roots of the first eight primes.
S_seed :: (count: int) -> S_Digest {
  if !S_length(count) { return S_Digest.{error = 1}; }
  h := [8] u32.[0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
  return S_Digest.{words = h};
}
S_hash :: (input: string) -> S_Digest #export_c {
  seed := S_seed(input.count);
  if seed.error != 0 { return seed; }
  h := seed.words;
  blocks := S_blocks(input.count); total := blocks * 64;
  for i: 0..<blocks { S_compress(h[..], input, i * 64, total); }
  return S_Digest.{words = h};
}
// book:end hash
S_hex :: (digest: S_Digest, out: [] u8) -> bool #export_c {
  if digest.error != 0 || out.count < 64 { return false; }
  digits := "0123456789abcdef";
  for i: 0..<8 { for j: 0..<8 { n := (digest.words[i] >> ((7 - j) * 4)) & 15; out[i * 8 + j] = digits[n]; } }
  return true;
}
#test "binary SHA256 vector 1" {
  input:=[1] u8.[0];out:[64] u8;
  #assert S_hex(S_hash(cast(string)input[..]),out[..]);
  #assert cast(string)out[..]=="6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d";
}
#test "binary SHA256 vector 3" {
  input:=[3] u8.[0,1,2];out:[64] u8;
  #assert S_hex(S_hash(cast(string)input[..]),out[..]);
  #assert cast(string)out[..]=="ae4b3280e56e2faf83f414a6e3dabe9d5fbe18976544c05fed121accb85b53fc";
}
#test "binary SHA256 vector 55" {
  input:=[55] u8.[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54];out:[64] u8;
  #assert S_hex(S_hash(cast(string)input[..]),out[..]);
  #assert cast(string)out[..]=="463eb28e72f82e0a96c0a4cc53690c571281131f672aa229e0d45ae59b598b59";
}
#test "binary SHA256 vector 56" {
  input:=[56] u8.[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55];out:[64] u8;
  #assert S_hex(S_hash(cast(string)input[..]),out[..]);
  #assert cast(string)out[..]=="da2ae4d6b36748f2a318f23e7ab1dfdf45acdc9d049bd80e59de82a60895f562";
}
#test "binary SHA256 vector 57" {
  input:=[57] u8.[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56];out:[64] u8;
  #assert S_hex(S_hash(cast(string)input[..]),out[..]);
  #assert cast(string)out[..]=="2fe741af801cc238602ac0ec6a7b0c3a8a87c7fc7d7f02a3fe03d1c12eac4d8f";
}
#test "binary SHA256 vector 63" {
  input:=[63] u8.[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62];out:[64] u8;
  #assert S_hex(S_hash(cast(string)input[..]),out[..]);
  #assert cast(string)out[..]=="29af2686fd53374a36b0846694cc342177e428d1647515f078784d69cdb9e488";
}
#test "binary SHA256 vector 64" {
  input:=[64] u8.[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63];out:[64] u8;
  #assert S_hex(S_hash(cast(string)input[..]),out[..]);
  #assert cast(string)out[..]=="fdeab9acf3710362bd2658cdc9a29e8f9c757fcf9811603a8c447cd1d9151108";
}
#test "binary SHA256 vector 65" {
  input:=[65] u8.[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64];out:[64] u8;
  #assert S_hex(S_hash(cast(string)input[..]),out[..]);
  #assert cast(string)out[..]=="4bfd2c8b6f1eec7a2afeb48b934ee4b2694182027e6d0fc075074f2fabb31781";
}
#test "binary SHA256 vector 119" {
  input:=[119] u8.[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,85,86,87,88,89,90,91,92,93,94,95,96,97,98,99,100,101,102,103,104,105,106,107,108,109,110,111,112,113,114,115,116,117,118];out:[64] u8;
  #assert S_hex(S_hash(cast(string)input[..]),out[..]);
  #assert cast(string)out[..]=="da18797ed7c3a777f0847f429724a2d8cd5138e6ed2895c3fa1a6d39d18f7ec6";
}
#test "binary SHA256 vector 120" {
  input:=[120] u8.[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,85,86,87,88,89,90,91,92,93,94,95,96,97,98,99,100,101,102,103,104,105,106,107,108,109,110,111,112,113,114,115,116,117,118,119];out:[64] u8;
  #assert S_hex(S_hash(cast(string)input[..]),out[..]);
  #assert cast(string)out[..]=="f52b23db1fbb6ded89ef42a23ce0c8922c45f25c50b568a93bf1c075420bbb7c";
}
#test "binary SHA256 vector 128" {
  input:=[128] u8.[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,85,86,87,88,89,90,91,92,93,94,95,96,97,98,99,100,101,102,103,104,105,106,107,108,109,110,111,112,113,114,115,116,117,118,119,120,121,122,123,124,125,126,127];out:[64] u8;
  #assert S_hex(S_hash(cast(string)input[..]),out[..]);
  #assert cast(string)out[..]=="471fb943aa23c511f6f72f8d1652d9c880cfa392ad80503120547703e56a2be5";
}
#test "binary SHA256 vector 256" {
  input:=[256] u8.[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,85,86,87,88,89,90,91,92,93,94,95,96,97,98,99,100,101,102,103,104,105,106,107,108,109,110,111,112,113,114,115,116,117,118,119,120,121,122,123,124,125,126,127,128,129,130,131,132,133,134,135,136,137,138,139,140,141,142,143,144,145,146,147,148,149,150,151,152,153,154,155,156,157,158,159,160,161,162,163,164,165,166,167,168,169,170,171,172,173,174,175,176,177,178,179,180,181,182,183,184,185,186,187,188,189,190,191,192,193,194,195,196,197,198,199,200,201,202,203,204,205,206,207,208,209,210,211,212,213,214,215,216,217,218,219,220,221,222,223,224,225,226,227,228,229,230,231,232,233,234,235,236,237,238,239,240,241,242,243,244,245,246,247,248,249,250,251,252,253,254,255];out:[64] u8;
  #assert S_hex(S_hash(cast(string)input[..]),out[..]);
  #assert cast(string)out[..]=="40aff2e9d2d8922e47afd4648e6967497158785fbd1da870e7110266bf944880";
}
#test "binary SHA256 vector 1000" {
  input:=[1000] u8.[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,85,86,87,88,89,90,91,92,93,94,95,96,97,98,99,100,101,102,103,104,105,106,107,108,109,110,111,112,113,114,115,116,117,118,119,120,121,122,123,124,125,126,127,128,129,130,131,132,133,134,135,136,137,138,139,140,141,142,143,144,145,146,147,148,149,150,151,152,153,154,155,156,157,158,159,160,161,162,163,164,165,166,167,168,169,170,171,172,173,174,175,176,177,178,179,180,181,182,183,184,185,186,187,188,189,190,191,192,193,194,195,196,197,198,199,200,201,202,203,204,205,206,207,208,209,210,211,212,213,214,215,216,217,218,219,220,221,222,223,224,225,226,227,228,229,230,231,232,233,234,235,236,237,238,239,240,241,242,243,244,245,246,247,248,249,250,251,252,253,254,255,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,85,86,87,88,89,90,91,92,93,94,95,96,97,98,99,100,101,102,103,104,105,106,107,108,109,110,111,112,113,114,115,116,117,118,119,120,121,122,123,124,125,126,127,128,129,130,131,132,133,134,135,136,137,138,139,140,141,142,143,144,145,146,147,148,149,150,151,152,153,154,155,156,157,158,159,160,161,162,163,164,165,166,167,168,169,170,171,172,173,174,175,176,177,178,179,180,181,182,183,184,185,186,187,188,189,190,191,192,193,194,195,196,197,198,199,200,201,202,203,204,205,206,207,208,209,210,211,212,213,214,215,216,217,218,219,220,221,222,223,224,225,226,227,228,229,230,231,232,233,234,235,236,237,238,239,240,241,242,243,244,245,246,247,248,249,250,251,252,253,254,255,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,85,86,87,88,89,90,91,92,93,94,95,96,97,98,99,100,101,102,103,104,105,106,107,108,109,110,111,112,113,114,115,116,117,118,119,120,121,122,123,124,125,126,127,128,129,130,131,132,133,134,135,136,137,138,139,140,141,142,143,144,145,146,147,148,149,150,151,152,153,154,155,156,157,158,159,160,161,162,163,164,165,166,167,168,169,170,171,172,173,174,175,176,177,178,179,180,181,182,183,184,185,186,187,188,189,190,191,192,193,194,195,196,197,198,199,200,201,202,203,204,205,206,207,208,209,210,211,212,213,214,215,216,217,218,219,220,221,222,223,224,225,226,227,228,229,230,231,232,233,234,235,236,237,238,239,240,241,242,243,244,245,246,247,248,249,250,251,252,253,254,255,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,85,86,87,88,89,90,91,92,93,94,95,96,97,98,99,100,101,102,103,104,105,106,107,108,109,110,111,112,113,114,115,116,117,118,119,120,121,122,123,124,125,126,127,128,129,130,131,132,133,134,135,136,137,138,139,140,141,142,143,144,145,146,147,148,149,150,151,152,153,154,155,156,157,158,159,160,161,162,163,164,165,166,167,168,169,170,171,172,173,174,175,176,177,178,179,180,181,182,183,184,185,186,187,188,189,190,191,192,193,194,195,196,197,198,199,200,201,202,203,204,205,206,207,208,209,210,211,212,213,214,215,216,217,218,219,220,221,222,223,224,225,226,227,228,229,230,231];out:[64] u8;
  #assert S_hex(S_hash(cast(string)input[..]),out[..]);
  #assert cast(string)out[..]=="a8af099bf2e878609558dbf69d8f88f4a31040a8cf84b549a0cfa912f12ffc3f";
}
#forall "every rotate distance" n:0..<32 {
  bits:u32=0x91e6a5f3;expected:u64=0;
  for i:0..<32 {if ((cast(u64)bits>>i)&1)!=0 {expected=expected | (cast(u64)1<<((i-n+32)%32));}}
  #assert S_rotr(bits,n)==cast(u32)expected;
}
#test "logical maximum SHA length without allocating the input" {
  count:=2305843009213693951;total:=S_blocks(count)*64;
  #assert total==2305843009213694016;
  #assert S_padding(count,count,total)==128;
  #assert S_padding(count,total-9,total)==0;
  for i:0..<7 {#assert S_padding(count,total-8+i,total)==255;}
  #assert S_padding(count,total-1,total)==248;
}
// Reproducible compiler fingerprints, never admission/signatures. SHA-256
// itself is in ../crypto/sha256-kernel.coax; this file builds the runtime
// boundary and the provenance envelope on it.
#load "../crypto/sha256-kernel.coax";
#authority;
F_Limits :: struct {descriptor:int;runtime:int;source:int;}
F_Result :: struct {written:int;provenance:int;error:string;}
F_Writer :: struct {out:[] u8;at:int;error:string;}
F_capacities :: ()->F_Limits #export_c {return F_Limits.{descriptor=8388608,runtime=1048576,source=8388608};}
F_write :: (w:[] F_Writer,text:string) {
  if text.count>w[0].out.count-w[0].at {w[0].error="fingerprint output budget";return;}
  for c:text {w[0].out[w[0].at]=c;w[0].at+=1;}
}
F_runtime :: (core:string,memory:string,out:[] u8)->F_Result #export_c {
  marker:="// @proto-runtime-checks\n";at:=0;
  while at+marker.count<=core.count {
    if core[at..<at+marker.count]==marker {
      w:=[1] F_Writer.[F_Writer.{out=out}];F_write(w[..],core[0..<at]);F_write(w[..],memory);
      if w[0].error!="" {return F_Result.{error=w[0].error};}return F_Result.{written=w[0].at};
    }at+=1;
  }return F_Result.{error="missing wire runtime check boundary"};
}
F_digest :: (w:[] F_Writer,digest:S_Digest) {
  hex:[64] u8;if !S_hex(digest,hex[..]) {w[0].error="invalid fingerprint input length";return;}
  F_write(w,cast(string)hex[..]);
}
F_hash :: (w:[] F_Writer,input:string) {F_digest(w,S_hash(input));}
F_package :: (raw:string,prefix:string,compiler:string,runtime:string,importer:string,fingerprints:string,body:string,out:[] u8)->F_Result #export_c {
  w:=[1] F_Writer.[F_Writer.{out=out}];
  F_write(w[..],"// coax-proto-v4 generator=coax0-coax-v1 compiler=");
  for c:compiler {one:=[1] u8.[c];if c==32 {one[0]=45;}F_write(w[..],cast(string)one[..]);}
  F_write(w[..]," importer_sha256=");F_hash(w[..],importer);
  F_write(w[..]," fingerprint_sha256=");F_hash(w[..],fingerprints);
  F_write(w[..]," descriptor_sha256=");F_hash(w[..],raw);
  F_write(w[..]," runtime_sha256=");F_hash(w[..],runtime);F_write(w[..]," prefix=");F_write(w[..],prefix);
  provenance:=w[0].at;F_write(w[..],"\n");F_write(w[..],body);
  if w[0].error!="" {return F_Result.{error=w[0].error};}
  return F_Result.{written=w[0].at,provenance=provenance};
}

#test "writer, runtime boundary and capacity policy" {
  c:=F_capacities();#assert c.descriptor==8388608 && c.runtime==1048576 && c.source==8388608;
  #assert S_seed(-1).error==1 && S_seed(2305843009213693952).error==1 && S_seed(0).error==0;
  out:[128] u8;writer:=[1] F_Writer.[F_Writer.{out=out[0..<3]}];
  F_write(writer[..],"abc");#assert writer[0].error=="" && writer[0].at==3 && cast(string)out[0..<3]=="abc";
  F_write(writer[..],"");#assert writer[0].error=="" && writer[0].at==3;
  F_write(writer[..],"x");#assert writer[0].error=="fingerprint output budget" && writer[0].at==3;
  writer[0]=F_Writer.{out=out[..]};F_digest(writer[..],S_Digest.{error=1});
  #assert writer[0].error=="invalid fingerprint input length" && writer[0].at==0;
  r:=F_runtime("core\n// @proto-runtime-checks\n#authority;","memory",out[..]);
  #assert r.error=="" && r.written==11 && cast(string)out[0..<r.written]=="core\nmemory";
  r=F_runtime("// @proto-runtime-checks\n","",out[0..<0]);#assert r.error=="" && r.written==0;
  r=F_runtime("missing","",out[..]);#assert r.error=="missing wire runtime check boundary" && r.written==0;
  r=F_runtime("x// @proto-runtime-checks\n","yz",out[0..<2]);#assert r.error=="fingerprint output budget" && r.written==0;
}
#test "complete fingerprint envelope is exact and bounded" {
  out:[1024] u8;
  r:=F_package("desc","P","coax0 1.2.3 debug","runtime","parser","sha","BODY",out[..]);
  #assert r.error=="" && r.provenance==404 && r.written==409;
  #assert cast(string)out[0..<r.written]=="// coax-proto-v4 generator=coax0-coax-v1 compiler=coax0-1.2.3-debug importer_sha256=b17d45121150928f2146af49e195eff1eef5d67325be273a733fb74acadaa342 fingerprint_sha256=d600474b1b8e50d3633c91c0cf1efc454b79c9624a43fd7de441ee71745726ab descriptor_sha256=97864e878fe129a3d4c35681c3ad4b12743f04f7cd705643f2fa1142dfede601 runtime_sha256=d92c6a81b2ff50096bcda80885427d1f59a25b5f483f7055523504925d16ab23 prefix=P\nBODY";
  r=F_package("desc","P","coax0 1.2.3 debug","runtime","parser","sha","BODY",out[0..<409]);
  #assert r.error=="" && r.written==409;
  r=F_package("desc","P","coax0 1.2.3 debug","runtime","parser","sha","BODY",out[0..<408]);
  #assert r.error=="fingerprint output budget" && r.written==0 && r.provenance==0;
}
)COAX_FP";
