# 193. Compress activations exactly

> **Status: partly implemented.** A lossless codec for chapter 186's
> boundary messages runs on the CPU (profile boundary-codec-v1): a framed
> message with a version byte and a codec byte, a raw codec every receiver
> accepts, a byte-plane codec with a static rANS coder, and the same after
> an XOR with the row above. Every frame decodes to the sender's bits, and
> the receiver refuses each kind of bad frame with its own reason. On
> Ministral 3 3B's real layer-0 and layer-1 boundaries the planes codec
> saves 16% of the bytes, and on chapter 182's link model it costs more
> CPU time than it saves on any link faster than about 0.17 Gbit/s. Still
> to build: frames over chapter 188's link with measured end-to-end
> latency, negotiation carried in a handshake, the codec inside
> `stg_pipeline`, and lossy formats as their own profile.

A stage boundary sends one row of `width` f32 values for each token.
For Ministral 3 3B that is 12,288 bytes of values and a 36-byte header,
12,324 bytes a token ([chapter 186](ch186-split-a-model-into-stages.md)).
This chapter asks whether those bytes can be made smaller without changing
a single bit, and whether doing so is worth the CPU time.

The answer has two parts. The bytes do shrink, by about 16%, because the
exponents of activations are predictable. The rest of each value, its sign
and 23 mantissa bits, is close to random, and nothing lossless removes
randomness. And on the links chapter 182 models, the time the coder spends
is larger than the time the saved bytes would have spent on the wire.

The codec is in an `#authority` file,
[`boundary-codec.coax`](../lib/boundary-codec.coax), because it admits and
refuses frames.

```shell
coax/bin/coax run psionic/examples/boundary-codec.coax
coax/bin/coax run psionic/examples/boundary-codec-refuse.coax
psionic/check.sh --only boundary-codec
COAX_CC=gcc psionic/tests/check_boundary_codec_slices.sh ~/models/ministral-3-3b-instruct-2512-bf16-b6d637be
```

## The frame

A frame carries one chapter 186 message. The header's nine words travel as
they are, so the receiver can run chapter 186's checks (model, layer,
position, width, checksum) after it decodes the values.

```text
byte  0        version, 1
byte  1        codec: 0 raw, 1 planes, 2 delta
bytes 2..37    chapter 186's 9 header words, little-endian
bytes 38..41   P, the payload's length in bytes
bytes 42..     the payload, P bytes
last 4 bytes   FNV-1a of everything before them
```

A raw payload is the values' bits, 4 bytes each. A raw frame is 10 bytes
longer than chapter 186's message: the two leading bytes, the length and
the checksum, less the 36 header bytes that both carry.

<!-- source: psionic/lib/boundary-codec.coax types -->
```jai
BcCodec :: enum u8 {raw;planes;delta;}
BcRefusal :: enum u8 {none;partial;version;codec;declined;geometry;truncated;excess;checksum;room;size;plane;table;stream;}
// A frame the sender wrote: its bytes (0 when the message was refused),
// the codec it used, and the bytes each plane took (0 when sent raw).
BcFrame :: struct {bytes:int;codec:BcCodec;plane:[4]int;}
// What the receiver found: a refusal, or the codec and the words written.
BcDecoded :: struct {refusal:BcRefusal;codec:BcCodec;words:int;}

BC_VERSION :: 1;
BC_CODECS :: 3;
BC_WORDS :: 9;          // chapter 186's header words
BC_HEAD :: 42;          // version, codec, 36 header bytes, the payload's length
BC_TAIL :: 4;           // the checksum
BC_ROWS_MAX :: 64;
BC_WIDTH_MAX :: 8192;
BC_SCALE :: 4096;       // frequencies sum to 2^12
BC_LOW :: 8388608;      // the coder's state stays in [2^23, 2^31)
BC_EMIT :: 19;          // emit a byte while state >> 19 >= frequency
```

## Byte planes

An f32 is 32 bits: a sign, 8 exponent bits and 23 mantissa bits. The
planes codec splits every value into four bytes and codes each position of
those bytes as its own stream, a *plane*:

- Plane 0: the exponent.
- Plane 1: the sign, then the top 7 mantissa bits.
- Plane 2: mantissa bits 8 to 15.
- Plane 3: mantissa bits 0 to 7.

The exponent gets its own plane because it is the part with structure.
Activations sit within a few powers of two of each other, so a few
exponent values cover most of a row. The other planes are close to
uniform. Putting the sign next to mantissa bits costs nothing, since both
are near random.

