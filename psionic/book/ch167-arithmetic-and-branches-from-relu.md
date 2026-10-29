# 167. Arithmetic and branches from ReLU

[Chapter 166](ch166-attention-as-exact-memory.md) gave a transformer an exact
memory. A machine also needs arithmetic and branches. In a transformer those
have to come from the feed-forward layer, which is matrix products and a
nonlinearity, nothing else. This chapter shows that one kind of feed-forward
layer computes integer products, steps, equality tests and conditional
selection exactly, inside a range it states.

```shell
coax/bin/coax run psionic/examples/relu-gadgets.coax
psionic/tests/check_gates.sh
```

```jai
7 * -3 = -21.0
step(0) = 0.0, step(5) = 1.0
eq0(0) = 1.0, eq0(-4) = 0.0
select(1, 11, 22) = 11.0, select(0, 11, 22) = 22.0
2^26 * 2^26 in binary64 = 4503599627370496.0 (exact)
4097 * 4097: binary64 16785409.0, binary32 16785408.0 (exact is 16785409)
step(0.5) = 0.5: the gadget is a ramp between integers
```

The [example](../examples/relu-gadgets.coax) runs in under a second.

## A gated layer

The layer is a gated feed-forward block of the kind modern transformers use,
with ReLU as its gate: the residual `x` becomes

```jai
x + O · ((V · x) ⊙ ReLU(G · x))
```

Each hidden unit multiplies one linear function of the residual (its value) by
the ReLU of another (its gate). That product of two linear functions is what
makes exact multiplication possible. A plain ReLU layer can't multiply two
inputs at all.

<!-- source: psionic/lib/relu-arith.coax layer -->
```jai
// One gated feed-forward layer: out = x + O * ((V x) ⊙ ReLU(G x)), with V and G
// of shape units × width and O of shape width × units, stored V, G, O in a
// row-major run. Every product and sum is rounded to binary32 when `single`
// is set, in unit order, so the same weights can be measured in both formats.
ra_weight_count :: (width:int,units:int)->int #export_c {return 3*width*units;}
// False for NaN, infinities and magnitudes past 2^53.
ra_exact_range :: (v:f64)->bool {return abs(v)<=f64(RA_EXACT_MAX);}
ra_round :: (x:f64,single:bool)->f64 {
    if single {return f64(f32(x));}
    return x;
}
ra_dot :: (w:[]f64,base:int,width:int,x:[]f64,single:bool)->f64 {
    sum:f64=0;
    for i:0..<width {sum=ra_round(sum+ra_round(w[base+i]*x[i],single),single);}
    return sum;
}
ra_forward :: (width:int,units:int,w:[]f64,x:[]f64,out:[]f64,single:bool,overlap:bool)->RaStatus #export_c {
    if width<1 || width>RA_WIDTH_MAX || units<0 || units>RA_UNITS_MAX {return RaStatus.shape;}
    if w.count!=ra_weight_count(width,units) || x.count!=width || out.count<width {return RaStatus.capacity;}
    if overlap {return RaStatus.alias;}
    for v:w {if !ra_exact_range(v) {return RaStatus.number;}}
    for v:x {if !ra_exact_range(v) {return RaStatus.number;}}
    hidden:[128]f64;stage:[64]f64;
    for u:0..<units {
        g:=ra_dot(w,(units+u)*width,width,x,single);
        // ReLU without a comparison: (g + |g|) / 2 is g when positive and 0 otherwise.
        hidden[u]=ra_round(ra_dot(w,u*width,width,x,single)*((g+abs(g))*0.5),single);
    }
    for i:0..<width {
        sum:=x[i];
        for u:0..<units {sum=ra_round(sum+ra_round(w[2*units*width+i*units+u]*hidden[u],single),single);}
        stage[i]=sum;
    }
    for i:0..<width {out[i]=stage[i];}
    return RaStatus.ok;
}
```

ReLU is written as `(g + |g|) / 2`, which is exact in binary64. When `single`
is set, every product and sum is rounded to binary32 in unit order. The same
weights can then be measured in both formats, as chapter 172 did for attention.

## Units from linear forms

A unit reads two linear forms over residual slots. Slot 0 always holds 1, so
a form can include a constant. Adding two forms merges terms on the same slot
and drops terms that cancel, so `x − x` is the empty form. A unit adds
`scale × value × ReLU(gate)` to one target slot.

