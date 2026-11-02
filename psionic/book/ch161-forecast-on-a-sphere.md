# 161. Forecast on a sphere

> **Status: partly implemented.** The spherical harmonic transform, the
> spherical Fourier layer and a small forecaster trained and rolled out on
> synthetic dynamics run on the CPU. The pinned FourCastNet3 checkpoint and a
> step against its recorded output, its 72 variables on the 721 × 1440 grid,
> full-size activation and memory budgets, and ERA5 data and terms remain.

A weather model forecasts fields on the whole Earth: temperature, wind and
pressure at every latitude and longitude. FourCastNet3 does this with
*spherical* Fourier neural operators, which work in the harmonics of the
sphere rather than in the sines and cosines of a flat, periodic image. The
difference matters at the poles. A latitude-longitude grid squeezes a whole
row of points into a tiny circle near each pole, and a flat FFT treats the
north and south edges of the image as neighbours. A spherical transform does
neither: it sees the field as a function on the sphere.

This chapter builds the pieces that idea needs, in
[`sphere.coax`](../lib/sphere.coax):

- a **spherical harmonic transform**: Gauss-Legendre latitudes, the
  orthonormal associated Legendre functions by their three-term recurrence,
  and [chapter 151](ch151-waveforms-and-spectral-features.md)'s FFT around
  each latitude, with its inverse;
- a **reaction-diffusion equation on the sphere**, solved with that
  transform, which generates the training and evaluation data;
- a **spherical Fourier layer** that multiplies each degree of the
  transform by one weight, so it commutes with rotations of the sphere;
- a **forecaster** (a pointwise lift, three such layers and a pointwise
  projection) trained with [chapter 124](ch124-adamw-and-explicit-precision.md)'s
  AdamW to predict the field one step ahead, then **rolled out** by feeding
  each forecast back in, with **area-weighted** errors reported for each lead
  time and for the polar caps separately;
- the same forecaster built from [chapter 156](ch156-a-spectral-neural-operator.md)'s
  **planar** Fourier layers, trained on the same data, as the comparison.