<!-- source: psionic/lib/boundary-codec.coax planes -->
```jai
// Plane 0 is the exponent, plane 1 the sign and the top 7 mantissa bits,
// planes 2 and 3 the other two mantissa bytes. bc_place puts a symbol back.
bc_symbol :: (w:int,p:int)->int {
    if p==0 {return (w>>23)&255;}
    if p==1 {return ((w>>24)&128)|((w>>16)&127);}
    return (w>>(24-8*p))&255;
}
bc_place :: (s:int,p:int)->int {
    if p==0 {return s<<23;}
    if p==1 {return ((s&128)<<24)|((s&127)<<16);}
    return s<<(24-8*p);
}
// Value i of the payload, XOR-ed with the one a row above for delta.
bc_value :: (pay:[]u32,i:int,width:int,delta:bool)->int {
    w:=cast(int)pay[i];
    if delta && i>=width {w=w^cast(int)pay[i-width];}
    return w;
}
```

The delta codec uses the same planes on `value XOR value one row above`.
If two positions had similar activations, their XOR would have many zero
bits. The measurements below show that they don't.

## Coding a plane

Each plane is coded with rANS, a range coder that keeps its state in one
integer. It needs a frequency table that sums to 4,096 (12 bits):

1. Count each symbol (byte value) in the plane. Say k symbols occur.
2. Give each present symbol 1 slot plus its share of the other 4,096 − k
   slots, rounded down: `1 + count × (4096 − k) / n`.
3. Give the slots left over to the most frequent symbol (the first one on a
   tie).

Every present symbol gets at least one slot, so every symbol can be coded.
Symbol s owns the slots `start[s] ..< start[s] + freq[s]`, where `start`
adds up the frequencies of the smaller symbols.

The encoder walks the values from last to first. Its state x starts at
2²³. For each symbol of frequency f it first writes x's low byte and shifts
x down by 8 while `x >> 19 >= f`. Then it sets
`x = (x / f) × 4096 + x mod f + start`. This keeps x in [2²³, 2³¹). At the
end the encoder writes x in 4 bytes in front of the bytes it wrote, so the
decoder reads from front to back.

The decoder reverses each step. It takes `slot = x mod 4096`, finds the
symbol s that owns that slot, sets `x = freq[s] × (x >> 12) + slot −
start[s]`, and reads bytes into x's low end while x < 2²³. A correct stream
ends with x back at 2²³ and every byte read.

A coded plane is: mode 1, a 32-byte map with one bit per symbol present,
2 bytes of frequency for each present symbol, the stream's length in 4
bytes, then the stream. A plain plane is mode 0 and one byte a value. The
encoder writes whichever is smaller, and plain on a tie. If all four planes
come out plain, the frame is sent raw.

### By hand: 1.0, −2.0, 0.5, 1.0

The four values have bits `3f800000`, `c0000000`, `3f000000` and
`3f800000`, so plane 0 (the exponent) holds 127, 128, 126, 127. Three
symbols occur, so k = 3 and 4,093 slots are shared:

```text
127: count 2, 1 + 2 x 4093 / 4 = 1 + 2046 = 2047
126: count 1, 1 + 4093 / 4     = 1 + 1023 = 1024
128: count 1,                               1024
sum 4095; the slot left goes to 127:        2048
starts: 126 at 0, 127 at 1024, 128 at 3072
```

The encoder takes the values last first. With these frequencies `x >> 19`
is at most 512, never at least f, so no byte is written:

```text
start  x = 8,388,608 (2^23)
127    x = (8388608 / 2048) x 4096 + 0 + 1024           = 16,778,240
126    x = (16778240 / 1024) x 4096 + 0 + 0            = 67,112,960
128    x = (67112960 / 1024) x 4096 + 0 + 3072         = 268,454,912
127    x = (268454912 / 2048) x 4096 + 1024 + 1024     = 536,909,824
```

The stream is that state in 4 bytes. It grew from 2²³ to about 2²⁹: 6
bits, which is the plane's entropy (1.5 bits a value, 4 values). To decode,
`536909824 mod 4096 = 2048` is in 127's slots (1024 to 3071), and
`2048 × (536909824 >> 12) + 2048 − 1024 = 268,454,912`, the state before
the encoder's last step. Three more steps give 128, 126, 127 and return x to
2²³.

The coded plane would take 1 + 32 + 3 × 2 + 4 + 4 = 47 bytes against 5
plain, so it is stored plain. All four planes are, so the frame is sent
raw: 42 + 16 + 4 = 62 bytes for a 52-byte chapter 186 message. A plane pays
for its 37 bytes of table and length only when it holds hundreds of
values. A Ministral row holds 3,072.

