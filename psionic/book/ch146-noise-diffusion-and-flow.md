# 146. Noise, diffusion and flow

Diffusion and flow-matching models generate data by learning to undo noise.
Training takes a data point, mixes it with Gaussian noise at some level, and
teaches a model to say what was mixed in. Sampling starts from pure noise and
uses the model to walk back, a level at a time, to something that looks like
the data.

This chapter builds every part of that on a target small enough to check by
hand: points in the plane drawn from a mixture of four Gaussians. For this
target the best possible model is known in closed form at every noise level,
so each piece can be compared with the right answer rather than with another
implementation. It builds, in
[`diffusion.coax`](../lib/diffusion.coax):

- **schedules**: linear and cosine noise schedules, and any list of betas,
  admitted only when it is in range and never decreasing;
- the **forward process**: the noisy point `x = alpha x0 + sigma eps`, with
  normal draws from [chapter 53](ch053-choosing-the-next-token.md)'s counter
  generator so a seed names every draw;
- the **exact posterior**: what a noisy point says about its data and its
  noise, for one class or the whole mixture;
- four **objectives**: predict the noise, the data, the diffusion velocity,
  or the flow velocity, and the exact conversions between them;
- a **model**: one affine predictor `a x + b` per noise level and class,
  trained by gradient descent and compared with least squares and with the
  population optimum;
- four **samplers**: DDPM (stochastic), DDIM (deterministic), and Euler and
  Heun on the flow-matching ODE;
- a **gate**, fixed before any sample was drawn, that compares sample
  statistics with the target.

The chapter teaches the methods. It does not qualify any later world-model
or robotics architecture: chapters 166 and 169 bring their own models, data
and checks.

## Run it

```shell
coax/bin/coax run psionic/examples/diffusion.coax
coax/bin/coax run psionic/examples/diffusion-refuse.coax
coax/bin/coax test psionic/lib/diffusion.coax
coax/bin/coax mutate psionic/lib/diffusion.coax --jobs 8
```

