# 156. A spectral neural operator

A Fourier neural operator (FNO) learns a map from one field to another, such
as from a material's permeability to the pressure it produces. Its building
block is the Fourier layer: transform each input field, keep only its lowest
frequencies, multiply each kept frequency by a learned complex weight, and
transform back. Because the weights belong to frequencies rather than to grid
points, the same layer can run on a coarse grid or a fine one.

This chapter builds one Fourier layer, forward and backward, in
[`fourier.coax`](../lib/fourier.coax):

- a **two-dimensional FFT** made from [chapter 151](ch151-waveforms-and-spectral-features.md)'s
  one-dimensional transform, rows then columns, checked against a direct 2-D
  DFT;
- **mode truncation**: keep the signed frequencies p in (−k1, k1) down the
  rows and q in (−k2, k2) across, and drop the rest;
- the **layer**: complex weights on the kept modes, mixing input channels
  into output channels, an inverse transform, a pointwise linear path and a
  bias, then an activation (none, or GELU in its tanh form);
- its **gradients** for every weight and every input value, written by hand
  and checked against central finite differences;
- **refusals** for a field too small to hold the kept modes, too many modes
  or channels, bad sizes and lengths, and non-finite or oversized values.

Everything runs in binary64 on one CPU core, with sums in a fixed order. That
is the profile `psionic-fourier-v1`. A grid has 1 to 64 points a side and is
periodic; a layer has 1 to 4 input and output channels and keeps at most 8
modes in each direction; every input and parameter lies within 10⁶ in
magnitude. Training a stack of these layers on a partial differential
equation is [chapter 157](ch157-learn-darcy-flow.md)'s job.

## Run it

The program is [`fourier-layer.coax`](../examples/fourier-layer.coax); the
checks' oracles and fixtures are in
[`fourier-checks.coax`](../tests/fourier-checks.coax).

```shell
coax/bin/coax run psionic/examples/fourier-layer.coax
coax/bin/coax test psionic/lib/fourier.coax
coax/bin/coax mutate psionic/lib/fourier.coax --root-only
```

```text
x: 1.0 2.0 3.0 4.0
FFT real: 10.0 -2.0 -2.0 -2.0
FFT imaginary: 0.0 2.0 0.0 -2.0
y: 1.25 2.75 3.75 3.25
dL/dx for L = y[0]: 0.875 0.5 0.125 0.0
dL/dwr: -0.5 2.5 -0.5
dL/dwi: 0.5 0.0 -0.5
6 x 10 FFT within 1e-12 of the direct DFT: true
layer: 206 weights; 6 x 10 output within 1e-12 of the direct convolution: true
8 x 8 and 16 x 16 agree on shared points within 1e-12: true
  channel 0, row 0, 8 x 8: -0.149 -0.166 -0.1631 0.042
  channel 0, row 0, 16 x 16: -0.149 -0.166 -0.1631 0.042
a 4 x 10 field with k1 = 3: modes
k1 = 9: modes
a 65 x 1 grid: size
a NaN input: number
an input of 2,000,000: number
119 input values for 2 x 6 x 10: shape
room for 119 outputs: capacity
5 output channels: channels
```

The example compiles and runs in about ten seconds.

## One layer, worked by hand

Take one channel on a 1 × 4 grid, x = 1 2 3 4, with k1 = 1 and k2 = 2. One
row means only p = 0 is kept down the rows; across, q runs over −1, 0 and 1.
Those land in bins 3, 0 and 1 of a length-4 transform, and bin 2 (q = ±2) is
dropped.

Chapter 151 worked out the transform of 1 2 3 4:
X = 10, −2 + 2i, −2, −2 − 2i. Give the three kept modes the weights

| q | bin | X[q] | weight W[q] | W[q] · X[q] |
| --- | --- | --- | --- | --- |
| −1 | 3 | −2 − 2i | 0.5 | −1 − i |
| 0 | 0 | 10 | 1 | 10 |
| 1 | 1 | −2 + 2i | i | −2 − 2i |