<!-- source: psionic/lib/boundary-codec.coax encode -->
```jai
// The frequency table of plane p: every present symbol gets 1 slot, then
// its share of the other 4,096 - k (k symbols present), rounded down; the
// slots left over go to the most frequent symbol, the first on a tie.
BcTable :: struct {k:int;freq:[256]int;}
bc_table :: (pay:[]u32,width:int,delta:bool,p:int)->BcTable {
    n:=pay.count;count:[256]int;t:BcTable;
    for i:0..<n {count[bc_symbol(bc_value(pay,i,width,delta),p)]+=1;}
    for c:count {if c>0 {t.k+=1;}}
    sum:=0;top:=0;
    for s:0..<256 {
        if count[s]>0 {t.freq[s]=1+count[s]*(BC_SCALE-t.k)/n;sum+=t.freq[s];}
        if count[s]>count[top] {top=s;}
    }
    t.freq[top]+=BC_SCALE-sum;
    return t;
}
// rANS over plane p, last value first. The state x starts at 2^23; before
// a symbol of frequency f it gives up low bytes while x >> 19 >= f, then
// becomes (x / f) * 4096 + x % f + start. The bytes go to the end of work,
// backwards, and the final state goes in front of them, so the decoder
// reads forwards. Returns where the stream starts in work.
bc_stream :: (pay:[]u32,width:int,delta:bool,p:int,freq:[]int,work:[]u8)->int {
    n:=pay.count;start:[256]int;
    for s:1..<256 {start[s]=start[s-1]+freq[s-1];}
    x:=BC_LOW;w:=work.count;
    for j:0..<n {
        s:=bc_symbol(bc_value(pay,n-1-j,width,delta),p);f:=freq[s];
        while x>>BC_EMIT>=f {w-=1;work[w]=cast(u8)(x&255);x=x>>8;}
        x=(x/f)*BC_SCALE+x%f+start[s];
    }
    w-=4;bc_put32(work,w,x);
    return w;
}
// Write plane p of the payload at out[at..]; return the bytes it took.
// Coded: mode 1, a 32-byte map of the symbols present, each one's
// frequency in 2 bytes, the stream's length in 4, then the stream. Plain:
// mode 0 and one byte a value. Coded only when that is smaller.
bc_plane :: (pay:[]u32,width:int,delta:bool,p:int,out:[]u8,at:int,work:[]u8)->int {
    n:=pay.count;t:=bc_table(pay,width,delta,p);w:=bc_stream(pay,width,delta,p,t.freq[..],work);
    stream:=work.count-w;coded:=1+32+2*t.k+4+stream;
    if coded>=1+n {
        out[at]=0;
        for i:0..<n {out[at+1+i]=cast(u8)bc_symbol(bc_value(pay,i,width,delta),p);}
        return 1+n;
    }
    out[at]=1;e:=at+33;
    for b:0..<32 {
        v:=0;for j:0..<8 {if t.freq[8*b+j]>0 {v|=1<<j;}}
        out[at+1+b]=cast(u8)v;
    }
    for s:0..<256 {
        if t.freq[s]>0 {out[e]=cast(u8)(t.freq[s]&255);out[e+1]=cast(u8)(t.freq[s]>>8);e+=2;}
    }
    bc_put32(out,e,stream);
    for b:0..<stream {out[e+4+b]=work[w+b];}
    return coded;
}
// Frame chapter 186's message msg (9 header words, then rows x width
// values) with `codec`. A coded frame whose planes all went plain is sent
// raw instead.
bc_encode :: (codec:BcCodec,msg:[]u32,out:[]u8,work:[]u8)->BcFrame #export_c {
    f:BcFrame;n:=msg.count-BC_WORDS;
    if n<1 || out.count<bc_bound(n) || work.count<bc_work(n) {return f;}
    rows:=cast(int)msg[5];width:=cast(int)msg[6];
    if !bc_geometry(rows,width) || rows*width!=n {return f;}
    pay:=msg[BC_WORDS..<BC_WORDS+n];at:=BC_HEAD;coded:=false;f.codec=codec;
    if codec!=BcCodec.raw {
        for p:0..<4 {b:=bc_plane(pay,width,codec==BcCodec.delta,p,out,at,work);coded=coded || out[at]==1;f.plane[p]=b;at+=b;}
    }
    if !coded {
        f.codec=BcCodec.raw;at=BC_HEAD;
        for p:0..<4 {f.plane[p]=0;}
        for i:0..<n {bc_put32(out,at,cast(int)pay[i]);at+=4;}
    }
    out[0]=BC_VERSION;out[1]=cast(u8)f.codec;bc_put32(out,38,at-BC_HEAD);
    for k:0..<BC_WORDS {bc_put32(out,2+4*k,cast(int)msg[k]);}
    bc_put32(out,at,bc_checksum(out[0..<at]));
    f.bytes=at+BC_TAIL;
    return f;
}
// The sender's choice: the codec it wants if the receiver accepts it (bit
// c of accept), raw otherwise. Every receiver accepts raw.
bc_choose :: (accept:int,want:BcCodec)->BcCodec #export_c {
    if (accept>>cast(int)want)&1==1 {return want;}
    return BcCodec.raw;
}
```