Everything runs in binary64 on one CPU core, with sums in a fixed order. That
is the profile `psionic-sphere-v1`. A grid has 1 to 32 latitudes and 1 to 64
longitudes and keeps degrees below some L with L at most the number of
latitudes and 2L − 1 at most the number of longitudes; a network is at most 4
channels wide (chapter 156's limit) with at most 4 layers.

**What is not here.** No FourCastNet3 code, weights or outputs are used, and
no ERA5 data is downloaded: its terms and the data's size need their own
qualification. In their place the data is generated in Coax from an equation
whose solution the chapter checks independently. The forecaster is tiny,
the grid is 16 × 32, and there is one variable instead of 72. What carries
over to the full-size model is the transform, the layer, the area weighting
and the rollout evaluation.

## Run it

The program is [`sphere-forecast.coax`](../examples/sphere-forecast.coax);
the checks' oracles are in [`sphere-checks.coax`](../tests/sphere-checks.coax).

```shell
coax/bin/coax run psionic/examples/sphere-forecast.coax
coax/bin/coax test psionic/lib/sphere.coax
coax/bin/coax mutate psionic/lib/sphere.coax --root-only
```

```text
2 Gauss nodes: x = +-0.5774 (1/sqrt 3), weights 1.0 and 1.0
cos(theta): a_10 = 2.0467 (sqrt(4 pi/3) = 2.0467), a_00 = 0.0
sin(theta) cos(phi): a_11 = 1.4472 (sqrt(2 pi/3) = 1.4472)
16 x 32 grid, degrees below 16: round trip of 136 coefficients, largest error below 1e-13: true
one step of 0.25, largest error with 2, 4, 8 substeps: 0.0002059 0.00005103 0.00001269; ratios 4.035 4.0203
16 training and 8 held-out trajectories, 5 frames each
seed 1000 at the north pole: -0.1829 -0.28 -0.4149 -0.5783 -0.7379
sphere network: 841 parameters, 80 steps; loss 0.01334 then 0.0006676
  held-out relative error by lead: 0.0569 0.0954 0.1167 0.1336
  lead 4 rms error, polar caps 0.088, below 60 degrees 0.0846
plane network: 21673 parameters, 80 steps; loss 0.01279 then 0.0004318
  held-out relative error by lead: 0.0419 0.0745 0.0997 0.1251
  lead 4 rms error, polar caps 0.1211, below 60 degrees 0.0741
persistence by lead: 0.3062 0.4986 0.6183 0.6926
a grid of 33 latitudes: size
16 degrees on 30 longitudes: degree
a negative diffusion: flow
a NaN in the field: number
17 degrees in a layer on this grid: network
```

The example trains each network on 16 trajectories for only 80 steps, so
that it compiles, checks and runs in under a minute in a debug build; its
errors, about 13% after four steps, are correspondingly rough. The longer run
under [A longer run](#a-longer-run) is the chapter's measured result.

## The transform, worked by hand

A function f on the sphere, at colatitude θ (0 at the north pole) and
longitude φ, is a sum of spherical harmonics

f(θ, φ) = Σ over l ≥ 0 and −l ≤ m ≤ l of a_lm Y_lm(θ, φ),
Y_lm = P_lm(cos θ) e^(imφ).

l is the *degree* (how many times the harmonic changes sign from pole to
pole and around), m the *order* (how many times around a latitude). This
chapter scales P_lm so that every Y_lm has unit norm on the sphere: the
integral of P_lm(x)² over x from −1 to 1 is 1/(2π). It uses no
Condon-Shortley phase, so Y_l,−m is the conjugate of Y_lm, and a real field
needs only the coefficients with m ≥ 0.

To find a_lm, integrate f against the conjugate harmonic over the sphere:

a_lm = ∫∫ f(θ, φ) P_lm(cos θ) e^(−imφ) dφ d(cos θ).

The φ integral, on nlon equally spaced longitudes, is the FFT of each
latitude's row times 2π/nlon. It is exact for orders m below nlon/2 on a
band-limited row. The integral over x = cos θ is a Gauss-Legendre quadrature:
nodes x_j at the zeros of the Legendre polynomial P_nlat and weights w_j. It
is exact for every polynomial of degree below 2 nlat, and P_lm times
P_l'm is such a polynomial (times 1 − x² for odd m) whenever both degrees are
below nlat. So a field that is a finite sum of harmonics below degree L
comes back exactly, up to rounding, when L ≤ nlat and 2L − 1 ≤ nlon.

**Two latitudes, three longitudes.** With nlat = 2, P_2(x) = (3x² − 1)/2 has
zeros x = ±1/√3 ≈ ±0.5774, and both weights are 1 (they must sum to 2, the
length of [−1, 1], and are equal by symmetry). Take f = cos θ, which is x on
each latitude: 1/√3 on the northern row of three points and −1/√3 on the
southern row. Each row is constant, so its FFT is zero except at m = 0,
where it is 3x_j; times 2π/3 that is 2π x_j. Then, with
P_10(x) = √(3/(4π)) x,

a_10 = Σ over j of w_j P_10(x_j) 2π x_j = √(3/(4π)) 2π (1/3 + 1/3)
     = √(3/(4π)) · 4π/3 = √(4π/3) ≈ 2.0467,

and a_00 = Σ w_j (1/√(4π)) 2π x_j = 0, because the two rows cancel. Now take
f = sin θ cos φ: on both rows sin θ = √(1 − 1/3) = √(2/3), times cos φ_k at
φ = 0, 2π/3, 4π/3. The FFT at m = 1 is Σ_k cos φ_k e^(−iφ_k) = 3/2, so the row
integral is (2π/3)(3/2)√(2/3) = π√(2/3). With P_11(x) = √(3/(8π)) sin θ,

a_11 = 2 · √(3/(8π)) √(2/3) · π √(2/3) = (4π/3) √(3/(8π)) = √(2π/3) ≈ 1.4472.

These are the transcript's first three lines. A `#test` asserts them within
10⁻¹⁵, and also that sin θ sin φ gives a_11 = −i √(2π/3) (sin φ is
(e^(iφ) − e^(−iφ))/(2i)), and that the inverse transform gives back each field.

### The grid

<!-- source: psionic/lib/sphere.coax grid -->
```jai
// The Legendre polynomial P_n and its derivative at z by the three-term
// recurrence k P_k = (2k - 1) z P_(k-1) - (k - 1) P_(k-2).
SpLeg :: struct {p:f64;dp:f64;}
sp_legendre_n :: (n:int,z:f64)->SpLeg {
    a:=1.0;b:=z;
    for k:2..<n+1 {c:=(f64(2*k-1)*z*b-f64(k-1)*a)/f64(k);a=b;b=c;}
    return SpLeg.{p=b,dp=f64(n)*(z*b-a)/(z*z-1)};
}
// Node j of n from Newton's method on P_n, started at cos(pi (j + 3/4)/(n + 1/2))
// and stopped at the first step that is not smaller than the one before; then
// w_j = 2/((1 - x_j^2) P_n'(x_j)^2).
sp_grid :: (nlat:int,nlon:int,deg:int)->SpGrid {
    g:SpGrid;g.nlat=nlat;g.nlon=nlon;g.deg=deg;
    if nlat<1 || nlat>SP_LAT_MAX || nlon<1 || nlon>SP_LON_MAX {g.status=SpStatus.size;return g;}
    if deg<1 || deg>nlat || 2*deg-1>nlon {g.status=SpStatus.degree;return g;}
    for j:0..<nlat {
        z:=cos(SP_PI*(f64(j)+0.75)/(f64(nlat)+0.5));
        q:=sp_legendre_n(nlat,z);d:=q.p/q.dp;
        while true {
            z-=d;q=sp_legendre_n(nlat,z);e:=q.p/q.dp;
            if abs(e)>=abs(d) {break;}
            d=e;
        }
        g.x[j]=z;g.w[j]=2/((1-z*z)*q.dp*q.dp);
    }
    return g;
}
```

Newton's method finds each node from the standard first guess and stops at
the first step that is not smaller than the one before, which is where
rounding takes over; a fixed count of steps would either waste work or stop
short. A check builds every grid from 1 to 32 latitudes and finds the nodes
symmetric about the equator and the weights summing to 2, both within
10⁻¹⁴.

### The Legendre functions

<!-- source: psionic/lib/sphere.coax legendre -->
```jai
// The orthonormal associated Legendre functions of order m at x, degrees m
// to deg - 1, into out[l]: integral of P_lm^2 over [-1, 1] is 1/(2 pi), so
// Y_lm = P_lm(cos theta) e^(i m phi) has unit norm on the sphere.
//   P_00 = 1/sqrt(4 pi)
//   P_mm = sqrt((2m + 1)/(2m)) sin(theta) P_(m-1)(m-1)
//   P_(m+1)m = sqrt(2m + 3) x P_mm
//   P_lm = a (x P_(l-1)m - b P_(l-2)m), a = sqrt((4l^2 - 1)/(l^2 - m^2)),
//          b = sqrt(((l - 1)^2 - m^2)/(4 (l - 1)^2 - 1))
sp_legendre :: (deg:int,m:int,x:f64,out:[]f64) {
    s:=sqrt((1-x)*(1+x));p:=1/sqrt(4*SP_PI);
    for k:1..<m+1 {p*=sqrt(f64(2*k+1)/f64(2*k))*s;}
    out[m]=p;
    if m+1<deg {out[m+1]=sqrt(f64(2*m+3))*x*p;}
    for l:m+2..<deg {
        a:=sqrt(f64(4*l*l-1)/f64(l*l-m*m));b:=sqrt(f64((l-1)*(l-1)-m*m)/f64(4*(l-1)*(l-1)-1));
        out[l]=a*(x*out[l-1]-b*out[l-2]);
    }
}
// Coefficient (l, m), 0 <= m <= l < deg, sits at l (l + 1)/2 + m.
sp_index :: (l:int,m:int)->int {return l*(l+1)/2+m;}
sp_coeffs :: (deg:int)->int {return deg*(deg+1)/2;}
```

The recurrence climbs from P_mm, which carries the factor sin^m θ, through
the degrees of one order. It needs no factorials, so it neither overflows
nor loses digits to cancellation the way the explicit polynomial does. The
checks compare it with exactly that polynomial,
P_l(x) = 2^(−l) Σ_k (−1)^k C(l, k) C(2l − 2k, l) x^(l − 2k), differentiated m
times, at four points for every degree and order below 12.

### Analysis and synthesis

<!-- source: psionic/lib/sphere.coax transform -->
```jai
// Down: a_lm = sum over j of s_j P_lm(x_j) F_j(m), where F_j(m) is the FFT
// of row j at bin m. With s_j = w_j 2 pi/nlon this is the transform; with
// s_j = 1 it is, up to a factor, the synthesis's adjoint.
sp_down :: (g:SpGrid,deg:int,f:[]f64,ar:[]f64,ai:[]f64,weighted:bool) {
    for t:0..<sp_coeffs(deg) {ar[t]=0;ai[t]=0;}
    re:[SP_LON_MAX]f64;im:[SP_LON_MAX]f64;col:[SP_DEG_MAX]f64;n:=g.nlon;
    for j:0..<g.nlat {
        for k:0..<n {re[k]=f[j*n+k];im[k]=0;}
        sf_forward(re[0..<n],im[0..<n]);
        s:=1.0;if weighted {s=g.w[j]*2*SP_PI/f64(n);}
        for m:0..<deg {
            sp_legendre(deg,m,g.x[j],col[..]);
            for l:m..<deg {t:=sp_index(l,m);ar[t]+=s*col[l]*re[m];ai[t]+=s*col[l]*im[m];}
        }
    }
}
// Up: f_jk = sum over l, m of c_m P_lm(x_j) Re(a_lm e^(i m phi_k)), c_0 = 1
// and c_m = 2 for the orders m and -m together. Bins m and nlon - m hold the
// conjugate of G_j(m) = sum over l of a_lm P_lm(x_j), so the forward FFT's
// real part is the sum; 2 deg - 1 <= nlon keeps the bins apart. a_l0's
// imaginary part is not read.
SpSum :: struct {re:f64;im:f64;}
// G_j(m) = sum over l of a_lm P_lm(x_j).
sp_column :: (deg:int,m:int,x:f64,ar:[]f64,ai:[]f64)->SpSum {
    col:[SP_DEG_MAX]f64;sp_legendre(deg,m,x,col[..]);s:SpSum;
    for l:m..<deg {t:=sp_index(l,m);s.re+=col[l]*ar[t];s.im+=col[l]*ai[t];}
    return s;
}
sp_up :: (g:SpGrid,deg:int,ar:[]f64,ai:[]f64,f:[]f64) {
    n:=g.nlon;
    for j:0..<g.nlat {
        re:[SP_LON_MAX]f64;im:[SP_LON_MAX]f64;   // zero outside the bins written
        re[0]=sp_column(deg,0,g.x[j],ar,ai).re;
        for m:1..<deg {
            s:=sp_column(deg,m,g.x[j],ar,ai);
            re[m]=s.re;im[m]= -s.im;re[n-m]=s.re;im[n-m]=s.im;
        }
        sf_forward(re[0..<n],im[0..<n]);
        for k:0..<n {f[j*n+k]=re[k];}
    }
}
sp_values_ok :: (v:[]f64)->bool {return fo_values_ok(v);}
// The transform of a field on the grid: every coefficient below g.deg.
sp_analysis :: (g:SpGrid,f:[]f64,ar:[]f64,ai:[]f64)->SpStatus {
    if g.status!=SpStatus.ok {return g.status;}
    if f.count!=g.nlat*g.nlon {return SpStatus.shape;}
    if ar.count<sp_coeffs(g.deg) || ai.count<sp_coeffs(g.deg) {return SpStatus.capacity;}
    if !sp_values_ok(f) {return SpStatus.number;}
    sp_down(g,g.deg,f,ar,ai,true);
    return SpStatus.ok;
}
// The field on the grid from its coefficients.
sp_synthesis :: (g:SpGrid,ar:[]f64,ai:[]f64,f:[]f64)->SpStatus {
    if g.status!=SpStatus.ok {return g.status;}
    n:=sp_coeffs(g.deg);
    if ar.count!=n || ai.count!=n {return SpStatus.shape;}
    if f.count<g.nlat*g.nlon {return SpStatus.capacity;}
    if !sp_values_ok(ar) || !sp_values_ok(ai) {return SpStatus.number;}
    sp_up(g,g.deg,ar,ai,f);
    return SpStatus.ok;
}
```

The synthesis puts G_j(m) = Σ_l a_lm P_lm(x_j) into FFT bins m and nlon − m,
conjugated, so the forward FFT's real part is Σ_m c_m Re(G_j(m) e^(imφ_k)),
with c_0 = 1 and c_m = 2 for each pair of orders ±m. The condition
2L − 1 ≤ nlon is what keeps bins m and nlon − m apart.

The same `sp_down` with every latitude weight set to 1 is, up to the factor
c_m, the adjoint of the synthesis. The layer's backward pass uses it.

## The data: a reaction-diffusion equation

The training data is the solution of

∂u/∂t = κ ∇²u + r (u − u³)

on the unit sphere, the Allen-Cahn equation: diffusion smooths u, and the
reaction pushes each point towards −1 or 1, so smooth initial fields
sharpen into regions of either phase separated by thin fronts. It stands in
for the reanalysis data the full model learns from. Two properties make it a
fair test of a spherical model: it has nothing special at the poles (it
commutes with every rotation of the sphere), and its one-step map is
nonlinear.

The Laplacian is diagonal in the harmonics: ∇²Y_lm = −l(l + 1) Y_lm. So the
diffusion alone multiplies a_lm by exp(−κ l(l + 1) t), exactly. The reaction
alone, at a constant field, has the closed form
u(t) = u₀ / √(u₀² + (1 − u₀²) e^(−2rt)).

<!-- source: psionic/lib/sphere.coax flow -->
```jai
// Diffusion at most 1, rate at most 100, a step in (0, 100], 1 to 10,000
// substeps; NaN fails every comparison.
sp_flow_ok :: (f:SpFlow)->bool {
    return f.kappa>=0 && f.kappa<=1 && f.rate>=0 && f.rate<=100 && f.dt>0 && f.dt<=100 && f.substeps>=1 && f.substeps<=SP_SUBSTEPS_MAX;
}
// The equation is solved for the coefficients below g.deg (a Galerkin
// system): the diffusion multiplies degree l by exp(-kappa l (l + 1) t)
// exactly, and the reaction's coefficients N(a) come from rate (f - f^3) on
// the grid, f the synthesis of a, transformed back.
sp_diffuse :: (g:SpGrid,kappa:f64,t:f64,ar:[]f64,ai:[]f64) {
    for l:1..<g.deg {   // degree 0 does not decay
        d:=exp(-kappa*f64(l*(l+1))*t);
        for m:0..<l+1 {ar[sp_index(l,m)]*=d;ai[sp_index(l,m)]*=d;}
    }
}
sp_reaction :: (g:SpGrid,rate:f64,ar:[]f64,ai:[]f64,nr:[]f64,ni:[]f64) {
    f:[SP_CELLS_MAX]f64;cells:=g.nlat*g.nlon;
    sp_up(g,g.deg,ar,ai,f[0..<cells]);
    for t:0..<cells {v:=f[t];f[t]=rate*(v-v*v*v);}
    sp_down(g,g.deg,f[0..<cells],nr,ni,true);
}
// One Runge-Kutta stage: the reaction's coefficients at a + c k into out.
// The first stage has c = 0.
sp_stage :: (g:SpGrid,rate:f64,n:int,ar:[]f64,ai:[]f64,c:f64,kr:[]f64,ki:[]f64,outr:[]f64,outi:[]f64) {
    br:[SP_COEF_MAX]f64;bi:[SP_COEF_MAX]f64;
    for t:0..<n {br[t]=ar[t]+c*kr[t];bi[t]=ai[t]+c*ki[t];}
    sp_reaction(g,rate,br[0..<n],bi[0..<n],outr,outi);
}
// One step of dt in substeps Strang steps of h: diffusion for h/2, the
// reaction for h by the classical fourth-order Runge-Kutta method, diffusion
// for h/2. The output is the synthesis of the final coefficients.
sp_step :: (g:SpGrid,flow:SpFlow,u:[]f64,next:[]f64)->SpStatus {
    if g.status!=SpStatus.ok {return g.status;}
    cells:=g.nlat*g.nlon;
    if u.count!=cells {return SpStatus.shape;}
    if next.count<cells {return SpStatus.capacity;}
    if !sp_flow_ok(flow) {return SpStatus.flow;}
    if !sp_values_ok(u) {return SpStatus.number;}
    h:=flow.dt/f64(flow.substeps);n:=sp_coeffs(g.deg);
    ar:[SP_COEF_MAX]f64;ai:[SP_COEF_MAX]f64;
    // The four stages' coefficients, k1 to k4.
    ar1:[SP_COEF_MAX]f64;ai1:[SP_COEF_MAX]f64;ar2:[SP_COEF_MAX]f64;ai2:[SP_COEF_MAX]f64;
    ar3:[SP_COEF_MAX]f64;ai3:[SP_COEF_MAX]f64;ar4:[SP_COEF_MAX]f64;ai4:[SP_COEF_MAX]f64;
    sp_down(g,g.deg,u,ar[0..<n],ai[0..<n],true);
    for s:0..<flow.substeps {
        sp_diffuse(g,flow.kappa,h/2,ar[..],ai[..]);
        // Stage q reads a + c h k_(q-1): c = 0, 1/2, 1/2, 1.
        sp_stage(g,flow.rate,n,ar[..],ai[..],0,ar[..],ai[..],ar1[..],ai1[..]);
        sp_stage(g,flow.rate,n,ar[..],ai[..],h/2,ar1[..],ai1[..],ar2[..],ai2[..]);
        sp_stage(g,flow.rate,n,ar[..],ai[..],h/2,ar2[..],ai2[..],ar3[..],ai3[..]);
        sp_stage(g,flow.rate,n,ar[..],ai[..],h,ar3[..],ai3[..],ar4[..],ai4[..]);
        for t:0..<n {
            ar[t]+=h/6*(ar1[t]+2*ar2[t]+2*ar3[t]+ar4[t]);
            ai[t]+=h/6*(ai1[t]+2*ai2[t]+2*ai3[t]+ai4[t]);
        }
        sp_diffuse(g,flow.kappa,h/2,ar[..],ai[..]);
    }
    sp_up(g,g.deg,ar[0..<n],ai[0..<n],next[0..<cells]);
    return SpStatus.ok;
}
```

The solver works on the coefficients below the grid's degree L: each
substep of length h is diffusion for h/2, the reaction for h by the classical
fourth-order Runge-Kutta method (each stage: synthesis, the cubic at each
grid point, analysis), and diffusion for another h/2. This is Strang
splitting, second order in h. The checks confirm each part independently:

- with r = 0, every coefficient of a random band-limited field decays by
  exactly exp(−κ l(l + 1) t);
- with a constant field, the reaction matches the closed form, and its
  error falls 16 times each time h halves (fourth order); −1, 0 and 1 stay
  where they are;
- on a seeded field, the error against 128 substeps falls 4 times each
  time h halves (second order). The transcript's fifth line shows the same
  on the forecasting grid: 2.06 × 10⁻⁴, 5.10 × 10⁻⁵ and 1.27 × 10⁻⁵ with 2,
  4 and 8 substeps, ratios 4.04 and 4.02.

The data uses κ = 0.02, r = 2 and frames 0.25 apart, each made with 16
substeps. Initial fields come from the counter generator: below degree 6,
the real and imaginary parts of a_lm are uniform in [−0.6, 0.6) divided by
l + 1.

<!-- source: psionic/lib/sphere.coax seeded -->
```jai
SP_INIT_DEG :: 6;
SP_INIT_AMP :: 0.6;
// A seeded smooth field: below degree 6, a_lm's real and imaginary parts are
// uniform in [-0.6, 0.6) divided by l + 1 (a_l0 is real), drawn from the
// counter generator at 2 (l (l + 1)/2 + m) and the next counter.
sp_initial :: (g:SpGrid,seed:u64,u:[]f64)->SpStatus {
    if g.status!=SpStatus.ok {return g.status;}
    if g.deg<SP_INIT_DEG {return SpStatus.degree;}
    if u.count!=g.nlat*g.nlon {return SpStatus.shape;}
    ar:[SP_COEF_MAX]f64;ai:[SP_COEF_MAX]f64;
    for l:0..<SP_INIT_DEG {
        for m:0..<l+1 {t:=sp_index(l,m);ar[t]=SP_INIT_AMP*(2*cr_draw(CrSeed.{seed=seed,counter=cast(u64)(2*t)}).value-1)/f64(l+1);}
        for k:0..<l {t:=sp_index(l,k+1);ai[t]=SP_INIT_AMP*(2*cr_draw(CrSeed.{seed=seed,counter=cast(u64)(2*t+1)}).value-1)/f64(l+1);}
    }
    sp_up(g,g.deg,ar[..],ai[..],u);
    return SpStatus.ok;
}
// A trajectory: the initial field for seed, then steps steps of flow, each
// frame at s cells in out.
sp_trajectory :: (g:SpGrid,flow:SpFlow,seed:u64,out:[]f64)->SpStatus {
    cells:=g.nlat*g.nlon;
    if out.count<cells || out.count%cells!=0 {return SpStatus.shape;}
    status:=sp_initial(g,seed,out[0..<cells]);
    for s:1..<out.count/cells {
        if status==SpStatus.ok {status=sp_step(g,flow,out[(s-1)*cells..<s*cells],out[s*cells..<(s+1)*cells]);}
    }
    return status;
}
```

A trajectory is the initial field and four steps. Training pairs are every
consecutive pair of frames; held-out trajectories start from seeds the
training never saw (3000 and up, against 1000 and up).

## The spherical Fourier layer

A Fourier layer (chapter 156) transforms its input, multiplies the kept
modes by learned weights, transforms back and adds a pointwise path. On the
sphere the modes are the a_lm, and the question is which weights to allow.
A rotation of the sphere mixes the coefficients of one degree among
themselves (all orders m of degree l) but never mixes degrees. So
multiplying every order of degree l by the same weight w_l commutes with
every rotation, while a separate weight for each order does not. This is the
spherical convolution theorem: convolving with a kernel that is symmetric
about the pole multiplies each degree by one number.

<!-- source: psionic/lib/sphere.coax layer -->
```jai
// The layer and its input: grid, channels, degrees, lengths, then values.
sp_layer_check :: (l:SpLayer,g:SpGrid,x:[]f64)->SpStatus {
    if g.status!=SpStatus.ok {return g.status;}
    if l.cin<1 || l.cout<1 || l.cin>SP_WIDTH_MAX || l.cout>SP_WIDTH_MAX {return SpStatus.channels;}
    if l.deg<1 || l.deg>g.deg {return SpStatus.degree;}
    if l.w.count!=l.cout*l.cin*l.deg || l.lin.count!=l.cout*l.cin || l.bias.count!=l.cout || x.count!=l.cin*g.nlat*g.nlon {return SpStatus.shape;}
    if !sp_values_ok(l.w) || !sp_values_ok(l.lin) || !sp_values_ok(l.bias) || !sp_values_ok(x) {return SpStatus.number;}
    return SpStatus.ok;
}
// y_o = act(synthesis(b_o) + sum over i of lin_oi x_i + bias_o), where
// b_o,lm = sum over i of w_oil a_i,lm and a_i is the transform of x_i below
// degree deg. One weight serves every order m of a degree, so the layer
// commutes with rotations of the sphere.
sp_layer_forward :: (l:SpLayer,g:SpGrid,x:[]f64,pre:[]f64,y:[]f64)->SpStatus {
    status:=sp_layer_check(l,g,x);if status!=SpStatus.ok {return status;}
    cells:=g.nlat*g.nlon;n:=sp_coeffs(l.deg);
    if pre.count<l.cout*cells || y.count<l.cout*cells {return SpStatus.capacity;}
    ar:[SP_WIDTH_MAX*SP_COEF_MAX]f64;ai:[SP_WIDTH_MAX*SP_COEF_MAX]f64;
    for i:0..<l.cin {sp_down(g,l.deg,x[i*cells..<(i+1)*cells],ar[i*n..<ar.count],ai[i*n..<ai.count],true);}
    for o:0..<l.cout {
        br:[SP_COEF_MAX]f64;bi:[SP_COEF_MAX]f64;s:[SP_CELLS_MAX]f64;
        for r:0..<l.deg {d:=l.deg-1-r;for m:0..<d+1 {   // highest degree first
            t:=sp_index(d,m);zr:=0.0;zi:=0.0;
            for i:0..<l.cin {w:=l.w[(o*l.cin+i)*l.deg+d];zr+=w*ar[i*n+t];zi+=w*ai[i*n+t];}
            br[t]=zr;bi[t]=zi;
        }}
        sp_up(g,l.deg,br[0..<n],bi[0..<n],s[0..<cells]);
        for t:0..<cells {
            v:=s[t]+l.bias[o];
            for i:0..<l.cin {v+=l.lin[o*l.cin+i]*x[i*cells+t];}
            pre[o*cells+t]=v;y[o*cells+t]=fo_act(l.act,v);
        }
    }
    return SpStatus.ok;
}
```

**One channel by hand.** On the 2 × 3 grid, keep degrees 0 and 1 with
weights 0.5 and 3, lin 0.25, bias −1 and no activation. Take
f = 1 + cos θ: degree 0 holds the 1 and degree 1 the cos θ. The spectral path
gives 0.5 · 1 + 3 cos θ, so

y = 0.5 + 3 cos θ + 0.25 (1 + cos θ) − 1

at each grid point, which a `#test` checks. Keeping only degree 0 drops the
3 cos θ term.

**Rotation equivariance, checked.** The checks build a random band-limited
input below degree 5 on two channels, rotate it by R = R_z(α) R_y(β) (by
evaluating the coefficient sum at R⁻¹p for each grid point p, with the
explicit Legendre polynomial), and run the layer. Its output must equal the
original output at R⁻¹p, computed point by point from b_lm = Σ_i w_oil a_ilm
and the pointwise path. It does, within 10⁻¹², for four rotations, with
and without GELU. A second check rotates z by 45 degrees: the per-degree layer
returns exactly twice the rotated field, a layer that scales order 0 by 2
and order 1 by 1.5 misses by more than 0.3, and chapter 156's planar layer
on the same grid misses by more than 0.1.

### The backward pass

<!-- source: psionic/lib/sphere.coax adjoint -->
```jai
// Given dL/dy, write dL/dx and every parameter's gradient (overwritten).
// With g_o = dL/dy_o act'(pre_o), let h_o = sp_down(g_o) unweighted: the
// synthesis's adjoint is c_m h_o (c_0 = 1, c_m = 2) and the transform's
// adjoint is (2 pi/nlon) w_j times the synthesis of a coefficient divided by
// c_m. So dL/dw_oil = sum over m of c_m Re(conj(h_o,lm) a_i,lm), and
// dL/dx_i = (2 pi/nlon) w_j synthesis(sum over o of w_oil h_o) plus the
// pointwise path.
sp_layer_backward :: (l:SpLayer,g:SpGrid,x:[]f64,pre:[]f64,dy:[]f64,dx:[]f64,gr:SpGrad)->SpStatus {
    status:=sp_layer_check(l,g,x);if status!=SpStatus.ok {return status;}
    cells:=g.nlat*g.nlon;n:=sp_coeffs(l.deg);nc:=l.cout*cells;
    if pre.count!=nc || dy.count!=nc {return SpStatus.shape;}
    if !sp_values_ok(pre) || !sp_values_ok(dy) {return SpStatus.number;}
    if dx.count<l.cin*cells || gr.w.count<l.w.count || gr.lin.count<l.cout*l.cin || gr.bias.count<l.cout {return SpStatus.capacity;}
    ar:[SP_WIDTH_MAX*SP_COEF_MAX]f64;ai:[SP_WIDTH_MAX*SP_COEF_MAX]f64;
    hr:[SP_WIDTH_MAX*SP_COEF_MAX]f64;hi:[SP_WIDTH_MAX*SP_COEF_MAX]f64;gz:[SP_WIDTH_MAX*SP_CELLS_MAX]f64;
    for i:0..<l.cin {sp_down(g,l.deg,x[i*cells..<(i+1)*cells],ar[i*n..<ar.count],ai[i*n..<ai.count],true);}
    for o:0..<l.cout {
        sum:=0.0;
        for t:0..<cells {v:=dy[o*cells+t]*fo_slope(l.act,pre[o*cells+t]);gz[o*cells+t]=v;sum+=v;}
        gr.bias[o]=sum;
        for i:0..<l.cin {s:=0.0;for t:0..<cells {s+=gz[o*cells+t]*x[i*cells+t];}gr.lin[o*l.cin+i]=s;}
        sp_down(g,l.deg,gz[o*cells..<gz.count],hr[o*n..<hr.count],hi[o*n..<hi.count],false);
    }
    for i:0..<l.cin {
        dr:[SP_COEF_MAX]f64;di:[SP_COEF_MAX]f64;s:[SP_CELLS_MAX]f64;
        for o:0..<l.cout {for d:0..<l.deg {
            q:=0.0;
            for m:0..<d+1 {t:=sp_index(d,m);c:=2.0;if m==0 {c=1;}q+=c*(hr[o*n+t]*ar[i*n+t]+hi[o*n+t]*ai[i*n+t]);}
            gr.w[(o*l.cin+i)*l.deg+d]=q;
        }}
        for r:0..<l.deg {d:=l.deg-1-r;for m:0..<d+1 {   // highest degree first
            t:=sp_index(d,m);zr:=0.0;zi:=0.0;
            for o:0..<l.cout {w:=l.w[(o*l.cin+i)*l.deg+d];zr+=w*hr[o*n+t];zi+=w*hi[o*n+t];}
            dr[t]=zr;di[t]=zi;
        }}
        sp_up(g,l.deg,dr[0..<n],di[0..<n],s[0..<cells]);
        for j:0..<g.nlat {for k:0..<g.nlon {
            t:=j*g.nlon+k;v:=s[t]*g.w[j]*2*SP_PI/f64(g.nlon);
            for o:0..<l.cout {v+=l.lin[o*l.cin+i]*gz[o*cells+t];}
            dx[i*cells+t]=v;
        }}
    }
    return SpStatus.ok;
}
```

The adjoints follow from writing the transform and the synthesis as real
linear maps of the grid values and the (re, im) coefficient pairs. The
synthesis's adjoint is the unweighted `sp_down` times c_m; the transform's
adjoint is the synthesis of a coefficient divided by c_m, times
(2π/nlon) w_j at each latitude. In the gradient of an input the two factors
c_m cancel. Every gradient, of every weight and every input value, is
checked against central differences on three layers of different shapes.

## The forecaster

The network reads one field u and predicts the next frame as u + out:

- a lift from 1 to 4 channels, h = A u + a at each point;
- three layers, each spherical (degrees below 16, one weight per degree and
  channel pair) or planar (chapter 156's layer keeping 8 modes a direction,
  a complex weight per mode and channel pair), all with GELU;
- a projection from 4 channels to 1, out = q · h + q₀.

The two networks differ only in the layers. The spherical one has 841
parameters, the planar one 21,673: the planar layer's 225 complex modes per
channel pair against the spherical layer's 16 degrees.

<!-- source: psionic/lib/sphere.coax forward -->
```jai
sp_block :: (ws:[]f64,k:int,b:int)->[]f64 {return ws[k*b..<k*b+b];}
// The network, the grid it must fit, the lengths, then every value.
sp_admit :: (net:SpNet,p:[]f64,g:SpGrid,u:[]f64,ws:[]f64)->SpStatus {
    if g.status!=SpStatus.ok {return g.status;}
    if !sp_net_ok(net,g) {return SpStatus.network;}
    cells:=g.nlat*g.nlon;
    if p.count!=sp_params(net) || u.count!=cells || ws.count<sp_work(net,cells) {return SpStatus.shape;}
    if !sp_values_ok(p) || !sp_values_ok(u) {return SpStatus.number;}
    return SpStatus.ok;
}
// The network's output at every grid point, written to ws at sp_out.
sp_forward :: (net:SpNet,p:[]f64,g:SpGrid,u:[]f64,ws:[]f64)->SpStatus {
    status:=sp_admit(net,p,g,u,ws);if status!=SpStatus.ok {return status;}
    w:=net.width;L:=net.layers;cells:=g.nlat*g.nlon;wc:=w*cells;
    for c:0..<w {for t:0..<cells {ws[c*cells+t]=p[c]*u[t]+p[w+c];}}
    for l:0..<L {
        h:=sp_block(ws,l,wc);pre:=sp_block(ws,L+1+l,wc);y:=sp_block(ws,l+1,wc);
        if net.kind==SpKind.sphere {
            if sp_layer_forward(sp_sphere_layer(net,p,l),g,h,pre,y)!=SpStatus.ok {return SpStatus.number;}
        } else {
            if fo_forward(sp_plane_layer(net,p,l),g.nlat,g.nlon,h,pre,y)!=FoStatus.ok {return SpStatus.number;}
        }
    }
    proj:=sp_proj(net);out:=sp_out(net,cells);
    for t:0..<cells {v:=p[proj+w];for c:0..<w {v+=p[proj+c]*ws[L*wc+c*cells+t];}ws[out+t]=v;}
    return SpStatus.ok;
}
// One forecast step: u_next = u + out.
sp_predict :: (net:SpNet,p:[]f64,g:SpGrid,u:[]f64,next:[]f64,ws:[]f64)->SpStatus {
    status:=sp_forward(net,p,g,u,ws);if status!=SpStatus.ok {return status;}
    cells:=g.nlat*g.nlon;if next.count<cells {return SpStatus.capacity;}
    out:=sp_out(net,cells);
    for t:0..<cells {next[t]=u[t]+ws[out+t];}
    return SpStatus.ok;
}
```

The whole gradient is written by hand, as in chapter 157, and checked
against central differences for both kinds of layer.

**Area weights.** Grid cells near the poles are small: a row at colatitude
θ_j covers the fraction w_j/(2 nlon) of the sphere per point, and on the
16 × 32 grid a point in the polar rows weighs about a seventh of one at
the equator. The
loss and every error are weighted by that area, so the poles count for what
they cover, not for how many grid points crowd there.

<!-- source: psionic/lib/sphere.coax metrics -->
```jai
// The area-weighted relative error sqrt(sum a_j (x - y)^2 / sum a_j y^2).
sp_rel_error :: (g:SpGrid,x:[]f64,y:[]f64)->f64 {
    e:=0.0;s:=0.0;
    for j:0..<g.nlat {for k:0..<g.nlon {t:=j*g.nlon+k;d:=x[t]-y[t];e+=sp_area(g,j)*d*d;s+=sp_area(g,j)*y[t]*y[t];}}
    return sqrt(e/s);
}
// The area-weighted root mean square of x - y over the polar rows
// (|x_j| >= cap, that is, latitude at least asin(cap)) or the others.
sp_band_rms :: (g:SpGrid,x:[]f64,y:[]f64,cap:f64,polar:bool)->f64 {
    e:=0.0;s:=0.0;
    for j:0..<g.nlat {
        if (abs(g.x[j])>=cap)==polar {
            for k:0..<g.nlon {t:=j*g.nlon+k;d:=x[t]-y[t];e+=sp_area(g,j)*d*d;s+=sp_area(g,j);}
        }
    }
    if s==0 {return 0;}
    return sqrt(e/s);
}
```

Training is the same minibatch AdamW loop as chapter 157, with the
learning rate halving after each quarter of the epochs; a check replays it
step by step with `aw_step` and requires the same parameters bit for bit.

## Rollouts and lead times

A forecast for four steps ahead feeds the forecast for one step back in,
three times:

<!-- source: psionic/lib/sphere.coax rollout -->
```jai
// Forecasts fed back as inputs: frame s of frames is the forecast s + 1
// steps after u.
sp_rollout :: (net:SpNet,p:[]f64,g:SpGrid,u:[]f64,frames:[]f64)->SpStatus {
    cells:=g.nlat*g.nlon;
    if frames.count<cells || frames.count%cells!=0 {return SpStatus.shape;}
    ws:[(2*SP_LAYERS_MAX+3)*SP_WIDTH_MAX*SP_CELLS_MAX+SP_CELLS_MAX]f64;
    status:=sp_predict(net,p,g,u,frames[0..<cells],ws[..]);
    for s:1..<frames.count/cells {
        if status==SpStatus.ok {status=sp_predict(net,p,g,frames[(s-1)*cells..<s*cells],frames[s*cells..<(s+1)*cells],ws[..]);}
    }
    return status;
}
```

Errors grow with lead time, because each step starts from the previous
step's error. The transcript reports, for each lead, the mean over held-out
trajectories of the area-weighted relative error
√(Σ a_j (x − y)² / Σ a_j y²), and at lead 4 the area-weighted root mean square
error in the polar caps (latitude above 60 degrees, |x_j| ≥ sin 60°) and
below them. Persistence, the forecast that nothing changes, is the floor any
forecaster must beat: its error is 0.31 one step ahead and 0.69 four steps
ahead.

## A longer run

The acceptance threshold was fixed before the held-out seeds were used: on
the 48 held-out trajectories, the **spherical forecaster's mean
area-weighted relative error at lead 4 must be at most 0.10**. Short runs on
validation seeds (2000 and up, not used here) reached about 0.03, so 0.10
leaves room for a different set of fields while still requiring the network
to have learned the dynamics: persistence is at 0.69.

The run trains both networks on 96 trajectories (384 pairs) for 200 epochs
(9,600 AdamW steps each) in a release build. Its program is
[`sphere-long.coax`](../tests/sphere-long.coax), and its output is kept in
[`sphere-long.out`](../fixtures/sphere-long.out):

```shell
coax/bin/coax run psionic/tests/sphere-long.coax --release
```

```text
data: 16 x 32 Gauss grid, kappa 0.02, rate 2, steps of 0.25 in 16 substeps
training: seeds 1000 to 1095, 384 pairs, 200 epochs, batch 8, rate 0.01 halved each quarter
sphere network: width 4, 3 layers, 16 modes; 841 parameters
  epoch 0: loss 0.00511
  epoch 50: loss 0.00001152
  epoch 100: loss 6.818e-06
  epoch 150: loss 3.566e-06
  epoch 199: loss 3.434e-06
  held-out (seeds 3000 to 3047) relative error by lead, mean: 0.0027 0.004 0.0048 0.0062
  worst: 0.0037 0.0058 0.0071 0.0092
  lead 4 rms error: polar caps (above 60 degrees) 0.0041, below 60 degrees 0.0041
plane network: width 4, 3 layers, 8 modes; 21673 parameters
  epoch 0: loss 0.004191
  epoch 50: loss 0.00007924
  epoch 100: loss 0.0000611
  epoch 150: loss 0.00004904
  epoch 199: loss 0.00004581
  held-out (seeds 3000 to 3047) relative error by lead, mean: 0.017 0.0318 0.0459 0.0611
  worst: 0.0251 0.0475 0.0667 0.0883
  lead 4 rms error: polar caps (above 60 degrees) 0.0835, below 60 degrees 0.0317
persistence by lead: 0.3096 0.5005 0.6175 0.6894
threshold: spherical mean at lead 4 at most 0.1: pass
```

The spherical forecaster's mean held-out error at lead 4 is 0.0062, so the
run passes the threshold, and its worst trajectory, at 0.0092, is also inside
it. Its errors grow slowly with lead time (0.0027, 0.0040, 0.0048 and 0.0062), and persistence is about a
hundred times worse at every lead.

**The planar baseline.** With 26 times as many parameters, the planar
forecaster ends at 0.0611 after four steps, about ten times the spherical
one, and its error is not spread evenly. Its root mean square error in the
polar caps is 0.0835, against 0.0317 below 60 degrees; the spherical
forecaster's is 0.0041 and 0.0041, the same in both bands. The planar layer
treats the latitude-longitude grid as a flat, periodic image: it does not
know that a whole row near a pole is a small circle, or that the top row's
neighbour across the pole is the same row half a turn round, and it wraps
the north pole onto the south pole. Its kept modes can describe smooth
structure in grid coordinates, which near the poles is not smooth structure
on the sphere. The spherical layer's degrees are the sphere's own, so there
is nothing special for it to get wrong at the poles. These are pole
artifacts, and the area-weighted band errors are what make them visible:
an unweighted mean over grid points would count the crowded polar rows
several times over.

Both results are on synthetic dynamics that are exactly rotation-invariant,
which favours the spherical layer by construction. Real atmospheric fields
are not: the Earth rotates, has land and sea, and has a different climate
at each latitude. FourCastNet3 adds position-dependent parts for that, and
none of them are here.

## Refusals

The grid refuses 0 or more than 32 latitudes and 0 or more than 64
longitudes with `size`, and a degree count that is 0, above the latitude
count or too large for the longitudes (2L − 1 > nlon) with `degree`. The
transform checks the grid, the field's length (`shape`), the coefficient
arrays' room (`capacity`) and every value (finite and at most 10⁶ in
magnitude, `number`). The solver refuses diffusion outside [0, 1], a rate
outside [0, 100], a step outside (0, 100] and substeps outside 1 to 10,000
with `flow`, NaN included. The layer and the network refuse channel counts,
degree counts, parameter lengths and values the same way, and nothing is
written before the checks pass. The transcript's last five lines show five
of these: 33 latitudes, 16 degrees on 30 longitudes (31 bins needed), a
negative diffusion, a NaN in the field and a network keeping 17 degrees on a
grid that holds 16.

## How it is checked

The checks in [`sphere.coax`](../lib/sphere.coax) come before the code they
test. The oracles in [`sphere-checks.coax`](../tests/sphere-checks.coax) do
not call the library's recurrence or transforms:

- **Hand values**: the Gauss nodes and weights for 1, 2 and 3 latitudes,
  the degree-1 transforms above, the Legendre functions of degrees 0 to 2 in
  closed form, and the transforms of 1 and cos² θ.
- **Gauss quadrature** integrates every power below 2n exactly for 1 to 12
  nodes.
- **The explicit Legendre polynomial** against the recurrence for degrees
  below 12.
- **Orthonormality** of the discrete harmonics by direct quadrature with the
  explicit polynomial, and **round trips** of random band-limited fields on
  six grids, including longitudes that are not powers of two (chapter 151's
  chirp transform).
- **The direct double sum** over every grid point against the FFT-based
  transform, for any field, and the point-by-point synthesis.
- **The solver**: exact diffusion, the reaction's closed form at fourth
  order, and Strang splitting at second order.
- **The layer and the network**: hand values, central differences for every
  gradient, rotation equivariance, the training replay and the rollout.

`coax mutate psionic/lib/sphere.coax --root-only` changes the library's own
code one site at a time and reruns the checks. Of 1,044 mutants, all 1,044 are caught (run in two shards). The oracles and
drivers live in the checks file, so they are not mutated themselves.

The library is not an `#authority` file: it decides no money, admission or
policy.

## What this does not do

- It does not run FourCastNet3. There is no pinned revision, no checkpoint,
  no step compared with a recorded reference output, and no ERA5 data; those
  remain, and ERA5's terms need their own qualification before any data is
  used.
- The grid is 16 × 32 with one variable, not 721 × 1440 with 72. The
  Legendre functions are recomputed for every transform rather than stored,
  which is cheap at this size; at full size the transform's cost and the
  activation and memory budgets have to be stated before running, and are
  not stated here.
- FourCastNet3's spherical layers are wider and add more than one weight per
  degree (its local and attention-like parts are not reproduced). The layer
  here is the plain per-degree convolution.
- Training runs on one CPU core in binary64. There is no GPU path, no mixed
  precision and no checkpointing.
- The data is synthetic. The forecaster is measured against the solver on
  trajectories from the same distribution it was trained on.

## Dependencies

- [Chapter 151](ch151-waveforms-and-spectral-features.md): the FFT, of any
  size.
- [Chapter 156](ch156-a-spectral-neural-operator.md): the planar Fourier
  layer, GELU and the comparison network's layers.
- [Chapter 157](ch157-learn-darcy-flow.md): the training pattern and the
  frozen held-out threshold.
- [Chapter 124](ch124-adamw-and-explicit-precision.md): AdamW.

## Sources

- [Chapter plan](PLAN.md), Part XVI.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), FourCastNet3 entry.
- Bonev et al., "Spherical Fourier Neural Operators: Learning Stable
  Dynamics on the Sphere" (2023), for the per-degree spherical convolution.