and set bin 2 to zero. The inverse transform is
s[n] = (1/4) Σ Z[k] iᵏⁿ, and the layer keeps its real part:

- s[0] = (10 + (−2 − 2i) + (−1 − i)) / 4 = (7 − 3i) / 4, real part 1.75
- s[1] = (10 + (−2 − 2i)·i + (−1 − i)·(−i)) / 4 = (11 − i) / 4, real part 2.75
- s[2] = (10 − (−2 − 2i) − (−1 − i)) / 4 = (13 + 3i) / 4, real part 3.25
- s[3] = (10 + (−2 − 2i)·(−i) + (−1 − i)·i) / 4 = (9 + i) / 4, real part 2.25

The values sum to 10, the input's sum, because the weight on q = 0 is 1. With
a pointwise weight of 0.5 and a bias of −1 and no activation,
y = s + 0.5x − 1 = 1.25 2.75 3.75 3.25, the transcript's `y` line. A field
that is all in the dropped mode, x = 1 −1 1 −1, has only bin 2 and gives a
spectral path of 0 0 0 0.

Now the gradient of L = y[0]. The bias gets 1 and the pointwise weight gets
x[0] = 1. For the spectral weights, L = Re Σ W[k] X[k] H[k], where
H = conj(FFT g)/4 and g = 1 0 0 0 is the gradient flowing into the layer's
output, so H[k] = 1/4 for every k. Then

- dL/d(Re W[k]) = Re(X[k] H[k]) = Re X[k] / 4: −0.5, 2.5, −0.5 for q = −1, 0, 1;
- dL/d(Im W[k]) = −Im(X[k] H[k]) = −Im X[k] / 4: 0.5, 0, −0.5.

For the input, dL/dx[m] = Re Σ W[k] H[k] e^(−2πi·km/4) plus the pointwise 0.5
at m = 0. W·H is 0.125, 0.25 and 0.25i at bins 3, 0 and 1, so the sums are
0.375, 0.5, 0.125 and 0, and dL/dx = 0.875 0.5 0.125 0. Those three lines
of the transcript are this calculation, and a `#test` asserts every value
exactly.

## The two-dimensional transform

A 2-D DFT separates: transform every row, then every column of the result.
`fo_fft2` checks the grid and then does exactly that with chapter 151's
`sf_forward`, which picks radix-2 for a power of two and Bluestein's chirp
transform for any other length. The inverse conjugates, transforms forward,
conjugates back and divides by ny · nx, as chapter 151's one-dimensional
inverse does.

<!-- source: psionic/lib/fourier.coax fft2 -->
```jai
// The 2-D FFT in place, row r of an ny x nx grid at r nx: chapter 151's
// one-dimensional transform on every row, then on every column. The inverse
// conjugates, transforms forward, conjugates back and divides by ny nx.
fo_fft2 :: (re:[]f64,im:[]f64,ny:int,nx:int,inverse:bool)->FoStatus {
    if ny<1 || nx<1 || ny>FO_SIDE_MAX || nx>FO_SIDE_MAX {return FoStatus.size;}
    if re.count!=ny*nx || im.count!=ny*nx {return FoStatus.shape;}
    for i:0..<ny*nx {if !sf_finite(re[i]) || !sf_finite(im[i]) {return FoStatus.number;}}
    fo_transform(re,im,ny,nx,inverse);
    return FoStatus.ok;
}
fo_transform :: (re:[]f64,im:[]f64,ny:int,nx:int,inverse:bool) {
    cells:=ny*nx;
    if inverse {for t:0..<cells {im[t]= -im[t];}}
    for r:0..<ny {sf_forward(re[r*nx..<(r+1)*nx],im[r*nx..<(r+1)*nx]);}
    cr:[FO_SIDE_MAX]f64;ci:[FO_SIDE_MAX]f64;
    for c:0..<nx {
        for r:0..<ny {cr[r]=re[r*nx+c];ci[r]=im[r*nx+c];}
        sf_forward(cr[0..<ny],ci[0..<ny]);
        for r:0..<ny {re[r*nx+c]=cr[r];im[r*nx+c]=ci[r];}
    }
    if inverse {for t:0..<cells {re[t]/=f64(cells);im[t]= -im[t]/f64(cells);}}
}
```