## Decoding and refusals

The receiver checks the frame in a fixed order and stops at the first
failure:

1. At least 46 bytes (`partial`).
2. Version 1 (`version`).
3. A known codec (`codec`) that it accepts (`declined`). Raw is always
   accepted.
4. 1 to 64 rows of 1 to 8,192 values (`geometry`).
5. Exactly the payload the frame announces (`truncated`, `excess`).
6. The checksum (`checksum`).
7. Room for the message (`room`).
8. The payload itself: the bytes the planes use add up to P (`size`), each
   mode is 0 or 1 (`plane`), each table sums to 4,096 (`table`), and each
   stream starts at or above 2²³, ends at 2²³ and uses every byte
   (`stream`).

A damaged frame almost always fails at the checksum. The checks in step 8
catch a sender that wrote a wrong frame and sealed it, and they keep a
hostile frame from reading outside its payload. A read past the payload's
end gives 0. The decoder's own loops can't read further than that: each
renormalization starts from a state of at least 2¹¹, so it reads at most 2
bytes.

<!-- source: psionic/lib/boundary-codec.coax decode -->
```jai
BcAt :: struct {refusal:BcRefusal;at:int;}
// Read plane p from pl[at0..] into out (OR-ed in); return where it ended.
bc_unplane :: (pl:[]u8,at0:int,p:int,out:[]u32)->BcAt {
    n:=out.count;at:=at0+1;mode:=bc_byte(pl,at0);
    if mode==0 {
        for i:0..<n {out[i]|=cast(u32)bc_place(bc_byte(pl,at+i),p);}
        return BcAt.{at=at+n};
    }
    if mode!=1 {return BcAt.{refusal=BcRefusal.plane};}
    // The table: the present symbols' frequencies, summing to 4,096.
    freq:[256]int;start:[256]int;e:=at+32;sum:=0;
    for s:0..<256 {
        if (bc_byte(pl,at+s/8)>>(s%8))&1==1 {freq[s]=bc_byte(pl,e)|(bc_byte(pl,e+1)<<8);e+=2;sum+=freq[s];}
    }
    if sum!=BC_SCALE {return BcAt.{refusal=BcRefusal.table};}
    // Each slot names its symbol; the slots symbols 1..255 do not claim
    // are symbol 0's.
    slot:[4096]u8;
    for s:1..<256 {start[s]=start[s-1]+freq[s-1];for j:start[s]..<start[s]+freq[s] {slot[j]=cast(u8)s;}}
    // The stream: a state of at least 2^23, then the bytes it reads. It
    // must end in the state the encoder started from, with every byte read.
    S:=bc_get32(pl,e);at=e+4;
    if at+S>pl.count {return BcAt.{refusal=BcRefusal.size};}
    st:=pl[at..<at+S];x:=bc_get32(st,0);r:=4;
    if x<BC_LOW {return BcAt.{refusal=BcRefusal.stream};}
    for i:0..<n {
        j:=x&(BC_SCALE-1);s:=cast(int)slot[j];x=freq[s]*(x>>12)+j-start[s];
        while x<BC_LOW {x=(x<<8)|bc_byte(st,r);r+=1;}
        out[i]|=cast(u32)bc_place(s,p);
    }
    if x!=BC_LOW || r!=S {return BcAt.{refusal=BcRefusal.stream};}
    return BcAt.{at=at+S};
}
// The receiver's checks, in order: a whole header, the version, a known
// codec it accepts (raw always), rows and width in range, exactly the
// payload the frame announces, the checksum, room, then the payload
// itself. out receives chapter 186's message; on a refusal its contents
// are not defined.
bc_decode :: (accept:int,frame:[]u8,out:[]u32)->BcDecoded #export_c {
    d:BcDecoded;
    if frame.count<BC_HEAD+BC_TAIL {d.refusal=BcRefusal.partial;return d;}
    if frame[0]!=BC_VERSION {d.refusal=BcRefusal.version;return d;}
    c:=cast(int)frame[1];
    if c>=BC_CODECS {d.refusal=BcRefusal.codec;return d;}
    d.codec=cast(BcCodec)c;
    if bc_choose(accept,d.codec)!=d.codec {d.refusal=BcRefusal.declined;return d;}
    rows:=bc_get32(frame,22);width:=bc_get32(frame,26);
    if !bc_geometry(rows,width) {d.refusal=BcRefusal.geometry;return d;}
    P:=bc_get32(frame,38);end:=BC_HEAD+P;
    if frame.count<end+BC_TAIL {d.refusal=BcRefusal.truncated;return d;}
    if frame.count>end+BC_TAIL {d.refusal=BcRefusal.excess;return d;}
    if bc_get32(frame,end)!=bc_checksum(frame[0..<end]) {d.refusal=BcRefusal.checksum;return d;}
    n:=rows*width;
    if out.count<BC_WORDS+n {d.refusal=BcRefusal.room;return d;}
    pay:=out[BC_WORDS..<BC_WORDS+n];pl:=frame[BC_HEAD..<end];
    if c==0 {
        if P!=4*n {d.refusal=BcRefusal.size;return d;}
        for i:0..<n {pay[i]=cast(u32)bc_get32(pl,4*i);}
    } else {
        for i:0..<n {pay[i]=0;}
        at:=0;
        for p:0..<4 {r:=bc_unplane(pl,at,p,pay);if r.refusal!=BcRefusal.none {d.refusal=r.refusal;return d;}at=r.at;}
        if at!=P {d.refusal=BcRefusal.size;return d;}
        if c==2 {for i:width..<n {pay[i]=pay[i]^pay[i-width];}}
    }
    for k:0..<BC_WORDS {out[k]=cast(u32)bc_get32(frame,2+4*k);}
    d.words=BC_WORDS+n;
    return d;
}
```