<!-- source: psionic/lib/relu-arith.coax forms -->
```jai
ra_term :: (slot:int,coef:int)->RaForm {f:RaForm;f.count=1;f.slots[0]=slot;f.coefs[0]=coef;return f;}
ra_one :: ()->RaForm {return ra_term(0,1);}
// Terms on the same slot merge and terms that cancel are dropped, so x - x is
// the empty form. A sum that needs more than eight distinct slots at any point
// is refused (count -1).
ra_plus :: (a:RaForm,b:RaForm)->RaForm {
    r:=a;
    if a.count<0 || b.count<0 {r.count=-1;return r;}
    for i:0..<b.count {
        found:=-1;
        for j:0..<r.count {if r.slots[j]==b.slots[i] {found=j;}}
        if found>=0 {r.coefs[found]+=b.coefs[i];}
        else {
            if r.count==RA_TERMS {r.count=-1;return r;}
            r.slots[r.count]=b.slots[i];r.coefs[r.count]=b.coefs[i];r.count+=1;
        }
    }
    kept:=0;
    for j:0..<r.count {if r.coefs[j]!=0 {r.slots[kept]=r.slots[j];r.coefs[kept]=r.coefs[j];kept+=1;}}
    r.count=kept;return r;
}
ra_neg :: (a:RaForm)->RaForm {r:=a;for i:0..<a.count {r.coefs[i]=-a.coefs[i];}return r;}
```

<!-- source: psionic/lib/relu-arith.coax unit -->
```jai
// Unit u reads the value form and the gate form, and adds scale * value *
// ReLU(gate) to residual slot `target`. Weights start at zero; units add.
ra_unit :: (w:[]f64,width:int,units:int,u:int,value:RaForm,gate:RaForm,target:int,scale:int)->bool {
    if u<0 || u>=units || target<0 || target>=width || w.count<ra_weight_count(width,units) {return false;}
    if value.count<0 || gate.count<0 {return false;}
    for i:0..<value.count {if value.slots[i]<0 || value.slots[i]>=width {return false;}}
    for i:0..<gate.count {if gate.slots[i]<0 || gate.slots[i]>=width {return false;}}
    for i:0..<value.count {w[u*width+value.slots[i]]+=f64(value.coefs[i]);}
    for i:0..<gate.count {w[(units+u)*width+gate.slots[i]]+=f64(gate.coefs[i]);}
    w[2*units*width+target*units+u]+=f64(scale);
    return true;
}
```

## The gadgets

| Gadget | Units | Identity |
| --- | --- | --- |
| `mul(a, b)` | 2 | `a·ReLU(b) − a·ReLU(−b) = a·b` |
| `relu(a)` | 1 | `1·ReLU(a)` |
| `step(a)` | 2 | `ReLU(a) − ReLU(a − 1)`: 1 for `a ≥ 1`, 0 for `a ≤ 0` |
| `eq0(a)` | 5 | `1 − step(a) − step(−a)` |
| `select(c, a, b)` | 2 | `b·ReLU(1) + (a − b)·ReLU(c)`, for `c` in {0, 1} |
| `copy(a)` | 1 | `a·ReLU(1)` |