For [[1, 2], [3, 4]] the rows give [3, −1] and [7, −1], and the columns give
[[10, −2], [−4, 0]]; the first check asserts those four numbers and the
inverse that returns the input.

## Kept modes and the layer's shape

<!-- source: psionic/lib/fourier.coax types -->
```jai
FoStatus :: enum u8 {ok;shape;size;modes;channels;number;capacity;}
FoAct :: enum u8 {none;gelu;}
// One Fourier layer from cin to cout channels. It keeps the signed
// frequencies p in (-k1, k1) down the rows and q in (-k2, k2) across:
// (2 k1 - 1)(2 k2 - 1) modes. Mode (p, q) from input i to output o has the
// weight wr + i wi at ((o cin + i)(2 k1 - 1) + p + k1 - 1)(2 k2 - 1) + q + k2 - 1.
// lin is cout x cin, bias has cout entries.
FoLayer :: struct {cin:int;cout:int;k1:int;k2:int;act:FoAct;wr:[]f64;wi:[]f64;lin:[]f64;bias:[]f64;}
// The gradient of a scalar loss for each parameter, laid out like the layer.
FoGrad :: struct {wr:[]f64;wi:[]f64;lin:[]f64;bias:[]f64;}
```

<!-- source: psionic/lib/fourier.coax bounds -->
```jai
FO_SIDE_MAX :: 64;
FO_CELLS_MAX :: 4096;            // 64 x 64
FO_CHANNELS_MAX :: 4;
FO_MODES_MAX :: 8;               // k1, k2 at most 8
FO_KEPT_MAX :: 225;              // (2 x 8 - 1)^2 kept modes
FO_VALUE_MAX :: 1000000.0;       // larger inputs and parameters are refused
```

A kept mode is indexed (a, b) with a in 0 ..< 2k1 − 1; its signed frequency
is p = a − (k1 − 1), and it lives in bin p mod ny. The 2k1 − 1 kept rows are
distinct bins only when ny ≥ 2k1 − 1, so a field smaller than that is
refused rather than folded onto itself.

<!-- source: psionic/lib/fourier.coax modes -->
```jai
// Kept mode (a, b), a in 0 ..< 2 k1 - 1, is the signed frequency
// p = a - (k1 - 1); it lives in bin p mod ny of the transform.
fo_bin :: (p:int,n:int)->int {return (p+n)%n;}
fo_cell :: (l:FoLayer,a:int,b:int,ny:int,nx:int)->int {return fo_bin(a-l.k1+1,ny)*nx+fo_bin(b-l.k2+1,nx);}
// The kept modes of each input channel's transform, kept[i][a][b].
fo_spectra :: (l:FoLayer,ny:int,nx:int,x:[]f64,kr:[]f64,ki:[]f64) {
    cells:=ny*nx;kept:=fo_kept(l);m2:=2*l.k2-1;re:[FO_CELLS_MAX]f64;im:[FO_CELLS_MAX]f64;
    for i:0..<l.cin {
        for t:0..<cells {re[t]=x[i*cells+t];im[t]=0;}
        fo_transform(re[0..<cells],im[0..<cells],ny,nx,false);
        for s:0..<kept {t:=fo_cell(l,s/m2,s%m2,ny,nx);kr[i*kept+s]=re[t];ki[i*kept+s]=im[t];}
    }
}
```