A sender chooses with `bc_choose`. The receiver lists the codecs it accepts
as bits. If the codec the sender wants is among them, it uses that codec;
otherwise it uses raw. Two peers with no other codec in common still talk.

## The example

The example, [`boundary-codec.coax`](../examples/boundary-codec.coax),
repeats the hand-worked plane. It then packs three real rows of
Ministral 3 3B's layer-0 boundary as chapter 186 messages with `stg_pack`
(the first 256 values of positions 0, 1 and 2, recorded by the slice
below). It frames each message with each codec, decodes it, receives it
with chapter 186's `stg_receive`, and compares every value's bits.

<!-- source: psionic/examples/boundary-codec.coax hand-example -->
```jai
main :: () {
    // The exponents of 1.0, -2.0, 0.5 and 1.0: 127, 128, 126, 127.
    m:=bcx_message(1,4);m[9]=0x3f800000;m[10]=0xc0000000;m[11]=0x3f000000;m[12]=0x3f800000;
    t:=bc_table(m[9..<13],4,false,0);
    print("exponent plane: % symbols, frequencies 126: %, 127: %, 128: %\n",t.k,t.freq[126],t.freq[127],t.freq[128]);
    work:[16]u8;w:=bc_stream(m[9..<13],4,false,0,t.freq[..],work[..]);
    print("rANS: % bytes, final state %\n",16-w,bc_get32(work[..],w));
    x:=bcx_frame(BcCodec.planes,m[0..<13]);
    print("frame: % bytes, codec %; chapter 186's message: % bytes\n",x.f.bytes,bc_codec_name(x.f.codec),4*13);
```

<!-- source: psionic/examples/boundary-codec.coax real-example -->
```jai
    // Rows 0, 1 and 2 after layer 0, the first 256 values of each.
    rows:[768]f32;for i:0..<768 {rows[i]=f32_from_bits(BCX_REAL[i]);}
    want:=StgExpect.{model=MINISTRAL,layer=1,start=0,width=256};
    for r:1..<4 {
        msg:[9+768]u32;n:=stg_pack(MINISTRAL,1,0,256,rows[0..<256*r],msg[..]);
        print("% rows: chapter 186's message % bytes\n",r,4*n);
        for c:0..<3 {
            out:[BCX_FRAME]u8;scratch:[2*768+8]u8;
            f:=bc_encode(cast(BcCodec)c,msg[0..<n],out[..],scratch[..]);
            got:[9+768]u32;d:=bc_decode(7,out[0..<f.bytes],got[..]);
            back:[768]f32;s:=stg_receive(want,got[0..<d.words],back[..]);
            same:=0;for i:0..<256*r {if f32_bits(back[i])==f32_bits(rows[i]) {same+=1;}}
            print("  %: % bytes (% per mille of raw), planes %/%/%/%; %, %, % of % values equal bit for bit\n",
                bc_codec_name(f.codec),f.bytes,1000*f.bytes/(4*n),f.plane[0],f.plane[1],f.plane[2],f.plane[3],
                bc_refusal_name(d.refusal),stg_refusal_name(s.refusal),same,256*r);
        }
    }
    print("a receiver that accepts raw and planes, a sender that wants delta: it sends %\n",bc_codec_name(bc_choose(3,BcCodec.delta)));
    print("a receiver that accepts all three, a sender that wants planes: it sends %\n",bc_codec_name(bc_choose(7,BcCodec.planes)));
}
```