<!-- source: psionic/lib/relu-arith.coax gadgets -->
```jai
// Each gadget writes its units starting at `u` and returns the next free unit,
// or -1 when they do not fit (units already placed stay written). Contracts:
// integer inputs within RA_VALUE_MAX; `select` needs a condition that is
// exactly 0 or 1. Breaking a contract gives a wrong number, not a refusal.
// a * b = a * ReLU(b) - a * ReLU(-b)
ra_mul :: (w:[]f64,width:int,units:int,u:int,a:RaForm,b:RaForm,target:int)->int #export_c {
    if !ra_unit(w,width,units,u,a,b,target,1) || !ra_unit(w,width,units,u+1,a,ra_neg(b),target,-1) {return -1;}
    return u+2;
}
// ReLU(a) = 1 * ReLU(a)
ra_relu :: (w:[]f64,width:int,units:int,u:int,a:RaForm,target:int)->int #export_c {
    if !ra_unit(w,width,units,u,ra_one(),a,target,1) {return -1;}
    return u+1;
}
// step(a) = ReLU(a) - ReLU(a - 1): 1 for a >= 1, 0 for a <= 0 on integers
ra_step :: (w:[]f64,width:int,units:int,u:int,a:RaForm,target:int)->int #export_c {
    if !ra_unit(w,width,units,u,ra_one(),a,target,1) || !ra_unit(w,width,units,u+1,ra_one(),ra_plus(a,ra_term(0,-1)),target,-1) {return -1;}
    return u+2;
}
// eq0(a) = 1 - (ReLU(a) - ReLU(a - 1)) - (ReLU(-a) - ReLU(-a - 1))
ra_eq0 :: (w:[]f64,width:int,units:int,u:int,a:RaForm,target:int)->int #export_c {
    one:=ra_one();n:=ra_neg(a);
    if !ra_unit(w,width,units,u,one,one,target,1) || !ra_unit(w,width,units,u+1,one,a,target,-1) ||
        !ra_unit(w,width,units,u+2,one,ra_plus(a,ra_term(0,-1)),target,1) || !ra_unit(w,width,units,u+3,one,n,target,-1) ||
        !ra_unit(w,width,units,u+4,one,ra_plus(n,ra_term(0,-1)),target,1) {return -1;}
    return u+5;
}
// select(c, a, b) = b + (a - b) * ReLU(c), for c in {0, 1}
ra_select :: (w:[]f64,width:int,units:int,u:int,c:RaForm,a:RaForm,b:RaForm,target:int)->int #export_c {
    if !ra_unit(w,width,units,u,b,ra_one(),target,1) || !ra_unit(w,width,units,u+1,ra_plus(a,ra_neg(b)),c,target,1) {return -1;}
    return u+2;
}
// copy(a) = a * ReLU(1)
ra_copy :: (w:[]f64,width:int,units:int,u:int,a:RaForm,target:int)->int #export_c {
    if !ra_unit(w,width,units,u,a,ra_one(),target,1) {return -1;}
    return u+1;
}
```

## Where exactness holds

Each identity is exact over the integers. Floating point keeps it exact only
while every intermediate is an integer the format represents:

- **binary64:** integers up to 2⁵³. With inputs within ±2²⁶ (`RA_VALUE_MAX`),
  every product is at most 2⁵², so every gadget is exact. Past that it isn't:
  (2²⁷ + 1)² needs 55 bits, and the check shows binary64 rounding it.
- **binary32:** integers up to 2²⁴, so a product is exact only while it stays
  under 2²⁴. 4096 × 4096 = 2²⁴ is exact; 4097 × 4097 rounds to 16,785,408.
- **The layer refuses** weights and inputs past 2⁵³, NaN and infinities as
  `number`.

The gadgets also have contracts that the layer can't check, because a layer
only computes:
- **`step` needs integer inputs.** Between integers it's a ramp, and
  `step(0.5)` is 0.5.
- **`select` needs a condition of exactly 0 or 1.** With 2, it returns
  `b + 2(a − b)`.

Chapter 168 enforces both: its evaluator refuses values past the range, and
its type rule allows only a step or an equality as a condition.

## How it is checked

- **Coax checks:**
  - The layer formula on a worked example, every refusal, and the width and
    unit limits.
  - Every product of integers from −6 to 6, and the binary64 and binary32
    boundaries.
  - Step, equality, ReLU, select and copy on −5 to 5 in both formats.
  - The ramp between integers, the contract violations, and the form and unit
    rules.
- **An independent oracle** in [gate_oracle.py](../tests/gate_oracle.py)
  produces 442 gadget results, including binary32 products recomputed with
  `struct` rounding. The native harness [gate.c](../tests/gate.c) agrees on
  every one, plain and under ASan/UBSan, in debug and release, and also checks
  NaN and infinity refusals.
- **Mutation testing** of `relu-arith.coax` kills all 295 mutants. The first
  run left 44 survivors, almost all in checks that couldn't tell a misplaced
  unit from a correct one. The fix was to size each gadget's layer to exactly
  its units, so a misplaced unit fails outright. Merging like terms came later,
  after chapter 169's harness found a form that grew without it.

The [verification record](../../docs/verification/psionic-ch167-168-2026-10-29.md)
has the commands and results.

## What this does not do yet

These gadgets are single layers placed by hand. Chapter 168 compiles whole
programs of them into a stack of layers.