The [example](../examples/diffusion.coax) takes about 8 seconds of CPU time
in a debug build, most of it training. Its whole transcript is under [Run
the example](#run-the-example).

## The target

Each class `c` is a normal distribution in the plane with mean `mu_c` and
standard deviation 0.5 in each coordinate. The four means sit on the axes,
2 from the origin. The unconditional target is the mixture: pick a class
with probability 1/4, then a point from it.

<!-- source: psionic/lib/diffusion.coax target -->
```jai
DF_LEVELS_MAX :: 1001;
DF_STEPS_MAX :: 1000;
DF_CELL_MAX :: 4096;            // training points for one level and class
DF_SPREAD :: 0.5;               // each class: N(mean, 0.5^2 I)
DF_MEAN :: [8]f64.[2,0, 0,2, -2,0, 0,-2];
DF_STRIDE :: 2002;              // normals reserved for one sample
DF_PI :: 3.141592653589793;
DF_CLIP :: 0.999;               // the cosine schedule's largest beta
DF_OFFSET :: 0.008;             // the cosine schedule's offset s
```

The mixture's mean is 0. Each coordinate's variance is the variance of the
class means, (4 + 0 + 4 + 0) / 4 = 2, plus 0.5² = 0.25: 2.25. A class's
points sit 2√2 ≈ 2.83 from the neighbouring means, and about 1 in 200 of
them is nearer a neighbour's mean than its own.

The types every call returns:

<!-- source: psionic/lib/diffusion.coax types -->
```jai
DfStatus :: enum u8 {ok;steps;range;order;number;shape;class;objective;}
// What a model predicts from the noisy point x = alpha x0 + sigma eps: the
// noise eps, the data x0, the diffusion velocity alpha eps - sigma x0, or the
// flow velocity eps - x0.
DfObjective :: enum u8 {eps;x0;v;flow;}
DfSampler :: enum u8 {ddpm;ddim;euler;heun;}
// beta[t] and abar[t] for t = 1..steps; abar[0] = 1. abar[t] is the product
// of (1 - beta[s]) for s <= t.
DfSchedule :: struct {status:DfStatus;steps:int;beta:[DF_LEVELS_MAX]f64;abar:[DF_LEVELS_MAX]f64;}
// A model: noise level j has x = alpha[j] x0 + sigma[j] eps, and class c's
// predictor there is a x + b with (a, b0, b1) = w[12 j + 3 c ..]. levels = 0
// stands for the exact posterior of the target.
DfModel :: struct {status:DfStatus;objective:DfObjective;levels:int;alpha:[]f64;sigma:[]f64;w:[]f64;}
// What a predictor says about a noisy point: its noise, its data, and (for
// the exact posterior) the class it most likely came from, the first on ties.
DfPair :: struct {eps:[2]f64;x0:[2]f64;likely:int;}
DfSample :: struct {status:DfStatus;x:[2]f64;}
// Mean and per-coordinate variance of n points, how many are nearest each
// class mean, and the variance about the nearest class mean.
DfStats :: struct {status:DfStatus;n:int;mean:[2]f64;var:f64;count:[4]int;within:f64;}
```

## Normal draws

A standard normal comes from two uniforms by the Box-Muller transform:
`z = sqrt(-2 ln(1 - u1)) cos(2π u2)`. The uniforms are chapter 53's: the top
53 bits of `cr_bits(seed, counter)`, divided by 2⁵³. Normal `k` uses counters
`2k` and `2k + 1`, so it is a pure function of the seed and `k`, and any
part of a run can be redrawn without the rest.

### A draw by hand

For seed 0 and `k = 0`, the counter test of chapter 53 gives
`u1 = 0.8833108082136426` and `u2 = 0.43152799704850997`.

| Step | Value |
| --- | --- |
| `1 - u1` | 0.1166891917863574 |
| `sqrt(-2 ln(1 - u1))` | 2.0727958700018596 |
| `cos(2π u2)` | −0.9088730639697532 |
| `z` | −1.8839083333524405 |

The transcript's `normal draw, seed 0, k 0` line prints that value. `1 - u1`
is never 0, since `u1` is at most 1 − 2⁻⁵³, so the logarithm is always
finite.

<!-- source: psionic/lib/diffusion.coax draw -->
```jai
// A standard normal from two uniforms, counters 2k and 2k + 1 (Box-Muller,
// cosine branch). 1 - u is never 0, so the log is finite.
df_normal :: (seed:u64,k:u64)->f64 #export_c {
    u1:=f64(cr_bits(seed,2*k)>>11)/9007199254740992.0;u2:=f64(cr_bits(seed,2*k+1)>>11)/9007199254740992.0;
    return sqrt(-2*log(1-u1))*cos(2*DF_PI*u2);
}
// Data point i of a class: its mean plus 0.5 times normals 2i and 2i + 1.
df_data :: (seed:u64,i:u64,class:int)->[2]f64 {
    return [2]f64.[DF_MEAN[2*class]+DF_SPREAD*df_normal(seed,2*i),DF_MEAN[2*class+1]+DF_SPREAD*df_normal(seed,2*i+1)];
}
// The forward process at one noise level: x = alpha x0 + sigma eps.
df_noise :: (x0:[2]f64,eps:[2]f64,alpha:f64,sigma:f64)->[2]f64 {
    return [2]f64.[alpha*x0[0]+sigma*eps[0],alpha*x0[1]+sigma*eps[1]];
}
```

## Schedules

A diffusion schedule has `T` steps. Step `t` adds a little noise: it scales
the point by `sqrt(1 - beta_t)` and adds noise of variance `beta_t`. After
`t` steps the point is `sqrt(abar_t) x0 + sqrt(1 - abar_t) eps` with a single
standard normal `eps`, where `abar_t` is the product of `1 - beta_s` for
`s ≤ t`. So any level can be reached in one draw, which is what training
uses. Write `alpha = sqrt(abar_t)` and `sigma = sqrt(1 - abar_t)`; then
`alpha² + sigma² = 1` at every level.

### A linear schedule by hand

Four steps from `beta_1 = 0.1` to `beta_4 = 0.4` space the betas evenly:
0.1, 0.2, 0.3, 0.4.

| t | beta_t | 1 − beta_t | abar_t |
| --- | --- | --- | --- |
| 1 | 0.1 | 0.9 | 0.9 |
| 2 | 0.2 | 0.8 | 0.9 · 0.8 = 0.72 |
| 3 | 0.3 | 0.7 | 0.72 · 0.7 = 0.504 |
| 4 | 0.4 | 0.6 | 0.504 · 0.6 = 0.3024 |

### A cosine schedule by hand

The cosine schedule sets `abar_t = f(t) / f(0)` with
`f(t) = cos²((t/T + 0.008) / 1.008 · π/2)`, then reads each beta back as
`1 - abar_t / abar_{t-1}`. For `T = 4`, `f` is 0.99984459, 0.84688053,
0.49376684, 0.14424968 and 3.7 × 10⁻³³. The ratios give betas 0.152988,
0.416958 and 0.707859. The last ratio is almost 0, so the last beta would be
almost 1 and the last level would hold no data at all. The schedule caps
each beta at 0.999, which keeps `abar_4 = 0.144272 · 0.001 = 0.000144` above
zero. The cap is written as `(b + 0.999 − |b − 0.999|) / 2`, the smaller of
the two without a comparison, so it can be off from `b` by one rounding
(the checks allow 2 × 10⁻¹⁶). With 100 steps, the schedule the example
trains on, `abar_100` is 2.4 × 10⁻⁷: almost pure noise, but not quite.

<!-- source: psionic/lib/diffusion.coax schedule -->
```jai
df_finite :: (v:f64)->bool {return v-v==0;}
// Admit betas[0..T-1] as beta_1..beta_T: 1 to 1,000 of them, finite, in
// (0, 1) and never decreasing; abar is their running product of 1 - beta.
df_betas :: (betas:[]f64)->DfSchedule {
    s:DfSchedule;
    if betas.count<1 || betas.count>DF_STEPS_MAX {s.status=DfStatus.steps;return s;}
    for b:betas {if !df_finite(b) {s.status=DfStatus.number;return s;}}
    for b:betas {if b<=0 || b>=1 {s.status=DfStatus.range;return s;}}
    for t:1..<betas.count {if betas[t]<betas[t-1] {s.status=DfStatus.order;return s;}}
    s.steps=betas.count;s.abar[0]=1;
    for t:1..<betas.count+1 {s.beta[t]=betas[t-1];s.abar[t]=s.abar[t-1]*(1-betas[t-1]);}
    return s;
}
// Linear betas from first (beta_1) to last (beta_T).
df_linear :: (steps:int,first:f64,last:f64)->DfSchedule {
    if steps<1 || steps>DF_STEPS_MAX {return DfSchedule.{status=DfStatus.steps};}
    if !df_finite(first) || !df_finite(last) {return DfSchedule.{status=DfStatus.number};}
    if last<first {return DfSchedule.{status=DfStatus.order};}
    b:[DF_STEPS_MAX]f64;b[0]=first;
    for t:1..<steps {b[t]=first+(last-first)*f64(t)/f64(steps-1);}
    return df_betas(b[0..<steps]);
}
// The cosine schedule: abar(t) = f(t) / f(0) with f(t) = cos^2((t/T + s) /
// (1 + s) * pi/2), each beta capped at 0.999 so the last level stays above 0.
df_cosine :: (steps:int)->DfSchedule {
    if steps<1 || steps>DF_STEPS_MAX {return DfSchedule.{status=DfStatus.steps};}
    b:[DF_STEPS_MAX]f64;prev:=df_cosine_f(0,steps);
    for t:1..<steps+1 {now:=df_cosine_f(t,steps);b[t-1]=df_cap(1-now/prev);prev=now;}
    return df_betas(b[0..<steps]);
}
// The smaller of b and 0.999, written without a comparison: (b + c - |b - c|) / 2.
df_cap :: (b:f64)->f64 {return (b+DF_CLIP-abs(b-DF_CLIP))/2;}
df_cosine_f :: (t:int,steps:int)->f64 {c:=cos((f64(t)/f64(steps)+DF_OFFSET)/(1+DF_OFFSET)*DF_PI/2);return c*c;}
```

A list of betas is refused when it has no entries or more than 1,000
(`steps`), when an entry is not finite (`number`), when an entry is not
strictly between 0 and 1 (`range`), and when it ever decreases (`order`). A
linear schedule whose last beta is below its first is refused as `order`
even with one step.

## The forward process and the posterior

Noise a class-0 point `x0 = (2, 0)` with `eps = (1, −1)` at `alpha = 0.8`,
`sigma = 0.6` (`abar = 0.64`):

```text
x = 0.8 · (2, 0) + 0.6 · (1, −1) = (2.2, −0.6)
```

Over all draws, class 0 at this level is normal with mean
`alpha mu_0 = (1.6, 0)` and variance `v = alpha² · 0.25 + sigma² =
0.16 + 0.36 = 0.52` in each coordinate. The example draws 4,000 such points
and finds mean (1.5963, 0.0004) and variance 0.5213; the standard error of
the mean is √(0.52 / 4,000) ≈ 0.011.

The reverse question is the one a model must answer: given only `x`, what
were `x0` and `eps`? For one class, `x0`, `eps` and `x` are jointly normal,
so the answer is a regression:

```text
E[eps | x] = (sigma / v) (x − alpha mu)
E[x0 | x]  = mu + (alpha · 0.25 / v) (x − alpha mu)
```

### The posterior by hand

At `x = (2.2, −0.6)`, `x − alpha mu_0 = (0.6, −0.6)`.

| Quantity | Value |
| --- | --- |
| `sigma / v` | 0.6 / 0.52 = 1.153846 |
| `E[eps given x]` | 1.153846 · (0.6, −0.6) = (0.692308, −0.692308) |
| `alpha · 0.25 / v` | 0.2 / 0.52 = 0.384615 |
| `E[x0 given x]` | (2, 0) + 0.384615 · (0.6, −0.6) = (2.230769, −0.230769) |

The two guesses put the point back together exactly:
`0.8 · (2.230769, −0.230769) + 0.6 · (0.692308, −0.692308) = (2.2, −0.6)`.
For the mixture, each class's answer is weighted by how likely that class
makes `x`; class 0 dominates here, and the transcript's mixture line is
close to the class-0 line.

The predicted noise is also the score, the gradient of the log density of
`x`, times `−sigma`. The checks compare the two on 15 points at 15 noise
levels, for every class and the mixture, with the gradient taken by central
differences of a log density written separately.

<!-- source: psionic/lib/diffusion.coax posterior -->
```jai
// What a noisy point x = alpha x0 + sigma eps says about its data and its
// noise. Each class makes x normal with mean alpha mu and variance
// v = alpha^2 0.5^2 + sigma^2 a coordinate; given x and the class, x0 and eps
// are normal too, with means mu + (alpha 0.5^2 / v)(x - alpha mu) and
// (sigma / v)(x - alpha mu). Class -1 is the mixture: the classes weighted
// by how likely each makes x, computed against the largest weight so that
// no exponential overflows.
df_posterior :: (x:[2]f64,alpha:f64,sigma:f64,class:int)->DfPair {
    if class>=0 {return df_class_posterior(x,alpha,sigma,class);}
    v:=alpha*alpha*DF_SPREAD*DF_SPREAD+sigma*sigma;lw:[4]f64;
    for c:0..<4 {d0:=x[0]-alpha*DF_MEAN[2*c];d1:=x[1]-alpha*DF_MEAN[2*c+1];lw[c]=-(d0*d0+d1*d1)/(2*v);}
    best:=0;for k:0..<3 {if lw[k+1]>lw[best] {best=k+1;}}
    total:=0.0;for c:0..<4 {total+=exp(lw[c]-lw[best]);}
    p:=DfPair.{likely=best};
    for c:0..<4 {
        w:=exp(lw[c]-lw[best])/total;one:=df_class_posterior(x,alpha,sigma,c);
        for d:0..<2 {p.eps[d]+=w*one.eps[d];p.x0[d]+=w*one.x0[d];}
    }
    return p;
}
df_class_posterior :: (x:[2]f64,alpha:f64,sigma:f64,class:int)->DfPair {
    v:=alpha*alpha*DF_SPREAD*DF_SPREAD+sigma*sigma;p:=DfPair.{likely=class};
    for d:0..<2 {
        r:=x[d]-alpha*DF_MEAN[2*class+d];
        p.eps[d]=sigma*r/v;p.x0[d]=DF_MEAN[2*class+d]+alpha*DF_SPREAD*DF_SPREAD*r/v;
    }
    return p;
}
```

The weights are computed against the largest one, so a point far from every
class (the checks use (40, −30)) still gets weights that sum to 1. The class
with the largest weight is returned as `likely`, the first one on ties: at
the origin all four are equally likely, the answer is class 0, and the four
guesses cancel to zero.

## Four objectives

A model can be asked for any of four things, all fixed by `x0` and `eps`:

| Objective | Target |
| --- | --- |
| `eps` | the noise `eps` |
| `x0` | the data `x0` |
| `v` | the diffusion velocity `alpha eps − sigma x0` |
| `flow` | the flow velocity `eps − x0` |

Given `x`, any one of them determines the others, because
`x = alpha x0 + sigma eps`. So a prediction of one objective can be read as
a prediction of any other, and the four losses of one prediction differ by
a known factor at each level.

### One guess, four losses

Keep `x0 = (2, 0)`, `eps = (1, −1)`, `x = (2.2, −0.6)`, and guess the noise
as `(0.5, −0.5)`. It misses each coordinate by 0.5: the `eps` loss is
0.5² + 0.5² = 0.5.

| Read as | Prediction | Target | Loss | Factor |
| --- | --- | --- | --- | --- |
| `eps` | (0.5, −0.5) | (1, −1) | 0.5 | 1 |
| `x0` = (x − 0.6 guess) / 0.8 | (2.375, −0.375) | (2, 0) | 0.28125 | (sigma / alpha)² = 0.5625 |
| `v` = (guess − 0.6 x) / 0.8 | (−1.025, −0.175) | (−0.4, −0.8) | 0.78125 | 1 / alpha² = 1.5625 |
| `flow` = guess − x0 guess | (−1.875, −0.125) | (−1, −1) | 1.53125 | (1 + sigma / alpha)² = 3.0625 |

Every loss is the `eps` loss times its factor. That is the whole difference
between the objectives at a fixed level: the same predictor, a different
weight on its error. Across levels the weights differ, which is why the
choice matters for a network that shares parameters across levels. Here each
level has its own predictor, so every objective has the same minimizer, and
the checks confirm it on 4,096 points per objective.

<!-- source: psionic/lib/diffusion.coax objectives -->
```jai
// The training target of each objective, from the data and the noise.
df_mix :: (o:DfObjective,x0:f64,eps:f64,alpha:f64,sigma:f64)->f64 {
    if o==DfObjective.eps {return eps;}
    if o==DfObjective.x0 {return x0;}
    if o==DfObjective.v {return alpha*eps-sigma*x0;}
    return eps-x0;
}
df_target :: (o:DfObjective,x0:[2]f64,eps:[2]f64,alpha:f64,sigma:f64)->[2]f64 {
    return [2]f64.[df_mix(o,x0[0],eps[0],alpha,sigma),df_mix(o,x0[1],eps[1],alpha,sigma)];
}
// A prediction of one objective, read as noise and data: whichever one it
// names, the other follows from x = alpha x0 + sigma eps.
df_as_pair :: (o:DfObjective,pred:[2]f64,x:[2]f64,alpha:f64,sigma:f64)->DfPair {
    p:DfPair;r:=alpha*alpha+sigma*sigma;
    for d:0..<2 {
        if o==DfObjective.eps {p.eps[d]=pred[d];p.x0[d]=(x[d]-sigma*pred[d])/alpha;}
        if o==DfObjective.x0 {p.x0[d]=pred[d];p.eps[d]=(x[d]-alpha*pred[d])/sigma;}
        if o==DfObjective.v {p.eps[d]=(alpha*pred[d]+sigma*x[d])/r;p.x0[d]=(alpha*x[d]-sigma*pred[d])/r;}
        if o==DfObjective.flow {p.eps[d]=(x[d]+alpha*pred[d])/(alpha+sigma);p.x0[d]=p.eps[d]-pred[d];}
    }
    return p;
}
df_restate :: (from:DfObjective,to:DfObjective,pred:[2]f64,x:[2]f64,alpha:f64,sigma:f64)->[2]f64 {
    p:=df_as_pair(from,pred,x,alpha,sigma);return df_target(to,p.x0,p.eps,alpha,sigma);
}
// The squared error of one prediction against its objective's target.
df_loss :: (o:DfObjective,pred:[2]f64,x0:[2]f64,eps:[2]f64,alpha:f64,sigma:f64)->f64 {
    t:=df_target(o,x0,eps,alpha,sigma);return (pred[0]-t[0])*(pred[0]-t[0])+(pred[1]-t[1])*(pred[1]-t[1]);
}
// The best affine predictor a x + b of an objective for one class: the
// posterior means are affine in x, and every target is a fixed combination
// of them.
df_optimal :: (o:DfObjective,alpha:f64,sigma:f64,class:int)->[3]f64 {
    v:=alpha*alpha*DF_SPREAD*DF_SPREAD+sigma*sigma;ae:=sigma/v;ax:=alpha*DF_SPREAD*DF_SPREAD/v;
    w:=[3]f64.[df_mix(o,ax,ae,alpha,sigma),0,0];
    for d:0..<2 {mu:=DF_MEAN[2*class+d];w[1+d]=df_mix(o,mu-ax*alpha*mu,-ae*alpha*mu,alpha,sigma);}
    return w;
}
```

For one class every posterior mean is affine in `x`, so the best affine
predictor is the best predictor of any kind. `df_optimal` writes it down:
for the `eps` objective, `a = sigma / v` and `b = −a alpha mu`.

## Training

The model holds one affine predictor `a x + b` (three numbers: `a`, `b0`,
`b1`) for every noise level and class. Training a cell draws `n` fresh data
points and noises of that class, forms `x` and the objective's target, and
runs gradient descent from zero on the mean squared error

```text
L(a, b) = (1/n) Σ_i Σ_d (a x_id + b_d − y_id)²
dL/da   = (2/n) Σ_i Σ_d r_id x_id        r = a x + b − y
dL/db_d = (2/n) Σ_i r_id
```

The loss is quadratic, so its minimum also has a closed form, the least
squares solution, which is the independent check on the descent:

```text
a = (Σ x·y − (Σx)·(Σy) / n) / (Σ |x|² − |Σx|² / n)
b = (Σy − a Σx) / n
```

### Least squares by hand

Three points `(1, 0)`, `(0, 1)`, `(−1, −1)` with targets `(3, 1)`, `(0, 0)`,
`(2, −1)`. Then `Σx = (0, 0)`, `Σ|x|² = 1 + 1 + 2 = 4`,
`Σy = (5, 0)`, and `Σ x·y = 3 + 0 + (−2 + 1) = 2`. So `a = 2 / 4 = 0.5` and
`b = (5/3, 0)`. One descent step from zero at rate 0.2 follows the negative
gradient: `dL/da = (2/3)(−2) = −4/3` and `dL/db = (2/3)(−5, 0)`, so the step
lands at `a = 0.2 · 4/3 = 0.266667`, `b = (0.666667, 0)`. After 300 steps
it is within 10⁻¹² of the closed form.

### How many steps

Along `a` and `b` the loss curves by the matrix
`(2/n)[[Σ|x|², Σxᵀ], [Σx, n I]]`. At the least noisy levels `x` is nearly
the data: `E|x|² = 4.5` and `E x = mu`, which gives the matrix
`[[9, 4], [4, 2]]` (and 2 for the other offset), with eigenvalues 10.82 and
0.185. Descent converges for any rate below 2 / 10.82 = 0.185. The library
fixes the rate at 0.15 and refuses anything above it; at that rate each
step keeps at most 1 − 0.15 · 0.185 = 0.972 of the error in the flattest
direction, and 1,000 steps bring it below 10⁻¹². The example checks every
one of its 404 cells against least squares: the largest difference is below
10⁻⁹.

<!-- source: psionic/lib/diffusion.coax fit -->
```jai
// Along any level of this target the loss curves at most 10.8 (alpha = 1:
// the largest eigenvalue of [[9, 4], [4, 2]]), so a rate up to 2 / 10.8
// converges; 0.15 leaves room for sampling noise. The flattest direction
// curves about 0.185, so each step keeps at most 0.972 of the error.
DF_ITERATIONS :: 1000;
DF_RATE :: 0.15;
DF_RATE_MAX :: 0.15;
// The mean squared error of w = (a, b0, b1) on the points xs (two values a
// point) against the targets ys, and its gradient.
df_mse :: (xs:[]f64,ys:[]f64,w:[3]f64)->f64 {
    sum:=0.0;n:=xs.count/2;
    for i:0..<n {for d:0..<2 {r:=w[0]*xs[2*i+d]+w[1+d]-ys[2*i+d];sum+=r*r;}}
    return sum/f64(n);
}
df_gradient :: (xs:[]f64,ys:[]f64,w:[3]f64)->[3]f64 {
    g:[3]f64;n:=xs.count/2;
    for i:0..<n {for d:0..<2 {r:=w[0]*xs[2*i+d]+w[1+d]-ys[2*i+d];g[0]+=r*xs[2*i+d];g[1+d]+=r;}}
    for k:0..<3 {g[k]*=2/f64(n);}
    return g;
}
// Gradient descent from zero.
df_descend :: (xs:[]f64,ys:[]f64,iterations:int,rate:f64)->[3]f64 {
    w:[3]f64;
    for step:0..<iterations {g:=df_gradient(xs,ys,w);for k:0..<3 {w[k]-=rate*g[k];}}
    return w;
}
// The same minimum in closed form: b = (sum y - a sum x) / n, and a from the
// centred sums. A cell whose points all coincide gets a = 0.
df_fit :: (xs:[]f64,ys:[]f64)->[3]f64 {
    n:=xs.count/2;sx:[2]f64;sy:[2]f64;sxx:=0.0;sxy:=0.0;
    for i:0..<n {for d:0..<2 {sx[d]+=xs[2*i+d];sy[d]+=ys[2*i+d];sxx+=xs[2*i+d]*xs[2*i+d];sxy+=xs[2*i+d]*ys[2*i+d];}}
    den:=sxx-(sx[0]*sx[0]+sx[1]*sx[1])/f64(n);a:=0.0;
    if den>0 {a=(sxy-(sx[0]*sy[0]+sx[1]*sy[1])/f64(n))/den;}
    return [3]f64.[a,(sy[0]-a*sx[0])/f64(n),(sy[1]-a*sx[1])/f64(n)];
}
// One cell's training points: point i draws its data from normals 4q, 4q + 1
// and its noise from 4q + 2, 4q + 3, where q = (4 level + class) n + i and
// n = xs.count / 2.
df_cell :: (seed:u64,level:int,class:int,o:DfObjective,alpha:f64,sigma:f64,xs:[]f64,ys:[]f64) {
    n:=xs.count/2;
    for i:0..<n {
        q:=cast(u64)((4*level+class)*n+i);x0:=df_data(seed,2*q,class);
        eps:=[2]f64.[df_normal(seed,4*q+2),df_normal(seed,4*q+3)];x:=df_noise(x0,eps,alpha,sigma);y:=df_target(o,x0,eps,alpha,sigma);
        for d:0..<2 {xs[2*i+d]=x[d];ys[2*i+d]=y[d];}
    }
}
```

A model takes its levels either from a schedule (`alpha = sqrt(abar_t)`,
`sigma = sqrt(1 - abar_t)`) or from a flow grid (`t = j / N`,
`alpha = 1 − t`, `sigma = t`). With no levels it stands for the exact
posterior, which the samplers use to separate sampler error from model
error.

<!-- source: psionic/lib/diffusion.coax model -->
```jai
// A model whose levels are a schedule's: level t has alpha = sqrt(abar_t),
// sigma = sqrt(1 - abar_t).
df_vp_model :: (s:DfSchedule,o:DfObjective,alpha:[]f64,sigma:[]f64,w:[]f64)->DfModel {
    if s.status!=DfStatus.ok {return DfModel.{status=s.status};}
    if o==DfObjective.flow {return DfModel.{status=DfStatus.objective};}
    levels:=s.steps+1;
    if alpha.count<levels || sigma.count<levels || w.count<12*levels {return DfModel.{status=DfStatus.shape};}
    for t:0..<levels {alpha[t]=sqrt(s.abar[t]);sigma[t]=sqrt(1-s.abar[t]);}
    return DfModel.{objective=o,levels=levels,alpha=alpha[0..<levels],sigma=sigma[0..<levels],w=w[0..<12*levels]};
}
// A flow model on the grid t = j / steps: alpha = 1 - t, sigma = t.
df_flow_model :: (steps:int,alpha:[]f64,sigma:[]f64,w:[]f64)->DfModel {
    if steps<1 || steps>DF_STEPS_MAX {return DfModel.{status=DfStatus.steps};}
    levels:=steps+1;
    if alpha.count<levels || sigma.count<levels || w.count<12*levels {return DfModel.{status=DfStatus.shape};}
    for j:0..<levels {sigma[j]=f64(j)/f64(steps);alpha[j]=1-sigma[j];}
    return DfModel.{objective=DfObjective.flow,levels=levels,alpha=alpha[0..<levels],sigma=sigma[0..<levels],w=w[0..<12*levels]};
}
// Train every level and class: n fresh points a cell, gradient descent from
// zero on the objective's squared error.
df_train :: (m:DfModel,seed:u64,n:int,iterations:int,rate:f64,scratch:[]f64)->DfStatus {
    if m.status!=DfStatus.ok || m.levels<1 || n<2 || n>DF_CELL_MAX || scratch.count<4*n {return DfStatus.shape;}
    if iterations<1 {return DfStatus.steps;}
    if !(rate>0 && rate<=DF_RATE_MAX) {return DfStatus.range;}
    xs:=scratch[0..<2*n];ys:=scratch[2*n..<4*n];
    for j:0..<m.levels {for c:0..<4 {
        df_cell(seed,j,c,m.objective,m.alpha[j],m.sigma[j],xs,ys);
        w:=df_descend(xs,ys,iterations,rate);for k:0..<3 {m.w[12*j+3*c+k]=w[k];}
    }}
    return DfStatus.ok;
}
// Every cell set to its population optimum.
df_fill_optimal :: (m:DfModel) {
    for j:0..<m.levels {for c:0..<4 {w:=df_optimal(m.objective,m.alpha[j],m.sigma[j],c);for k:0..<3 {m.w[12*j+3*c+k]=w[k];}}}
}
// A model with levels was trained (or set); one without is the exact posterior.
df_trained :: (m:DfModel)->bool {return m.levels>0;}
// What a model says about x at a level: the exact posterior when it has no
// levels, else its cell's prediction read as noise and data.
df_pair :: (m:DfModel,level:int,alpha:f64,sigma:f64,class:int,x:[2]f64)->DfPair {
    if !df_trained(m) {return df_posterior(x,alpha,sigma,class);}
    k:=12*level+3*class;pred:=[2]f64.[m.w[k]*x[0]+m.w[k+1],m.w[k]*x[1]+m.w[k+2]];
    return df_as_pair(m.objective,pred,x,m.alpha[level],m.sigma[level]);
}
```

Conditioning is the class index: the model keeps a separate predictor per
class, and a sample asks for one. A model with levels refuses class −1, the
mixture, since it was never trained on it.

## Samplers

### DDPM, by hand

DDPM walks back one level at a time. From `x_t` and the predicted noise it
takes the mean of `x_{t-1}`, then adds fresh noise with the posterior
variance `(1 − abar_{t-1}) / (1 − abar_t) · beta_t`. At the last step that
variance is 0, since `abar_0 = 1`, so the last step adds no noise:

```text
x_{t-1} = (x_t − beta_t / sqrt(1 − abar_t) · eps) / sqrt(1 − beta_t) + spread · z
```

On the four-step linear schedule at `t = 4`, from `x = (0.5, −1)` with the
exact class-0 noise:

| Quantity | Value |
| --- | --- |
| `alpha`, `sigma` | sqrt(0.3024) = 0.549909, sqrt(0.6976) = 0.835225 |
| `v` | 0.3024 · 0.25 + 0.6976 = 0.7732 |
| `x − alpha mu_0` | (0.5 − 1.099818, −1) = (−0.599818, −1) |
| `eps` = (sigma / v)(x − alpha mu_0) | 1.080218 · (−0.599818, −1) = (−0.647934, −1.080218) |
| `beta / sigma` | 0.4 / 0.835225 = 0.478913 |
| `x − 0.478913 eps` | (0.810304, −0.482669) |
| mean = that / sqrt(0.6) | (1.046098, −0.623124) |
| variance of the added noise | 0.496 / 0.6976 · 0.4 = 0.284404 |

### DDIM, by hand

DDIM is deterministic. It reads the noise as a data guess, then noises that
guess to an earlier level with the same noise, so it can skip levels. From
the same `x` and `eps` at `t = 4` to `t = 2`:

| Quantity | Value |
| --- | --- |
| data guess `(x − sigma eps) / alpha` | (1.041171, −0.097775) / 0.549909 = (1.893351, −0.177803) |
| `sqrt(abar_2)`, `sqrt(1 − abar_2)` | sqrt(0.72) = 0.848528, sqrt(0.28) = 0.529150 |
| next point | 0.848528 · guess + 0.529150 · eps = (1.263707, −0.722468) |

### Euler and Heun on the flow, by hand

Flow matching uses the straight path `x_t = (1 − t) x0 + t eps` and learns
its velocity `eps − x0`. Sampling solves the ODE `dx/dt = velocity` from
`t = 1` (noise) back to `t = 0`. At `t = 1` the point is pure noise, so the
best data guess is the class mean and the velocity is `x − mu`. For class 1
from `z = (0.4, −0.2)`, one Euler step of size −0.5:

```text
velocity at t = 1:   (0.4, −0.2) − (0, 2) = (0.4, −2.2)
Euler:               (0.4, −0.2) − 0.5 · (0.4, −2.2) = (0.2, 0.9)
velocity at t = 0.5: (0.24, −2.12)
Heun:                (0.4, −0.2) − 0.5 · ((0.4, −2.2) + (0.24, −2.12)) / 2 = (0.24, 0.88)
```

Heun takes Euler's step as a guess, measures the velocity there too, and
steps with the average.

<!-- source: psionic/lib/diffusion.coax samplers -->
```jai
// DDPM: the mean of x_{t-1} given x_t and the predicted noise, plus the
// posterior's spread times fresh noise z. At the last step the spread is 0,
// since abar_0 = 1.
df_ddpm_step :: (s:DfSchedule,t:int,x:[2]f64,eps:[2]f64,z:[2]f64)->[2]f64 {
    out:[2]f64;spread:=sqrt((1-s.abar[t-1])/(1-s.abar[t])*s.beta[t]);
    for d:0..<2 {
        out[d]=(x[d]-s.beta[t]/sqrt(1-s.abar[t])*eps[d])/sqrt(1-s.beta[t])+spread*z[d];
    }
    return out;
}
// DDIM: guess the data, then noise it to level u with the same noise.
df_ddim_step :: (s:DfSchedule,t:int,u:int,x:[2]f64,eps:[2]f64)->[2]f64 {
    out:[2]f64;
    for d:0..<2 {x0:=(x[d]-sqrt(1-s.abar[t])*eps[d])/sqrt(s.abar[t]);out[d]=sqrt(s.abar[u])*x0+sqrt(1-s.abar[u])*eps[d];}
    return out;
}
// The flow's velocity eps - x0 at time t, where x = (1 - t) x0 + t eps.
df_velocity :: (m:DfModel,level:int,class:int,t:f64,x:[2]f64)->[2]f64 {
    p:=df_pair(m,level,1-t,t,class,x);return [2]f64.[p.eps[0]-p.x0[0],p.eps[1]-p.x0[1]];
}
// One step of the flow ODE from t = i / steps to (i - 1) / steps: Euler, or
// Heun (Euler's guess, then the average of the two velocities).
df_flow_step :: (m:DfModel,class:int,i:int,steps:int,x:[2]f64,heun:bool)->[2]f64 {
    per:=(m.levels-1)/steps;    // flow-grid levels a step; unused by the exact posterior
    t:=f64(i)/f64(steps);u:=f64(i-1)/f64(steps);h:=u-t;
    v:=df_velocity(m,i*per,class,t,x);guess:=[2]f64.[x[0]+h*v[0],x[1]+h*v[1]];
    if !heun {return guess;}
    w:=df_velocity(m,(i-1)*per,class,u,guess);
    return [2]f64.[x[0]+h*(v[0]+w[0])/2,x[1]+h*(v[1]+w[1])/2];
}
df_flow_solve :: (m:DfModel,class:int,steps:int,heun:bool,z:[2]f64)->[2]f64 {
    x:=z;for k:0..<steps {x=df_flow_step(m,class,steps-k,steps,x,heun);}
    return x;
}
// What every sampler needs before it starts.
df_admit :: (kind:DfSampler,s:DfSchedule,m:DfModel,class:int,steps:int)->DfStatus {
    if class< -1 || class>3 {return DfStatus.class;}
    if m.status!=DfStatus.ok {return m.status;}
    if df_trained(m) && class<0 {return DfStatus.class;}
    if kind==DfSampler.ddpm || kind==DfSampler.ddim {
        if s.status!=DfStatus.ok {return s.status;}
        if df_trained(m) && m.objective==DfObjective.flow {return DfStatus.objective;}
        if df_trained(m) && m.levels!=s.steps+1 {return DfStatus.shape;}
        if kind==DfSampler.ddpm && steps!=s.steps {return DfStatus.steps;}
        if steps<1 || steps>s.steps {return DfStatus.steps;}
        return DfStatus.ok;
    }
    if df_trained(m) && m.objective!=DfObjective.flow {return DfStatus.objective;}
    if steps<1 || steps>DF_STEPS_MAX {return DfStatus.steps;}
    if df_trained(m) && (m.levels-1<steps || (m.levels-1)%steps!=0) {return DfStatus.steps;}
    return DfStatus.ok;
}
// Sample k: it starts from normals 2002 k and 2002 k + 1; DDPM's step t
// adds normals 2002 k + 2t and 2002 k + 2t + 1.
df_sample :: (kind:DfSampler,s:DfSchedule,m:DfModel,class:int,steps:int,seed:u64,k:int)->DfSample {
    status:=df_admit(kind,s,m,class,steps);if status!=DfStatus.ok {return DfSample.{status=status};}
    base:=DF_STRIDE*cast(u64)k;x:=[2]f64.[df_normal(seed,base),df_normal(seed,base+1)];
    if kind==DfSampler.euler || kind==DfSampler.heun {return DfSample.{x=df_flow_solve(m,class,steps,kind==DfSampler.heun,x)};}
    if kind==DfSampler.ddim {
        for i:0..<steps {
            t:=(steps-i)*s.steps/steps;u:=(steps-i-1)*s.steps/steps;
            e:=df_pair(m,t,sqrt(s.abar[t]),sqrt(1-s.abar[t]),class,x).eps;x=df_ddim_step(s,t,u,x,e);
        }
        return DfSample.{x=x};
    }
    for i:0..<steps {
        t:=steps-i;e:=df_pair(m,t,sqrt(s.abar[t]),sqrt(1-s.abar[t]),class,x).eps;
        z:=[2]f64.[df_normal(seed,base+cast(u64)(2*t)),df_normal(seed,base+cast(u64)(2*t+1))];
        x=df_ddpm_step(s,t,x,e,z);
    }
    return DfSample.{x=x};
}
df_samples :: (kind:DfSampler,s:DfSchedule,m:DfModel,class:int,steps:int,seed:u64,n:int,out:[]f64)->DfStatus {
    if n<1 || out.count<2*n {return DfStatus.shape;}
    status:=df_admit(kind,s,m,class,steps);if status!=DfStatus.ok {return status;}
    for k:0..<n {p:=df_sample(kind,s,m,class,steps,seed,k);out[2*k]=p.x[0];out[2*k+1]=p.x[1];}
    return DfStatus.ok;
}
```

Sample `k` reads its starting noise from normals `2002 k` and `2002 k + 1`,
and DDPM's step `t` reads normals `2002 k + 2t` and `2002 k + 2t + 1`, so
every sample is a pure function of the seed and `k`. A request is refused
before anything is written when the class is outside −1..3 (`class`), when a
trained model is asked for the mixture (`class`), when a diffusion sampler
gets a flow model or the reverse (`objective`), when the model's levels do
not match the schedule (`shape`), when DDPM's steps differ from the
schedule's or DDIM's exceed them (`steps`), or when a flow model's grid is
shorter than the steps or not a multiple of them (`steps`).

### The order of the flow solvers

For one class the flow ODE has a closed-form solution: the point at time `t`
is `(1 − t) mu + sqrt((1 − t)² 0.25 + t²) z`, so the exact end is
`mu + 0.5 z`. Starting from one `z`, the error of Euler's end point should
halve each time the step halves (first order), and Heun's should quarter
(second order). The transcript's table shows the ratios approaching 2 and 4.

## The gate

A sample set passes when each statistic is within 4 standard errors of the
target's value. The standard errors come from the target alone and were
fixed before any sample was drawn. Each test is written squared,
`dev² · n ≤ 16 · se² · n`, so the right side is an exact constant and no
square root rounds:

| Statistic | Target | Standard error, n samples |
| --- | --- | --- |
| a class's mean, each coordinate | `mu_c` | 0.5 / √n |
| a class's variance | 0.25 | 0.25 / √n |
| the mixture's mean, each coordinate | 0 | 1.5 / √n |
| the mixture's variance | 2.25 | √(1.0625 / n) |
| each class's share of nearest points | 1/4 | √(3/16 / n) |
| spread about the nearest mean | 0.25 | 0.25 / √n |

The mixture's variance has the odd standard error because `x²` and `y²` are
not independent there: `(x² + y²) / 2` has variance 1.0625 under the target
(`Var x² = 11.1875 − 2.25² = 6.125`, `Cov(x², y²) = 1.0625 − 2.25² = −4`, and
`(2 · 6.125 − 2 · 4) / 4 = 1.0625`). With 1,000 samples the variance must
land within 0.130 of 2.25, and the spread within 0.032 of 0.25. The checks
put every bound exactly on its edge, for example a class mean 0.25 off with
64 samples, and a mixture variance 0.25 off with 272, and see it pass, and
see the next value up fail.

<!-- source: psionic/lib/diffusion.coax gate -->
```jai
DF_GATE_Z :: 4.0;              // standard errors a statistic may stray
DF_TOTAL_VAR :: 2.25;          // the mixture's variance a coordinate: 2 + 0.5^2
DF_SQUARE_VAR :: 1.0625;       // the variance of (x^2 + y^2) / 2 under the mixture
// The squared distance from (x, y) to class c's mean.
df_distance2 :: (x:f64,y:f64,c:int)->f64 {d0:=x-DF_MEAN[2*c];d1:=y-DF_MEAN[2*c+1];return d0*d0+d1*d1;}
df_stats :: (xs:[]f64,n:int)->DfStats {
    if n<1 || xs.count<2*n {return DfStats.{status=DfStatus.shape};}
    st:=DfStats.{n=n};
    for i:0..<n {for d:0..<2 {st.mean[d]+=xs[2*i+d];}}
    for d:0..<2 {st.mean[d]/=f64(n);}
    for i:0..<n {
        best:=0;near:=df_distance2(xs[2*i],xs[2*i+1],0);
        for k:0..<3 {q:=df_distance2(xs[2*i],xs[2*i+1],k+1);if q<near {near=q;best=k+1;}}
        st.count[best]+=1;st.within+=near;
        for d:0..<2 {dev:=xs[2*i+d]-st.mean[d];st.var+=dev*dev;}
    }
    st.var/=f64(2*n);st.within/=f64(2*n);
    return st;
}
// Fixed before any sample was drawn: each statistic within 4 standard errors
// of the target's value, checked squared, as dev^2 n <= 16 se^2 n, so that no
// square root rounds. One class: its mean (se^2 n = 0.25 a coordinate) and
// its variance (0.0625). The mixture: the mean (2.25), the variance (1.0625:
// (x^2 + y^2) / 2 has that variance under the target), each class's share of
// nearest points (3/16 around 1/4) and the spread about the nearest mean
// (0.0625).
df_close :: (dev:f64,n:int,var_n:f64)->bool {return dev*dev*f64(n)<=DF_GATE_Z*DF_GATE_Z*var_n;}
df_gate :: (st:DfStats,class:int)->bool {
    if st.status!=DfStatus.ok {return false;}
    s2:=DF_SPREAD*DF_SPREAD;
    if class>=0 {
        return df_close(st.mean[0]-DF_MEAN[2*class],st.n,s2) && df_close(st.mean[1]-DF_MEAN[2*class+1],st.n,s2) && df_close(st.var-s2,st.n,s2*s2);
    }
    ok:=df_close(st.mean[0],st.n,DF_TOTAL_VAR) && df_close(st.mean[1],st.n,DF_TOTAL_VAR) && df_close(st.var-DF_TOTAL_VAR,st.n,DF_SQUARE_VAR) && df_close(st.within-s2,st.n,s2*s2);
    for c:0..<4 {if !df_close(f64(st.count[c])/f64(st.n)-0.25,st.n,3.0/16) {ok=false;}}
    return ok;
}
```

## Run the example

<!-- source: psionic/examples/diffusion.coax schedules -->
```jai
    lin:=df_linear(4,0.1,0.4);cos4:=df_cosine(4);
    print("linear, 4 steps: abar % % % %\n",r6(lin.abar[1]),r6(lin.abar[2]),r6(lin.abar[3]),r6(lin.abar[4]));
    print("cosine, 4 steps: beta % % % %\n",r6(cos4.beta[1]),r6(cos4.beta[2]),r6(cos4.beta[3]),r6(cos4.beta[4]));
    print("cosine, 4 steps: abar % % % %\n",r6(cos4.abar[1]),r6(cos4.abar[2]),r6(cos4.abar[3]),r6(cos4.abar[4]));
    s:=df_cosine(T);print("cosine, % steps: abar_1 %, abar_50 %, abar_100 %\n",T,r6(s.abar[1]),r6(s.abar[50]),s.abar[100]);
    
```

The first lines check the schedules by hand above. Then the forward process
and the four objectives:

<!-- source: psionic/examples/diffusion.coax objectives -->
```jai
    x0:=[2]f64.[2,0];eps:=[2]f64.[1,-1];x:=df_noise(x0,eps,0.8,0.6);guess:=[2]f64.[0.5,-0.5];
    print("x = 0.8 (2, 0) + 0.6 (1, -1) = (%, %)\n",x[0],x[1]);
    names:=[4]string.["eps","x0","v","flow"];
    for o:0..<4 {
        ob:=cast(DfObjective)o;p:=df_restate(DfObjective.eps,ob,guess,x,0.8,0.6);
        print("  guess as %: (%, %), loss %\n",names[o],r6(p[0]),r6(p[1]),r6(df_loss(ob,p,x0,eps,0.8,0.6)));
    }
    best:=df_posterior(x,0.8,0.6,0);mix:=df_posterior(x,0.8,0.6,-1);
    print("  best guess for class 0: eps (%, %), x0 (%, %)\n",r6(best.eps[0]),r6(best.eps[1]),r6(best.x0[0]),r6(best.x0[1]));
    print("  best guess for the mixture: eps (%, %), x0 (%, %)\n",r6(mix.eps[0]),r6(mix.eps[1]),r6(mix.x0[0]),r6(mix.x0[1]));
    
```

Then training, for the `eps` objective on the 100-step cosine schedule, 128
points per cell:

<!-- source: psionic/examples/diffusion.coax train -->
```jai
    al:[T+1]f64;sg:[T+1]f64;w:[12*(T+1)]f64;scratch:[4*CELL]f64;
    m:=df_vp_model(s,DfObjective.eps,al[..],sg[..],w[..]);
    print("train the noise predictor: % levels x 4 classes, % points, % steps at rate %: %\n",T+1,CELL,DF_ITERATIONS,DF_RATE,
        df_train(m,SEED_TRAIN,CELL,DF_ITERATIONS,DF_RATE,scratch[..]));
    xs:=scratch[0..<2*CELL];ys:=scratch[2*CELL..<4*CELL];gap:=0.0;off:=0.0;where:=0;
    for j:0..<T+1 {for c:0..<4 {
        df_cell(SEED_TRAIN,j,c,DfObjective.eps,al[j],sg[j],xs,ys);f:=df_fit(xs,ys);o:=df_optimal(DfObjective.eps,al[j],sg[j],c);
        for k:0..<3 {if abs(w[12*j+3*c+k]-f[k])>gap {gap=abs(w[12*j+3*c+k]-f[k]);}if abs(w[12*j+3*c+k]-o[k])>off {off=abs(w[12*j+3*c+k]-o[k]);where=j;}}
    }}
    print("  largest gap to least squares: %\n",ifx gap<1e-9 then "below 1e-9" else "too large");
    print("  largest distance to the population optimum: %, at level %\n",r4(off),where);
    o:=df_optimal(DfObjective.eps,al[50],sg[50],1);
    print("  level 50, class 1: learned (%, %, %), optimum (%, %, %)\n",r4(w[603]),r4(w[604]),r4(w[605]),r4(o[0]),r4(o[1]),r4(o[2]));
    
```

and sampling, 1,000 samples per measurement:

<!-- source: psionic/examples/diffusion.coax sample -->
```jai
    out:[2*N]f64;exact:=DfModel.{};passed:=0;gates:=0;
    print("% samples each, seed %:\n",N,SEED_SAMPLE);
    for c:0..<4 {gates+=1;if measure("  DDPM, learned",DfSampler.ddpm,s,m,c,T,out[..]) {passed+=1;}}
    gates+=1;if measure("  DDPM, exact mixture, 100 steps",DfSampler.ddpm,s,exact,-1,T,out[..]) {passed+=1;}
    gates+=1;if measure("  DDIM, exact mixture, 100 steps",DfSampler.ddim,s,exact,-1,T,out[..]) {passed+=1;}
    print("gates passed: % of %\n",passed,gates);
    print("too few steps:\n");
    few:=measure("  DDPM, exact mixture, 10 steps",DfSampler.ddpm,df_cosine(10),exact,-1,10,out[..]);
    few=measure("  DDIM, exact mixture, 10 steps",DfSampler.ddim,s,exact,-1,10,out[..]);
    zero:[12*(T+1)]f64;untrained:=df_vp_model(s,DfObjective.eps,al[..],sg[..],zero[..]);
    print("an untrained model (every weight 0):\n");
    none:=measure("  DDPM",DfSampler.ddpm,s,untrained,0,T,out[..]);
    
```

The flow model is trained on a 20-interval grid and sampled with 10 Heun
steps:

<!-- source: psionic/examples/diffusion.coax flow -->
```jai
    fa:[FLOW+1]f64;fs:[FLOW+1]f64;fw:[12*(FLOW+1)]f64;
    fm:=df_flow_model(FLOW,fa[..],fs[..],fw[..]);
    print("train the flow velocity: % levels x 4 classes: %\n",FLOW+1,df_train(fm,SEED_TRAIN,CELL,DF_ITERATIONS,DF_RATE,scratch[..]));
    fp:=0;
    for c:0..<4 {if measure("  Heun, learned, 10 steps",DfSampler.heun,s,fm,c,10,out[..]) {fp+=1;}}
    if measure("  Heun, exact mixture, 10 steps",DfSampler.heun,s,exact,-1,10,out[..]) {fp+=1;}
    print("gates passed: % of 5\ntoo few steps:\n",fp);
    few=measure("  Euler, exact mixture, 10 steps",DfSampler.euler,s,exact,-1,10,out[..]);
    
```

The whole transcript, identical in debug and release builds:

```text
linear, 4 steps: abar 0.9 0.72 0.504 0.3024
cosine, 4 steps: beta 0.152988 0.416958 0.707859 0.999
cosine, 4 steps: abar 0.847012 0.493844 0.144272 0.000144
cosine, 100 steps: abar_1 0.999369, abar_50 0.493844, abar_100 2.428572279349792e-07
normal draw, seed 0, k 0: -1.8839083333524405
forward, class 0 at alpha 0.8: mean (1.5963, 0.0004), variance 0.5213; closed form (1.6, 0), 0.52
x = 0.8 (2, 0) + 0.6 (1, -1) = (2.2, -0.6)
  guess as eps: (0.5, -0.5), loss 0.5
  guess as x0: (2.375, -0.375), loss 0.28125
  guess as v: (-1.025, -0.175), loss 0.78125
  guess as flow: (-1.875, -0.125), loss 1.53125
  best guess for class 0: eps (0.692308, -0.692308), x0 (2.230769, -0.230769)
  best guess for the mixture: eps (0.705979, -0.679305), x0 (2.220516, -0.240521)
train the noise predictor: 101 levels x 4 classes, 128 points, 1000 steps at rate 0.15: ok
  largest gap to least squares: below 1e-9
  largest distance to the population optimum: 0.6313, at level 4
  level 50, class 1: learned (1.1359, 0.0349, -1.5427), optimum (1.13, 0.0, -1.5881)
1000 samples each, seed 7:
  DDPM, learned, class 0: mean (1.9736, 0.0234), variance 0.2464, nearest 993 4 0 3, spread 0.2443 -> gate pass
  DDPM, learned, class 1: mean (-0.0235, 1.9756), variance 0.2385, nearest 4 993 3 0, spread 0.2358 -> gate pass
  DDPM, learned, class 2: mean (-2.007, -0.0077), variance 0.2351, nearest 0 3 993 4, spread 0.2323 -> gate pass
  DDPM, learned, class 3: mean (-0.0372, -2.0194), variance 0.2325, nearest 2 0 3 995, spread 0.2318 -> gate pass
  DDPM, exact mixture, 100 steps: mean (-0.0336, -0.0789), variance 2.2183, nearest 242 233 258 267, spread 0.2375 -> gate pass
  DDIM, exact mixture, 100 steps: mean (0.0034, 0.0378), variance 2.1972, nearest 246 263 243 248, spread 0.2437 -> gate pass
gates passed: 6 of 6
too few steps:
  DDPM, exact mixture, 10 steps: mean (-0.0309, 0.0414), variance 1.9292, nearest 250 267 247 236, spread 0.208 -> gate fail
  DDIM, exact mixture, 10 steps: mean (0.0042, 0.036), variance 1.9241, nearest 246 263 243 248, spread 0.2307 -> gate fail
an untrained model (every weight 0):
  DDPM, class 0: mean (-20.3788, 52.5759), variance 4059583.1037, nearest 246 261 244 249, spread 4056637.2909 -> gate fail
train the flow velocity: 21 levels x 4 classes: ok
  Heun, learned, 10 steps, class 0: mean (1.9776, 0.0319), variance 0.2276, nearest 997 2 0 1, spread 0.2281 -> gate pass
  Heun, learned, 10 steps, class 1: mean (-0.039, 2.0132), variance 0.2343, nearest 1 998 1 0, spread 0.2349 -> gate pass
  Heun, learned, 10 steps, class 2: mean (-1.9811, -0.004), variance 0.2266, nearest 0 2 997 1, spread 0.2261 -> gate pass
  Heun, learned, 10 steps, class 3: mean (-0.0099, -1.983), variance 0.2329, nearest 2 0 2 996, spread 0.2322 -> gate pass
  Heun, exact mixture, 10 steps: mean (0.0034, 0.0382), variance 2.2283, nearest 246 263 243 248, spread 0.2524 -> gate pass
gates passed: 5 of 5
too few steps:
  Euler, exact mixture, 10 steps: mean (0.004, 0.0362), variance 1.9067, nearest 246 263 243 248, spread 0.2462 -> gate fail
class 1 from z = (-0.150789, 1.235967): exact end (-0.075395, 2.617983)
  4 steps: Euler error 0.198548, Heun error 0.00602
  8 steps: Euler error 0.106104, Heun error 0.00304; halving the step divides them by 1.8713 and 1.9803
  16 steps: Euler error 0.05518, Heun error 0.000951; halving the step divides them by 1.9229 and 3.1964
  32 steps: Euler error 0.028185, Heun error 0.000261; halving the step divides them by 1.9578 and 3.6373
  64 steps: Euler error 0.01425, Heun error 0.000068; halving the step divides them by 1.9779 and 3.8275
sample 0, seed 7: (0.6705417335151039, 2.131017800549175), bits 4604214922547666393 4611981044286253987
again: same bits true; seed 8 gives (-0.4610600789839521, 2.078532038495612)
```

What it shows:

- **Training reaches least squares.** Every cell is within 10⁻⁹ of its
  closed-form minimum. The learned coefficients are within 0.05 of the
  population optimum at level 50; the largest distance, 0.63, is at level
  4, where the noise is so small that `x` says little about it and 128
  points pin `a` down poorly.
- **Learned models pass.** DDPM with the trained noise predictor puts each
  class's samples at its mean with its variance, and so does Heun with the
  trained flow velocity. Most samples are nearest their own class's mean;
  the few that are not match the 1 in 200 the target itself puts there.
- **The exact posterior passes too,** for DDPM and DDIM with 100 steps and
  Heun with 10, so the samplers themselves are right.
- **Too few steps fail.** DDPM and DDIM with 10 steps, and Euler with 10,
  all shrink the mixture's variance to about 1.9. The gate catches it on the
  variance. Heun with the same 10 steps does not shrink it, which is the
  second-order accuracy of the table at work.
- **Deterministic samplers agree on modes.** DDIM, Euler and Heun map the
  same starting noise to the same class (`nearest 246 263 243 248` in every
  line), because they solve the same ODE: DDIM is a first-order solver of
  it in a different time variable.
- **An untrained model fails** by a wide margin.
- **A seed names a sample.** The same seed reproduces sample 0 bit for bit,
  and seed 8 gives a different one.

## What it refuses

[`diffusion-refuse.coax`](../examples/diffusion-refuse.coax) asks for
every refusal in turn:

<!-- source: psionic/examples/diffusion-refuse.coax refuse -->
```jai
main :: () {
    print("linear schedule, 0 steps: %\n",df_linear(0,0.1,0.2).status);
    print("linear schedule, 1,001 steps: %\n",df_linear(1001,0.1,0.2).status);
    print("linear schedule, betas 0.3 down to 0.2: %\n",df_linear(4,0.3,0.2).status);
    print("linear schedule, first beta 0: %\n",df_linear(4,0,0.2).status);
    print("linear schedule, last beta 1: %\n",df_linear(4,0.1,1).status);
    print("linear schedule, a NaN beta: %\n",df_linear(4,0.1,0.0/0.0).status);
    print("cosine schedule, 0 steps: %\n",df_cosine(0).status);
    b:=[3]f64.[0.1,0.05,0.2];print("betas 0.1, 0.05, 0.2: %\n",df_betas(b[..]).status);
    b[1]=1.5;print("betas 0.1, 1.5, 0.2: %\n",df_betas(b[..]).status);

    s:=df_linear(4,0.1,0.4);al:[5]f64;sg:[5]f64;w:[60]f64;scratch:[64]f64;
    print("a diffusion model with flow levels: %\n",df_vp_model(s,DfObjective.flow,al[..],sg[..],w[..]).status);
    print("a 4-step model in room for 59 weights of 60: %\n",df_vp_model(s,DfObjective.eps,al[..],sg[..],w[0..<59]).status);
    print("a model on a refused schedule: %\n",df_vp_model(df_linear(4,0.3,0.2),DfObjective.eps,al[..],sg[..],w[..]).status);
    m:=df_vp_model(s,DfObjective.eps,al[..],sg[..],w[..]);
    print("train on 1 point a cell: %\n",df_train(m,1,1,10,0.1,scratch[..]));
    print("train 17 points in scratch for 16: %\n",df_train(m,1,17,10,0.1,scratch[..]));
    print("train 0 steps: %\n",df_train(m,1,8,0,0.1,scratch[..]));
    print("train at rate 0.2, above 0.15: %\n",df_train(m,1,8,10,0.2,scratch[..]));

    out:[8]f64;for i:0..<8 {out[i]=-7;}exact:=DfModel.{};
    print("sample class 4 of 0..3: %\n",df_samples(DfSampler.ddpm,s,exact,4,4,1,4,out[..]));
    print("sample the mixture from a trained model: %\n",df_samples(DfSampler.ddpm,s,m,-1,4,1,4,out[..]));
    print("DDPM in 3 steps on a 4-step schedule: %\n",df_samples(DfSampler.ddpm,s,exact,0,3,1,4,out[..]));
    print("DDIM in 5 steps on a 4-step schedule: %\n",df_samples(DfSampler.ddim,s,exact,0,5,1,4,out[..]));
    print("DDPM on a decreasing schedule: %\n",df_samples(DfSampler.ddpm,df_linear(4,0.3,0.2),exact,0,4,1,4,out[..]));
    print("Heun with a noise predictor: %\n",df_samples(DfSampler.heun,s,m,0,4,1,4,out[..]));
    f:=df_flow_model(4,al[..],sg[..],w[..]);
    print("Euler in 3 steps on a 4-interval flow grid: %\n",df_samples(DfSampler.euler,s,f,0,3,1,4,out[..]));
    print("5 samples into room for 4: %\n",df_samples(DfSampler.ddpm,s,exact,0,4,1,5,out[..]));
    untouched:=0;for v:out {if v==-7 {untouched+=1;}}print("values still -7: % of 8\n",untouched);
    print("statistics of 0 samples: %\n",df_stats(out[..],0).status);
}
```

```text
linear schedule, 0 steps: steps
linear schedule, 1,001 steps: steps
linear schedule, betas 0.3 down to 0.2: order
linear schedule, first beta 0: range
linear schedule, last beta 1: range
linear schedule, a NaN beta: number
cosine schedule, 0 steps: steps
betas 0.1, 0.05, 0.2: order
betas 0.1, 1.5, 0.2: range
a diffusion model with flow levels: objective
a 4-step model in room for 59 weights of 60: shape
a model on a refused schedule: order
train on 1 point a cell: shape
train 17 points in scratch for 16: shape
train 0 steps: steps
train at rate 0.2, above 0.15: range
sample class 4 of 0..3: class
sample the mixture from a trained model: class
DDPM in 3 steps on a 4-step schedule: steps
DDIM in 5 steps on a 4-step schedule: steps
DDPM on a decreasing schedule: order
Heun with a noise predictor: objective
Euler in 3 steps on a 4-interval flow grid: steps
5 samples into room for 4: shape
values still -7: 8 of 8
statistics of 0 samples: shape
```

The program exits with status 0: each refusal is a returned status, and the
sample buffer keeps its −7 values.

## How it is checked

The checks in [`diffusion.coax`](../lib/diffusion.coax) come before the code
they test, and compare it with something that does not share its code:

- **Hand values.** The normal draw, both schedules, the forward point, the
  posterior, the four losses, the least squares example, the DDPM step, the
  DDIM jump and the Euler and Heun steps, as worked above.
- **Finite differences.** The posterior noise equals `−sigma` times the
  gradient of a separately written log density, on 15 points and levels for
  every class and the mixture, within 10⁻⁸; the descent's gradient matches
  central differences of its loss.
- **Closed forms.** Gradient descent reaches the least squares solution
  within 10⁻⁹. On 4,096 points per objective, the least squares answers of
  all four objectives are one predictor (within 10⁻¹²), and the `eps` answer
  is within 0.06 of the population optimum. The optima of the four
  objectives, for every class, equal the exact posterior; a model set to
  them samples like the exact posterior with DDPM, DDIM, Euler and Heun.
- **Convergence order.** On the exact Gaussian flow, from 32 to 64 steps
  Euler's error falls by between 1.9 and 2.1, Heun's by between 3.5 and 4.1.
- **Replay.** A sample equals a hand-built replay of its steps from the
  normals its index names; the same seed gives the same bits and another
  seed different ones.
- **Boundaries.** Every refusal, and the gate at each tolerance's edge on
  both sides.

`coax mutate psionic/lib/diffusion.coax` changes the library one site at a
time and reruns the checks. All 936 mutants are caught: none survive, and
none is marked equivalent. The first run left 80; closing them changed the
code as well as the checks. A comparison whose two branches agree at its
boundary is not written: the cosine cap is the `abs` form above, the gate
compares squares against exact constants, and the DDPM step adds its
spread at every step rather than testing for the last one. Where a tie is
real, the tie is made visible: the posterior reports its most likely class
and the statistics count a point equally near all four means for class 0,
and the checks put a point exactly there. The training helpers take their
point count from their slices, so a slice too long changes the answer.

## What this chapter does not do

- The predictor is affine per noise level and class, which is exactly right
  for one Gaussian class and nothing more. It is not a neural network, it is
  not shared across levels, and its gradient is written by hand rather than
  derived by [chapter 106](ch106-differentiate-any-tensor-program.md)'s
  automatic differentiation. The unconditional mixture is sampled only with
  the exact posterior.
- Conditioning is a class index. Vector conditioning and classifier-free
  guidance are not built.
- Training draws fresh points per cell and runs full-batch descent; it does
  not sample levels at random or weight the loss across levels, the choices
  a shared network would need.
- The gate compares means, variances, shares and spread. It is not a full
  two-sample test of the distributions.
- Everything runs in binary64 on one CPU core, in two dimensions.

## Dependencies

- [Chapter 53](ch053-choosing-the-next-token.md): the counter-based
  generator every normal draw comes from.
- [Chapters 103](ch103-learning-with-arrays.md) and
  [104](ch104-gradients-as-a-program.md): a loss, its gradient, and descent.
- [Chapter 106](ch106-differentiate-any-tensor-program.md): the automatic
  differentiation a network-sized model would use instead of the hand
  gradient here.

## Sources

- [Chapter plan](PLAN.md), Part XIII.
- Jonathan Ho, Ajay Jain and Pieter Abbeel, "Denoising Diffusion
  Probabilistic Models", NeurIPS 2020: the forward process, the noise
  objective and the DDPM sampler.
- Jiaming Song, Chenlin Meng and Stefano Ermon, "Denoising Diffusion
  Implicit Models", ICLR 2021: the DDIM sampler.
- Alex Nichol and Prafulla Dhariwal, "Improved Denoising Diffusion
  Probabilistic Models", ICML 2021: the cosine schedule and its 0.999 cap.
- Tim Salimans and Jonathan Ho, "Progressive Distillation for Fast Sampling
  of Diffusion Models", ICLR 2022: the velocity objective.
- Yaron Lipman, Ricky T. Q. Chen, Heli Ben-Hamu, Maximilian Nickel and Matt
  Le, "Flow Matching for Generative Modeling", ICLR 2023, and Xingchao Liu,
  Chengyue Gong and Qiang Liu, "Flow Straight and Fast", ICLR 2023: the
  straight path and its velocity.
- Tero Karras, Miika Aittala, Timo Aila and Samuli Laine, "Elucidating the
  Design Space of Diffusion-Based Generative Models", NeurIPS 2022: Heun's
  method as a diffusion sampler.