```text
exponent plane: 3 symbols, frequencies 126: 1024, 127: 2048, 128: 1024
rANS: 4 bytes, final state 536909824
frame: 62 bytes, codec raw; chapter 186's message: 52 bytes
1 rows: chapter 186's message 1060 bytes
  raw: 1070 bytes (1009 per mille of raw), planes 0/0/0/0; none, none, 256 of 256 values equal bit for bit
  planes: 988 bytes (932 per mille of raw), planes 171/257/257/257; none, none, 256 of 256 values equal bit for bit
  delta: 988 bytes (932 per mille of raw), planes 171/257/257/257; none, none, 256 of 256 values equal bit for bit
2 rows: chapter 186's message 2084 bytes
  raw: 2094 bytes (1004 per mille of raw), planes 0/0/0/0; none, none, 512 of 512 values equal bit for bit
  planes: 1871 bytes (897 per mille of raw), planes 286/513/513/513; none, none, 512 of 512 values equal bit for bit
  delta: 1970 bytes (945 per mille of raw), planes 385/513/513/513; none, none, 512 of 512 values equal bit for bit
3 rows: chapter 186's message 3108 bytes
  raw: 3118 bytes (1003 per mille of raw), planes 0/0/0/0; none, none, 768 of 768 values equal bit for bit
  planes: 2735 bytes (879 per mille of raw), planes 382/769/769/769; none, none, 768 of 768 values equal bit for bit
  delta: 2881 bytes (926 per mille of raw), planes 528/769/769/769; none, none, 768 of 768 values equal bit for bit
a receiver that accepts raw and planes, a sender that wants delta: it sends raw
a receiver that accepts all three, a sender that wants planes: it sends planes
```

Only plane 0 is ever coded. The three other planes take 1 + n bytes, which
means they are stored plain. With one row there is no row above, so delta
equals planes. With more rows, delta's exponent plane is larger than
planes': XOR-ing two unrelated tokens' values mixes their exponents.

## Refusals

Each frame in
[`boundary-codec-refuse.coax`](../examples/boundary-codec-refuse.coax) is
wrong in one way. The last five are sealed again after
the change, so only the payload's own checks can see it. The last line
shows a frame around another model's message: the codec accepts it, and
chapter 186's receiver refuses it.

<!-- source: psionic/examples/boundary-codec-refuse.coax refuse -->
```jai
main :: () {
    // 64 values 1.0 + k/128, framed by planes: 240 bytes.
    m:=bcx_ramp();x:=bcx_frame(BcCodec.planes,m[..]);f:=x.bytes;n:=x.f.bytes;got:[73]u32;
    say("the frame as sent",bc_decode(7,f[0..<n],got[..]).refusal);
    say("the first 45 bytes",bc_decode(7,f[0..<45],got[..]).refusal);
    c:=f;c[0]=2;say("version 2",bc_decode(7,c[0..<n],got[..]).refusal);
    c=f;c[1]=3;say("codec 3",bc_decode(7,c[0..<n],got[..]).refusal);
    say("a receiver that accepts raw and delta",bc_decode(5,f[0..<n],got[..]).refusal);
    c=f;bc_put32(c[..],22,65);say("65 rows",bc_decode(7,c[0..<n],got[..]).refusal);
    say("the last byte lost",bc_decode(7,f[0..<n-1],got[..]).refusal);
    say("one byte too many",bc_decode(7,f[0..<n+1],got[..]).refusal);
    c=f;c[BC_HEAD+50]^=1;say("one bit flipped in the payload",bc_decode(7,c[0..<n],got[..]).refusal);
    say("room for 72 words",bc_decode(7,f[0..<n],got[0..<72]).refusal);
    // Sealed again after the change, so only the payload's own checks see it.
    say("resealed, plane 0's mode 2",bcx_reseal(x,BC_HEAD,2,got[..]));
    say("resealed, a frequency of 3840",bcx_reseal(x,BC_HEAD+34,0x0f,got[..]));
    say("resealed, a state of 2^23 + 1",bcx_reseal(x,BC_HEAD+39,1,got[..]));
    say("resealed, a 5-byte stream",bcx_reseal(x,BC_HEAD+35,5,got[..]));
    say("resealed, the payload a byte short",bcx_cut(x,1,got[..]));
    // A whole frame around a message for another model: the codec passes
    // it, and chapter 186's receiver refuses it.
    rows:[64]f32;for k:0..<64 {rows[k]=f32_from_bits(m[9+k]);}msg:[73]u32;
    w:=stg_pack(cast(u64)0x0186,1,0,64,rows[..],msg[..]);out:[BCX_FRAME]u8;scratch:[136]u8;
    g:=bc_encode(BcCodec.planes,msg[0..<w],out[..],scratch[..]);d:=bc_decode(7,out[0..<g.bytes],got[..]);back:[64]f32;
    s:=stg_receive(StgExpect.{model=cast(u64)0x722430146632596c,layer=1,start=0,width=64},got[0..<d.words],back[..]);
    print("another model's rows: codec %, boundary %\n",bc_refusal_name(d.refusal),stg_refusal_name(s.refusal));
}
```