<!-- source: psionic/lib/fourier.coax admit -->
```jai
fo_kept :: (l:FoLayer)->int {return (2*l.k1-1)*(2*l.k2-1);}
fo_values_ok :: (v:[]f64)->bool {
    for i:0..<v.count {if !sf_finite(v[i]) || abs(v[i])>FO_VALUE_MAX {return false;}}
    return true;
}
// A layer and its input: channels, grid, modes, lengths, then every value.
// A side shorter than 2k - 1 points cannot hold the kept modes apart.
fo_check :: (l:FoLayer,ny:int,nx:int,x:[]f64)->FoStatus {
    if l.cin<1 || l.cout<1 || l.cin>FO_CHANNELS_MAX || l.cout>FO_CHANNELS_MAX {return FoStatus.channels;}
    if ny<1 || nx<1 || ny>FO_SIDE_MAX || nx>FO_SIDE_MAX {return FoStatus.size;}
    if l.k1<1 || l.k2<1 || l.k1>FO_MODES_MAX || l.k2>FO_MODES_MAX {return FoStatus.modes;}
    if 2*l.k1-1>ny || 2*l.k2-1>nx {return FoStatus.modes;}
    m:=fo_kept(l)*l.cout*l.cin;
    if l.wr.count!=m || l.wi.count!=m || l.lin.count!=l.cout*l.cin || l.bias.count!=l.cout || x.count!=l.cin*ny*nx {return FoStatus.shape;}
    if !fo_values_ok(l.wr) || !fo_values_ok(l.wi) || !fo_values_ok(l.lin) || !fo_values_ok(l.bias) || !fo_values_ok(x) {return FoStatus.number;}
    return FoStatus.ok;
}
```

## The forward pass

For each output channel o, the layer sums W_oi · X_i over the input channels
at every kept mode, writes those sums into an otherwise zero spectrum, takes
the inverse transform and keeps the real part. It then adds the pointwise
path Σ lin_oi x_i and the bias, saves that value as `pre` for the backward
pass, and applies the activation.

<!-- source: psionic/lib/fourier.coax forward -->
```jai
// y_o = act(Re IFFT2(sum over i of W_oi . keep(FFT2 x_i)) + sum over i of
// lin_oi x_i + bias_o). pre holds the value before the activation, for the
// backward pass. Fields are channel-major: x[i][r][c] at (i ny + r) nx + c.
fo_forward :: (l:FoLayer,ny:int,nx:int,x:[]f64,pre:[]f64,y:[]f64)->FoStatus {
    status:=fo_check(l,ny,nx,x);if status!=FoStatus.ok {return status;}
    cells:=ny*nx;kept:=fo_kept(l);
    if pre.count<l.cout*cells || y.count<l.cout*cells {return FoStatus.capacity;}
    kr:[FO_CHANNELS_MAX*FO_KEPT_MAX]f64;ki:[FO_CHANNELS_MAX*FO_KEPT_MAX]f64;
    fo_spectra(l,ny,nx,x,kr[..],ki[..]);
    for o:0..<l.cout {
        re:[FO_CELLS_MAX]f64;im:[FO_CELLS_MAX]f64;   // zero outside the kept modes
        for s:0..<kept {
            zr:=0.0;zi:=0.0;
            for i:0..<l.cin {
                w:=(o*l.cin+i)*kept+s;k:=i*kept+s;
                zr+=l.wr[w]*kr[k]-l.wi[w]*ki[k];zi+=l.wr[w]*ki[k]+l.wi[w]*kr[k];
            }
            t:=fo_cell(l,s/(2*l.k2-1),s%(2*l.k2-1),ny,nx);re[t]=zr;im[t]=zi;
        }
        fo_transform(re[0..<cells],im[0..<cells],ny,nx,true);
        for t:0..<cells {
            v:=re[t]+l.bias[o];
            for i:0..<l.cin {v+=l.lin[o*l.cin+i]*x[i*cells+t];}
            pre[o*cells+t]=v;y[o*cells+t]=fo_act(l.act,v);
        }
    }
    return FoStatus.ok;
}
```

The activation is GELU in its tanh form,
z/2 · (1 + tanh(√(2/π) · (z + 0.044715 z³))), with its slope written out:

<!-- source: psionic/lib/fourier.coax act -->
```jai
fo_tanh :: (a:f64)->f64 {
    t:=exp(-2*abs(a));v:=(1-t)/(1+t);
    if a<0 {return -v;}
    return v;
}
FO_GELU_C :: 0.7978845608028654;   // sqrt(2 / pi)
FO_GELU_A :: 0.044715;
// GELU in its tanh form: z/2 (1 + tanh(c (z + a z^3))).
fo_act :: (act:FoAct,z:f64)->f64 {
    if act==FoAct.none {return z;}
    return 0.5*z*(1+fo_tanh(FO_GELU_C*(z+FO_GELU_A*z*z*z)));
}
fo_slope :: (act:FoAct,z:f64)->f64 {
    if act==FoAct.none {return 1;}
    t:=fo_tanh(FO_GELU_C*(z+FO_GELU_A*z*z*z));
    return 0.5*(1+t)+0.5*z*(1-t*t)*FO_GELU_C*(1+3*FO_GELU_A*z*z);
}
```

## The backward pass

Write g_o = dL/dy_o · act′(pre_o) for the gradient arriving at the output
before the activation. The bias gradient is Σ g_o and the pointwise weight's
is Σ g_o x_i. For the spectral path, the inverse transform of a spectrum Z
evaluated against g is

Σₙ g[n] · Re((1/N) Σₖ Z[k] e^(2πi·kn/N)) = Re Σₖ Z[k] · conj(G[k]) / N,

where G is the forward transform of the real field g and N = ny · nx. So with
H = conj(G)/N on the kept modes, the loss is linear in each Z = W · X, and

- dL/d(Re W) = Re(X H) and dL/d(Im W) = −Im(X H);
- dL/dx_i = Re FFT2(Σ_o W_oi H_o), because X_i[k] = Σₙ x_i[n] e^(−2πi·kn/N).

Each gradient therefore costs one forward transform per output channel and
one per input channel, the same as the forward pass.