```text
the frame as sent: none
the first 45 bytes: partial
version 2: version
codec 3: codec
a receiver that accepts raw and delta: declined
65 rows: geometry
the last byte lost: truncated
one byte too many: excess
one bit flipped in the payload: checksum
room for 72 words: room
resealed, plane 0's mode 2: plane
resealed, a frequency of 3840: table
resealed, a state of 2^23 + 1: stream
resealed, a 5-byte stream: stream
resealed, the payload a byte short: size
another model's rows: codec none, boundary model
```

## The original weights

[`check_boundary_codec_slices.sh`](../tests/check_boundary_codec_slices.sh)
runs Ministral 3 3B's embedding and first two layers on the original
weights over chapter 81's 10-token prompt
([`boundary_codec_slices.c`](../tests/boundary_codec_slices.c) maps the
file; the arithmetic and every check are Coax, through
[`boundary-codec-slices-runtime.coax`](../tests/boundary-codec-slices-runtime.coax)).
It packs the residual rows after layer 0 and after layer 1 as chapter 186
messages, first all 10 rows in one message (a prefill), then one row a
message (a decode step). Each message is framed by each codec 20 times and
decoded 20 times. Every decoded message must equal the packed one word for
word, and `stg_receive` must give back every row bit for bit. The program
stops at the first difference.

For a decode step it adds the link time of chapter 182's model (the
formula of `dg_transfer_ns`: 50 µs of latency, then the bits at the link's
rate, in `bc_slice_link_ns`) to the measured encode and decode time. Both codecs pay CPU time, since a raw frame also computes its
checksum. On this machine (Linux, gcc -O2, one CPU thread):

```text
ministral-manifest.coax matches the pinned checkpoint's header
10 tokens: [1, 3, 7493, 1395, 1278, 8961, 1307, 5498, 1063, 4]
prompt: 10 tokens; rows of 3072 f32 after layer 0 and after layer 1
boundary after layer 0, 10 rows in one message of 122916 bytes:
  raw      122926 bytes, ratio 1.000, planes 0/0/0/0 bytes, encode   155.5 us (  790 MB/s), decode   132.4 us (  928 MB/s), exact
  planes   103238 bytes, ratio 1.191, planes 11029/30721/30721/30721 bytes, encode   943.1 us (  130 MB/s), decode   289.4 us (  425 MB/s), exact
  delta    106361 bytes, ratio 1.156, planes 14152/30721/30721/30721 bytes, encode   969.7 us (  127 MB/s), decode   301.1 us (  408 MB/s), exact
  one row a message: planes 10338.4 bytes a token against raw 12334.0 (ratio 1.193); encode + decode 122.2 us against 28.8 us, exact
      1 Gbit/s, 50 us latency: raw 148672 + 28768 = 177440 ns, planes 132704 + 122217 = 254921 ns a token
     10 Gbit/s, 50 us latency: raw 59868 + 28768 = 88636 ns, planes 58271 + 122217 = 180488 ns a token
    100 Gbit/s, 50 us latency: raw 50987 + 28768 = 79755 ns, planes 50828 + 122217 = 173045 ns a token
    planes pays for its extra CPU time only below 0.171 Gbit/s
boundary after layer 1, 10 rows in one message of 122916 bytes:
  raw      122926 bytes, ratio 1.000, planes 0/0/0/0 bytes, encode   154.0 us (  798 MB/s), decode   132.3 us (  929 MB/s), exact
  planes   103267 bytes, ratio 1.190, planes 11058/30721/30721/30721 bytes, encode   945.5 us (  130 MB/s), decode   291.2 us (  422 MB/s), exact
  delta    106433 bytes, ratio 1.155, planes 14224/30721/30721/30721 bytes, encode   969.4 us (  127 MB/s), decode   300.6 us (  409 MB/s), exact
  one row a message: planes 10347.3 bytes a token against raw 12334.0 (ratio 1.192); encode + decode 122.6 us against 29.1 us, exact
      1 Gbit/s, 50 us latency: raw 148672 + 29096 = 177768 ns, planes 132776 + 122613 = 255389 ns a token
     10 Gbit/s, 50 us latency: raw 59868 + 29096 = 88964 ns, planes 58278 + 122613 = 180891 ns a token
    100 Gbit/s, 50 us latency: raw 50987 + 29096 = 80083 ns, planes 50828 + 122613 = 173441 ns a token
    planes pays for its extra CPU time only below 0.170 Gbit/s
Boundary codec slices: 20 s
```

What the run shows:

- **The ratio is 1.19, and it comes from the exponent alone.** The
  exponent plane shrinks from 30,720 bytes to 11,029, about 2.9 bits a
  value. The other three planes are stored plain. Even a perfect exponent
  coder would leave 24 near-random bits a value, so lossless coding of
  these f32 rows cannot pass about 32 / 24 = 1.33.
- **Delta is worse than planes** (1.156 against 1.191). Neighbouring
  positions are different tokens, and XOR-ing their bits adds disorder.
- **The coder is slow compared with the link.** Planes encodes at 130 MB/s
  and decodes at 425 MB/s. A decode step saves 1,996 bytes, which is
  16,000 bits, and spends about 93 µs more CPU than raw. That trade pays
  only on a link slower than 16,000 bits / 93 µs, about 0.17 Gbit/s. On
  chapter 182's 10 Gbit/s link a token takes 88.6 µs raw and 180.5 µs with
  planes; at 1 Gbit/s, 177.4 µs against 254.9 µs. The 50 µs of latency
  does not shrink with the bytes at all.

So raw stays the default between stages on any link this book models. The
planes codec is worth offering to a peer behind a slow link, such as a
home connection of tens of megabits. The receiver accepts it, and the
sender picks it only when its own measurement says it pays. This codec is
plain scalar Coax with bounds checks. A faster coder (several rANS states
interleaved, or a table-driven decoder) would move the break-even, but not
the 1.33 ceiling.

## Checks

`boundary-codec.coax` is `#authority`. Its checks, with helpers in
[`boundary-codec-checks.coax`](../tests/boundary-codec-checks.coax):

- FNV-1a matches its published values for "", "a" and "foobar".
- The plane split of 1.0, −2.0, 0.5, −0.0, a NaN with a payload, the
  smallest subnormal and all ones, and that `bc_place` puts every word back.
- The hand-worked plane: its table, its stream's state 536,909,824, the
  plain plane it becomes, a coded plane built by hand that decodes to the
  exponents, and the 62-byte raw frame.
- Table rounding and ties: the first of two equally frequent symbols takes
  the leftover slot; a symbol seen once in 4,200 values gets frequency 1
  and still decodes.
- A coded frame (64 values with one exponent) byte by byte: plane sizes,
  maps, frequencies, the state 2²³ of a one-symbol plane, the header and
  the checksum. Encoding into a buffer full of old bytes gives the same
  frame, and decoding into one gives the same words.
- A plane whose coded and plain sizes are equal (found by search) is
  stored plain.
- A `#forall` over 60 random messages of 1 to 3 rows of 1 to 40 values:
  activation-like values, NaN payloads, negative zero, infinities,
  subnormals, any 32 bits, repeated rows and BF16-like values. Each
  round-trips exactly through all three codecs and fits the bound.
- A `#forall` over 1 to 3 of the recorded real rows
  ([`boundary-codec-real.coax`](../tests/boundary-codec-real.coax)) and
  both coded codecs: exact round trips, and every frame smaller than raw.
- Each refusal, by the sender and by the receiver, at its boundary,
  including frames sealed again after a change to a mode, a frequency, a
  state, a stream length or the payload's length.
- `coax mutate --root-only` leaves no survivors (500 mutants).

The slice above is the check on original weights. It is not part of
`psionic/check.sh`, because it needs the 7.7 GB checkpoint.

## What remains

- **A real link.** The costs above use chapter 182's link model and
  measured CPU time in one process. Frames have not crossed
  [chapter 188](ch188-peers-transport-and-discovery.md)'s link, so
  end-to-end latency on a real network is not measured.
- **Negotiation on the wire.** `bc_choose` works on a bit mask. Carrying
  the mask in chapter 188's handshake, and choosing by a measured link
  rate, remain.
- **The codec in the pipeline.** `stg_pipeline` still moves raw messages;
  the codec runs beside it in the examples and the slice.
- **Lossy formats.** BF16 or 8-bit activations would halve or quarter the
  bytes, but they change the model's output. They need their own profile
  name and a quality evaluation under
  [chapter 17](ch017-qualify-honestly.md)'s rules. None exists yet, so no
  lossy codec shares a name with an exact one.

## Dependencies

- [Chapter 186](ch186-split-a-model-into-stages.md): the boundary message
  the frame carries, `stg_pack` and `stg_receive`, and the original-weight
  slice this chapter's slice follows.
- [Chapter 182](ch182-disaggregate-prefill-and-decode.md): the link model.
- [Chapter 188](ch188-peers-transport-and-discovery.md): the link the
  frames will cross.
- [Chapter 17](ch017-qualify-honestly.md): comparison rules for a lossy
  profile.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [DeepSeek target](../docs/targets/deepseek.md)
- J. Duda, "Asymmetric numeral systems: entropy coding combining speed of
  Huffman coding with compression rate of arithmetic coding", 2013
  (arXiv:1311.2540).