<!-- source: psionic/lib/fourier.coax backward -->
```jai
// Given dL/dy, write dL/dx and every parameter's gradient (overwritten, not
// added to). With g = dL/dy act'(pre) and H_o = conj(FFT2 g_o)/(ny nx) on
// the kept modes, L's spectral part is Re(sum over k of W X H), so
// dL/dwr = Re(X H), dL/dwi = -Im(X H), and dL/dx_i = Re FFT2(sum over o of
// W_oi H_o) plus the pointwise path.
fo_backward :: (l:FoLayer,ny:int,nx:int,x:[]f64,pre:[]f64,dy:[]f64,dx:[]f64,g:FoGrad)->FoStatus {
    status:=fo_check(l,ny,nx,x);if status!=FoStatus.ok {return status;}
    cells:=ny*nx;kept:=fo_kept(l);m:=kept*l.cout*l.cin;n:=l.cout*cells;
    if pre.count!=n || dy.count!=n {return FoStatus.shape;}
    if !fo_values_ok(pre) || !fo_values_ok(dy) {return FoStatus.number;}
    if dx.count<l.cin*cells || g.wr.count<m || g.wi.count<m || g.lin.count<l.cout*l.cin || g.bias.count<l.cout {return FoStatus.capacity;}
    kr:[FO_CHANNELS_MAX*FO_KEPT_MAX]f64;ki:[FO_CHANNELS_MAX*FO_KEPT_MAX]f64;
    fo_spectra(l,ny,nx,x,kr[..],ki[..]);
    gz:[FO_CHANNELS_MAX*FO_CELLS_MAX]f64;
    hr:[FO_CHANNELS_MAX*FO_KEPT_MAX]f64;hi:[FO_CHANNELS_MAX*FO_KEPT_MAX]f64;
    m2:=2*l.k2-1;
    for o:0..<l.cout {
        sum:=0.0;re:[FO_CELLS_MAX]f64;im:[FO_CELLS_MAX]f64;
        for t:0..<cells {v:=dy[o*cells+t]*fo_slope(l.act,pre[o*cells+t]);gz[o*cells+t]=v;sum+=v;re[t]=v;}
        g.bias[o]=sum;
        for i:0..<l.cin {s:=0.0;for t:0..<cells {s+=gz[o*cells+t]*x[i*cells+t];}g.lin[o*l.cin+i]=s;}
        fo_transform(re[0..<cells],im[0..<cells],ny,nx,false);
        for s:0..<kept {t:=fo_cell(l,s/m2,s%m2,ny,nx);hr[o*kept+s]=re[t]/f64(cells);hi[o*kept+s]= -im[t]/f64(cells);}
    }
    for i:0..<l.cin {
        re:[FO_CELLS_MAX]f64;im:[FO_CELLS_MAX]f64;   // zero outside the kept modes
        for s:0..<kept {
            ar:=0.0;ai:=0.0;
            for o:0..<l.cout {
                w:=(o*l.cin+i)*kept+s;h:=o*kept+s;k:=i*kept+s;
                g.wr[w]=kr[k]*hr[h]-ki[k]*hi[h];g.wi[w]= -(kr[k]*hi[h]+ki[k]*hr[h]);
                ar+=l.wr[w]*hr[h]-l.wi[w]*hi[h];ai+=l.wr[w]*hi[h]+l.wi[w]*hr[h];
            }
            t:=fo_cell(l,s/m2,s%m2,ny,nx);re[t]=ar;im[t]=ai;
        }
        fo_transform(re[0..<cells],im[0..<cells],ny,nx,false);
        for t:0..<cells {
            v:=re[t];
            for o:0..<l.cout {v+=l.lin[o*l.cin+i]*gz[o*cells+t];}
            dx[i*cells+t]=v;
        }
    }
    return FoStatus.ok;
}
```

The gradients are overwritten, not added to; a caller that trains over a
batch sums them itself.

## Resolution

Sample a field that has no frequency at or beyond k on an ny × nx grid. Its
transform divided by ny · nx is the same set of Fourier coefficients at any
grid size large enough to hold them, the layer multiplies the same
coefficients by the same weights, and the inverse transform evaluates the
same smooth function at the new grid's points. So one set of weights gives
the same values at 8 × 8 as at 16 × 16 wherever the two grids share a point.
The transcript shows the first row agreeing to every printed digit, and a
`#forall` checks it within 10⁻¹² on four grid pairs, with and without GELU.

The property needs the input to be band-limited. The same check adds a wave
at frequency ny − 1: on the coarse grid it is indistinguishable from
frequency −1, which the layer keeps, while the fine grid sees it as a high
frequency and drops it. The outputs then differ by more than 10⁻³.

## Refusals

`fo_check` names the first problem it finds, in a fixed order: channels,
grid size, modes, lengths, then values. `fo_forward` and `fo_backward` then
check that their outputs have room. Nothing is written before every check
passes. The last eight lines of the transcript show one refusal each:

- a 4 × 10 field under k1 = 3 cannot hold five distinct kept rows: `modes`;
- k1 = 9 is more than 8 modes: `modes`, even on a 64 × 64 grid;
- a side of 65 points: `size`;
- a NaN, or a value of 2,000,000: `number`;
- 119 input values where 2 × 6 × 10 = 120 are needed: `shape`;
- room for 119 outputs: `capacity`;
- 5 output channels: `channels`.

`fo_backward` additionally refuses a `pre` or `dy` of the wrong length
(`shape`), a non-finite one (`number`) and gradient slices that are too short
(`capacity`).

## How it is checked

The checks in [`fourier.coax`](../lib/fourier.coax) come before the code they
test. The oracles in [`fourier-checks.coax`](../tests/fourier-checks.coax) do
not call the FFT or the layer:

- **Hand values.** The 2 × 2 and 1 × 4 transforms, the 1 × 4 layer's output,
  its dropped mode and every gradient, as worked above; GELU against Python's
  `math.tanh` values at −1, 0.5, 1 and 2.
- **A direct 2-D DFT.** Written with `cos` and `sin` over every pair of grid
  points, it matches `fo_fft2` within 10⁻¹² on twelve shapes from 1 × 1 to
  16 × 10, forward and inverse, and the round trip returns the input within
  10⁻¹⁴.
- **A direct spectral convolution.** Each channel pair's kernel
  k[d] = (1/N) Σ Re(W[p][q] e^(2πi(p·dr/ny + q·dc/nx))) is built from the
  weights with `cos` and `sin`, and the spectral path is the circular
  convolution Σₘ k[n − m] x[m] over every pair of points. The layer matches
  it within 10⁻¹² on eight shapes from 1 × 1 to 8 × 8 and 6 × 9, up to 4
  channels, half with GELU.
- **Finite differences.** Every weight, pointwise weight, bias and input
  gradient matches a central difference with step 10⁻⁶ within 10⁻⁷, on five
  layers from 1 × 4 to 4 × 8 and up to three channels, with and without GELU.
- **Resolution.** The same weights at two grid sizes, as described above.

`coax mutate psionic/lib/fourier.coax --root-only` changes the library's own
code one site at a time and reruns the checks. Of 392 mutants, all 392 are caught. The oracles, the shape fixtures and the finite-difference driver live in the checks file, so they are not mutated themselves.

The library is not an `#authority` file: it decides no money, admission or
policy.

To keep the compile-time checks inside their work budget, chapter 151's
radix-2 transform now reverses the index bits in place instead of copying
into an 8,192-element scratch array. The butterflies and their order are
unchanged, so every result is the same bit for bit. Mutating
`spectral.coax` leaves three survivors in that loop, all of which produce
the same permutation: starting at index 1 instead of 0 (index 0 reverses to
itself), swapping when r ≥ i (a swap with itself) and swapping when r ≤ i
(each pair is then swapped from its other end). The other 612 of 615
mutants are caught.

## What this does not do

- One layer is not a network. Lifting the input into channels, stacking
  layers, projecting back to a field and training on Darcy flow are
  [chapter 157](ch157-learn-darcy-flow.md)'s work.
- The weights are a separate complex number for every kept signed mode, so
  the layer's output is the real part of a complex result rather than being
  real by a conjugate symmetry in the weights. The reference FNO
  implementation stores half the modes and uses a real-input transform; that
  saves half the spectral weights and work and is not built here.
- The grid is periodic. A field that is not periodic, such as pressure with
  fixed boundary values, needs padding first.
- The gradients are written by hand and checked with finite differences.
  They are not derived by [chapter 106](ch106-differentiate-any-tensor-program.md)'s
  automatic differentiation, and the layer is not yet a
  [chapter 117](ch117-models-are-compositions-of-modules.md) module.
- The transform runs on one CPU core. There is no GPU path.

## Dependencies

- [Chapter 151](ch151-waveforms-and-spectral-features.md): the
  one-dimensional FFT and its twiddles.
- [Chapter 106](ch106-differentiate-any-tensor-program.md): reverse-mode
  differentiation, which this chapter writes out by hand for one layer.
- [Chapter 109](ch109-learn-a-physical-system.md): the first physics chapter,
  whose held-out evaluation pattern chapter 157 applies to this layer.

## Sources

- [Chapter plan](PLAN.md), Part XVI.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), physics section.
- Zongyi Li, Nikola Kovachki, Kamyar Azizzadenesheli, Burigede Liu, Kaushik
  Bhattacharya, Andrew Stuart and Anima Anandkumar, "Fourier Neural Operator
  for Parametric Partial Differential Equations", ICLR 2021.
- Dan Hendrycks and Kevin Gimpel, "Gaussian Error Linear Units (GELUs)",
  2016, for the tanh form of GELU.
