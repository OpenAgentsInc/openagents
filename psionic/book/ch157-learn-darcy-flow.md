# 157. Learn Darcy flow

Darcy flow describes the pressure of a fluid seeping through a porous
material. Where the material is permeable the fluid moves easily and the
pressure stays low; where it is tight the pressure builds. Written as an
equation on the unit square, with permeability a(x, y), a source f and the
pressure u held at zero on the boundary:

−div(a grad u) = f.

Given a, a numerical solver finds u. A Fourier neural operator (FNO) learns
to do the same from examples: it reads the whole permeability field and
predicts the whole pressure field in one forward pass. This chapter builds
both, in [`darcy.coax`](../lib/darcy.coax):

- a **finite-difference solver**: a five-point scheme with the permeability
  on cell faces, solved by conjugate gradients, checked by hand on one and
  nine grid points and against a manufactured solution whose error falls at
  second order as the grid is refined;
- a **seeded data generator**: smooth random permeability fields
  a = exp(g), where g is a sum of nine cosine modes with seeded coefficients,
  and the solver's pressure for f = 1;
- a **small FNO**: a pointwise lift from (log a, x, y) to 4 channels, three
  Fourier layers from [chapter 156](ch156-a-spectral-neural-operator.md), and
  a pointwise projection through 16 hidden units to one output, with the
  whole network's gradient written by hand;
- **training** with [chapter 124](ch124-adamw-and-explicit-precision.md)'s
  AdamW on minibatches, and the **held-out relative L2 error** against the
  solver on fields from seeds the training never saw, at the training grid
  and at a grid twice as fine.

Everything runs in binary64 on one CPU core, with sums in a fixed order. That
is the profile `psionic-darcy-v1`. A grid has 1 to 32 interior points a side;
the network is at most 4 channels wide (chapter 156's limit), with at most 4
Fourier layers and a projection at most 16 wide. The source term is f = 1
throughout.

The architecture follows the shape of NVIDIA PhysicsNeMo's Darcy FNO example
(lift, Fourier layers, projection; relative L2 error against a solver on
generated data), but much smaller, and it is not a port of that recipe's
hyperparameters, its piecewise-constant permeability fields or its
checkpoint. The model does not replace the solver: the solver is the
reference that every number here is measured against.

## Run it

The program is [`darcy-flow.coax`](../examples/darcy-flow.coax); the checks'
oracles are in [`darcy-checks.coax`](../tests/darcy-checks.coax).

```shell
coax/bin/coax run psionic/examples/darcy-flow.coax
coax/bin/coax test psionic/lib/darcy.coax
coax/bin/coax mutate psionic/lib/darcy.coax --root-only
```

```text
1 point: u = 0.0625 (1 iteration)
9 points: centre 0.0703125 = 9/128, edge 0.0546875 = 7/128, corner 0.04296875 = 11/256 (3 iterations)
manufactured u, largest error: n = 7: 0.01291, n = 15: 0.003209, n = 31: 0.000801
observed order: 2.0086 and 2.0021
32 training and 16 held-out fields at 16 x 16; at most 195 solver iterations
field 1000: c = -1.0606 0.503 0.2187 ...; u at the centre node: 0.07059
network: 11789 parameters; 80 steps
  epoch 0: loss 0.1574
  epoch 5: loss 0.009465
  epoch 10: loss 0.003695
  epoch 15: loss 0.002795
  epoch 19: loss 0.002654
held-out relative L2 at 16 x 16: mean 0.1328, worst 0.2491
same weights at 32 x 32: mean 0.1462, worst 0.2655
a face coefficient of 0: coefficient
a 33 x 33 grid: size
9 modes: network
6 modes on an 8 x 8 grid: size
a NaN in the input: number
a learning rate of 2: number
```

The example trains on 32 fields for only 80 steps so that it stays quick (it
compiles, checks and runs in under half a minute); its error, about 13%, is
correspondingly rough. The longer run under [A longer run](#a-longer-run) is
the chapter's measured result.

## The solver, worked by hand

Put n × n interior nodes on the unit square with spacing h = 1/(n + 1): node
(i, j) sits at x = (i + 1)h, y = (j + 1)h. Integrate the equation over the
small square around a node. The flux through each of its four faces is the
face's permeability times the difference in u across it, divided by h. So
each node's equation is

(1/h²) Σ over its four faces of a_face · (u_node − u_neighbour) = f_node,

with u = 0 for a neighbour on the boundary. The permeability is sampled at
the face midpoints, between the two nodes the face separates.

**One node.** With n = 1, h = 1/2 and the only node at the centre. All four
neighbours are on the boundary. With a = 1 and f = 1 the equation is
4 · 4u = 1, so u = 1/16 = 0.0625: the first line of the transcript. (The
continuous problem's centre value is about 0.0737; one node is a coarse
grid.)

**Nine nodes.** With n = 3, h = 1/4 and 1/h² = 16. By symmetry there are only
three unknowns: the centre m, the four edge nodes e and the four corners c.
The centre's neighbours are four edge nodes; an edge node's are the centre,
two corners and the boundary; a corner's are two edge nodes and the
boundary:

- centre: 16(4m − 4e) = 1
- edge: 16(4e − m − 2c) = 1
- corner: 16(4c − 2e) = 1

Write s = 1/16. The corner equation gives c = (s + 2e)/4. The centre gives
m = e + s/4. Substituting both into the edge equation: 4e − e − s/4 −
(s + 2e)/2 = s, so 2e = 7s/4 and e = 7/128. Then m = 7/128 + 2/128 = 9/128
and c = (8/128 + 14/128)/4 = 11/256. The transcript's second line prints
0.0703125, 0.0546875 and 0.04296875, which are exactly these fractions, and a
`#test` asserts them within 10⁻¹⁵. Conjugate gradients finds them in three
iterations: the symmetric right-hand side only excites three independent
directions.

<!-- source: psionic/lib/darcy.coax solve -->
```jai
// Node (i, j), row j, sits at x = (i + 1) h, y = (j + 1) h with h = 1/(n + 1),
// at u[j n + i]. ax[j (n + 1) + i] is a on the face left of node i in row j
// (i = n is the face right of the last node); ay[j n + i] the face below
// node i in row j. Each node's equation is
// (1/h^2) sum over its four faces of a_face (u_node - u_neighbour) = f,
// with u = 0 beyond the boundary.
dk_apply :: (n:int,ax:[]f64,ay:[]f64,u:[]f64,out:[]f64) {
    q:=f64((n+1)*(n+1));
    for j:0..<n {for i:0..<n {
        c:=u[j*n+i];w:=0.0;e:=0.0;s:=0.0;t:=0.0;
        if i>0 {w=u[j*n+i-1];}
        if i<n-1 {e=u[j*n+i+1];}
        if j>0 {s=u[(j-1)*n+i];}
        if j<n-1 {t=u[(j+1)*n+i];}
        out[j*n+i]=q*(ax[j*(n+1)+i]*(c-w)+ax[j*(n+1)+i+1]*(c-e)+ay[j*n+i]*(c-s)+ay[(j+1)*n+i]*(c-t));
    }}
}
dk_dot :: (a:[]f64,b:[]f64)->f64 {s:=0.0;for i:0..<a.count {s+=a[i]*b[i];}return s;}
// Conjugate gradients from u = 0: the matrix is symmetric and positive
// definite when every face coefficient is positive. Stop when the residual
// is at most 1e-12 of f; refuse with converge if cap iterations don't get
// there.
dk_solve :: (n:int,ax:[]f64,ay:[]f64,f:[]f64,u:[]f64,cap:int)->DkSolve {
    status:=dk_check(n,ax,ay,f,u);if status!=DkStatus.ok {return DkSolve.{status=status};}
    cells:=n*n;r:[DK_CELLS_MAX]f64;p:[DK_CELLS_MAX]f64;ap:[DK_CELLS_MAX]f64;
    for t:0..<cells {u[t]=0;r[t]=f[t];p[t]=f[t];}
    ff:=dk_dot(f,f);rr:=ff;goal:=DK_SOLVE_TOL*DK_SOLVE_TOL*ff;steps:=0;
    while rr>goal && steps<cap {
        dk_apply(n,ax,ay,p[0..<cells],ap[0..<cells]);
        alpha:=rr/dk_dot(p[0..<cells],ap[0..<cells]);
        for t:0..<cells {u[t]+=alpha*p[t];r[t]-=alpha*ap[t];}
        next:=dk_dot(r[0..<cells],r[0..<cells]);beta:=next/rr;rr=next;
        for t:0..<cells {p[t]=r[t]+beta*p[t];}
        steps+=1;
    }
    residual:=0.0;if ff>0 {residual=sqrt(rr/ff);}
    if rr>goal {return DkSolve.{status=DkStatus.converge,iterations=steps,residual=residual};}
    return DkSolve.{iterations=steps,residual=residual};
}
```

The matrix is symmetric, and it is positive definite when every face
coefficient is positive, which is what conjugate gradients needs. The solver
stops when the residual is at most 10⁻¹² of f, or refuses with `converge`
once it has taken the caller's cap of iterations without getting there. The
example and the data generator pass a cap of 2,000; a 16 × 16 grid here takes
at most 195.

<!-- source: psionic/lib/darcy.coax check -->
```jai
dk_value_ok :: (v:[]f64)->bool {
    for x:v {if !sf_finite(x) || abs(x)>FO_VALUE_MAX {return false;}}
    return true;
}
// Grid size, lengths, finite values within 1e6, then coefficients of at
// least 1e-6 (a face of zero or less would leave the matrix singular or
// indefinite).
dk_check :: (n:int,ax:[]f64,ay:[]f64,f:[]f64,u:[]f64)->DkStatus {
    if n<1 || n>DK_N_MAX {return DkStatus.size;}
    if ax.count!=(n+1)*n || ay.count!=(n+1)*n || f.count!=n*n || u.count!=n*n {return DkStatus.shape;}
    if !dk_value_ok(ax) || !dk_value_ok(ay) || !dk_value_ok(f) {return DkStatus.number;}
    for x:ax {if x<1e-6 {return DkStatus.coefficient;}}
    for x:ay {if x<1e-6 {return DkStatus.coefficient;}}
    return DkStatus.ok;
}
```

## Convergence under refinement

A hand calculation pins the scheme on tiny grids; a manufactured solution
checks it on fine ones. Pick the answer first: u = sin(πx) sin(πy), which is
zero on the boundary, and a = 1 + xy. Differentiating gives the source that
produces it:

f = 2π²(1 + xy) u − π(y cos(πx) sin(πy) + x sin(πx) cos(πy)).

The oracle in the checks file builds the faces and f from these formulas
with `sin` and `cos`, solves, and returns the largest nodal error. A
second-order scheme's error falls by 4 each time h halves. On n = 7, 15 and
31 (h = 1/8, 1/16, 1/32) the errors are 0.01291, 0.003209 and 0.000801, so
the observed orders log₂(e₁/e₂) and log₂(e₂/e₃) are 2.009 and 2.002. A
`#test` requires both to lie between 1.95 and 2.05.

A separate check applies the operator node by node, with each face looked
up by its coordinates, to the solver's answer on a 6 × 6 grid with uneven
coefficients and recovers f within 10⁻¹⁰.

## The data

A permeability field is nine numbers. Draw nine uniform values in [−1, 1)
from the counter generator (`cr_draw`, splitmix64) with the field's seed, and
damp mode (k, l) by 4/(k² + l²) so the smooth modes dominate:

g(x, y) = Σ over k, l in 1..3 of c_kl cos(kπx) cos(lπy), a = exp(g).

Because the field is a formula, it can be sampled on any grid. The solver
gets a at the face midpoints. The network gets log a = g at the nodes, plus
each node's x and y, as three input channels.

<!-- source: psionic/lib/darcy.coax field -->
```jai
DK_FIELD_SCALE :: 4.0;
// Nine uniform draws in [-1, 1) from the counter generator, damped by
// 1/(k^2 + l^2) so the low modes dominate.
dk_field :: (seed:u64)->DkField {
    f:DkField;
    for t:0..<9 {
        d:=cr_draw(CrSeed.{seed=seed,counter=cast(u64)t});
        k:=t/3+1;l:=t%3+1;
        f.c[t]=(2*d.value-1)*DK_FIELD_SCALE/f64(k*k+l*l);
    }
    return f;
}
// log a at (x, y).
dk_log_a :: (f:DkField,x:f64,y:f64)->f64 {
    g:=0.0;
    for t:0..<9 {g+=f.c[t]*cos(f64(t/3+1)*DK_PI*x)*cos(f64(t%3+1)*DK_PI*y);}
    return g;
}
// One example on an n x n grid: the network's input (log a, x and y at every
// node, channel-major) and the solver's u for f = 1. a is sampled at the
// face midpoints for the solver and at the nodes for the network.
dk_sample :: (field:DkField,n:int,input:[]f64,u:[]f64,cap:int)->DkSolve {
    if n<1 || n>DK_N_MAX {return DkSolve.{status=DkStatus.size};}
    cells:=n*n;if input.count!=DK_IN*cells || u.count!=cells {return DkSolve.{status=DkStatus.shape};}
    h:=1.0/f64(n+1);ax:[DK_CELLS_MAX+DK_N_MAX]f64;ay:[DK_CELLS_MAX+DK_N_MAX]f64;f:[DK_CELLS_MAX]f64;
    faces:=(n+1)*n;
    for t:0..<faces {i:=t%(n+1);j:=t/(n+1);ax[t]=exp(dk_log_a(field,(f64(i)+0.5)*h,f64(j+1)*h));}
    for t:0..<faces {i:=t%n;j:=t/n;ay[t]=exp(dk_log_a(field,f64(i+1)*h,(f64(j)+0.5)*h));}
    for j:0..<n {for i:0..<n {
        t:=j*n+i;x:=f64(i+1)*h;y:=f64(j+1)*h;f[t]=1;
        input[t]=dk_log_a(field,x,y);input[cells+t]=x;input[2*cells+t]=y;
    }}
    return dk_solve(n,ax[0..<faces],ay[0..<faces],f[0..<cells],u,cap);
}
```

Training fields come from seeds 1000 upward and held-out fields from seeds
3000 upward, so the two sets never share a field. Seeds 2000 upward were
used as a validation set while choosing the network's size and training
schedule; the held-out seeds were not used for any choice.

## The network

<!-- source: psionic/lib/darcy.coax types -->
```jai
DkStatus :: enum u8 {ok;size;shape;number;coefficient;converge;network;}
// The solver's answer: its status, the conjugate-gradient iterations it
// took and the final residual relative to the right-hand side.
DkSolve :: struct {status:DkStatus;iterations:int;residual:f64;}
// A seeded permeability field: a(x, y) = exp(g(x, y)) with
// g = sum over k, l in 1..3 of c[3(k-1)+l-1] cos(k pi x) cos(l pi y).
DkField :: struct {c:[9]f64;}
// The network: lift 3 -> width, layers Fourier layers width -> width
// keeping modes per direction, then width -> hidden -> 1 pointwise.
DkNet :: struct {width:int;layers:int;modes:int;hidden:int;}
```

<!-- source: psionic/lib/darcy.coax bounds -->
```jai
DK_N_MAX :: 32;                  // interior points a side
DK_CELLS_MAX :: 1024;            // 32 x 32
DK_IN :: 3;                      // input channels: log a, x, y
DK_WIDTH_MAX :: 4;               // chapter 156's channel limit
DK_LAYERS_MAX :: 4;
DK_HIDDEN_MAX :: 16;
DK_SOLVE_TOL :: 1e-12;           // relative residual where the solver stops
DK_U_SCALE :: 10.0;              // the network predicts 10 u
```

The network has three parts:

1. **Lift.** At every node, a linear map from the 3 input channels to 4
   channels: h₀ = W·(log a, x, y) + b.
2. **Fourier layers.** Three of chapter 156's layers, 4 channels in and out,
   keeping the signed frequencies in (−6, 6) in each direction (121 modes).
   GELU follows the first two; the last has no activation.
3. **Projection.** At every node, z = Q₁h + c₁ with 16 hidden units, then
   out = Q₂ · GELU(z) + c₂.

The output is 10u: pressures here are about 0.07, and scaling the target to
about 0.7 keeps the loss and the weights in a comfortable range. All
parameters live in one array so that the optimizer can step them together;
`dk_layer` gives chapter 156 a view of one layer's slice.

<!-- source: psionic/lib/darcy.coax net -->
```jai
DK_PARAMS_MAX :: 30000;
dk_net_ok :: (net:DkNet)->bool {
    return net.width>=1 && net.width<=DK_WIDTH_MAX && net.layers>=1 && net.layers<=DK_LAYERS_MAX
        && net.modes>=1 && net.modes<=FO_MODES_MAX && net.hidden>=1 && net.hidden<=DK_HIDDEN_MAX;
}
dk_kept :: (net:DkNet)->int {return (2*net.modes-1)*(2*net.modes-1);}
dk_layer_size :: (net:DkNet)->int {w:=net.width;return 2*w*w*dk_kept(net)+w*w+w;}
// Parameters, in order: lift weights (width x 3) and biases; each Fourier
// layer's wr, wi, lin and bias; the projection's first weights
// (hidden x width) and biases, its second weights and its one bias.
dk_params :: (net:DkNet)->int {return 4*net.width+net.layers*dk_layer_size(net)+net.hidden*net.width+2*net.hidden+1;}
dk_proj :: (net:DkNet)->int {return 4*net.width+net.layers*dk_layer_size(net);}
// Fourier layer l as a view of p: GELU on every layer but the last.
dk_layer :: (net:DkNet,p:[]f64,l:int)->FoLayer {
    w:=net.width;m:=w*w*dk_kept(net);at:=4*w+l*dk_layer_size(net);
    act:=FoAct.gelu;if l==net.layers-1 {act=FoAct.none;}
    return FoLayer.{cin=w,cout=w,k1=net.modes,k2=net.modes,act=act,wr=p[at..<at+m],wi=p[at+m..<at+2*m],
        lin=p[at+2*m..<at+2*m+w*w],bias=p[at+2*m+w*w..<at+2*m+w*w+w]};
}
dk_grad :: (net:DkNet,g:[]f64,l:int)->FoGrad {
    v:=dk_layer(net,g,l);return FoGrad.{wr=v.wr,wi=v.wi,lin=v.lin,bias=v.bias};
}
// Working storage for one example: every layer's input and pre-activation,
// the projection's hidden values, the output and the backward buffers.
dk_work :: (net:DkNet,cells:int)->int {return ((2*net.layers+3)*net.width+2*net.hidden+1)*cells;}
```

With width 4, 3 layers, 6 modes and 16 hidden units the network has 11,789
parameters, almost all of them the 2 × 4 × 4 × 121 = 3,872 spectral weights
in each layer.

<!-- source: psionic/lib/darcy.coax init -->
```jai
// Lift, lin and projection weights uniform in +-1/sqrt(fan in); spectral
// weights uniform in +-1/width^2; biases zero.
dk_init :: (net:DkNet,p:[]f64,seed:u64) {
    w:=net.width;proj:=dk_proj(net);m:=w*w*dk_kept(net);
    for t:0..<p.count {
        d:=cr_draw(CrSeed.{seed=seed,counter=cast(u64)t});u:=2*d.value-1;scale:=1.0;
        if t<3*w {scale=1/sqrt(3.0);}
        else if t<4*w {scale=0;}
        else if t<proj {
            r:=(t-4*w)%dk_layer_size(net);
            if r<2*m {scale=1/f64(w*w);} else if r<2*m+w*w {scale=1/sqrt(f64(w));} else {scale=0;}
        }
        else if t<proj+net.hidden*w {scale=1/sqrt(f64(w));}
        else if t<proj+net.hidden*(w+1) {scale=0;}
        else if t<proj+net.hidden*(w+2) {scale=1/sqrt(f64(net.hidden));}
        else {scale=0;}
        p[t]=u*scale;
    }
}
```

<!-- source: psionic/lib/darcy.coax forward -->
```jai
// Block k of a working store cut into blocks of size b.
dk_block :: (ws:[]f64,k:int,b:int)->[]f64 {return ws[k*b..<k*b+b];}
// The network's output at every node, 10 u, written to ws at dk_out.
dk_out :: (net:DkNet,cells:int)->int {return ((2*net.layers+1)*net.width+net.hidden)*cells;}
dk_forward :: (net:DkNet,p:[]f64,n:int,input:[]f64,ws:[]f64)->DkStatus {
    status:=dk_admit(net,p,n,input,ws);if status!=DkStatus.ok {return status;}
    w:=net.width;L:=net.layers;cells:=n*n;wc:=w*cells;
    for c:0..<w {for t:0..<cells {
        v:=p[3*w+c];for i:0..<DK_IN {v+=p[c*DK_IN+i]*input[i*cells+t];}
        ws[c*cells+t]=v;
    }}
    for l:0..<L {
        h:=dk_block(ws,l,wc);pre:=dk_block(ws,L+1+l,wc);y:=dk_block(ws,l+1,wc);
        if fo_forward(dk_layer(net,p,l),n,n,h,pre,y)!=FoStatus.ok {return DkStatus.number;}
    }
    proj:=dk_proj(net);hid:=net.hidden;z:=(2*L+1)*wc;out:=dk_out(net,cells);
    for t:0..<cells {ws[out+t]=p[proj+hid*(w+2)];}
    for k:0..<hid {for t:0..<cells {
        v:=p[proj+hid*w+k];for c:0..<w {v+=p[proj+k*w+c]*ws[L*wc+c*cells+t];}
        ws[z+k*cells+t]=v;ws[out+t]+=p[proj+hid*(w+1)+k]*fo_act(FoAct.gelu,v);
    }}
    return DkStatus.ok;
}
// The network's shape, the grid (it must hold the kept modes), the lengths,
// then every parameter and input value.
dk_admit :: (net:DkNet,p:[]f64,n:int,input:[]f64,ws:[]f64)->DkStatus {
    if !dk_net_ok(net) {return DkStatus.network;}
    if n<1 || n>DK_N_MAX || 2*net.modes-1>n {return DkStatus.size;}
    if p.count!=dk_params(net) || input.count!=DK_IN*n*n || ws.count<dk_work(net,n*n) {return DkStatus.shape;}
    if !dk_value_ok(p) || !dk_value_ok(input) {return DkStatus.number;}
    return DkStatus.ok;
}
```

## The backward pass

The gradient runs the forward pass in reverse. Write d = dL/dout at each
node. The projection is two pointwise layers, so its gradients are sums over
the nodes: dL/dc₂ = Σ d, dL/dQ₂ₖ = Σ d · GELU(zₖ), and with
dzₖ = d · Q₂ₖ · GELU′(zₖ), dL/dc₁ₖ = Σ dzₖ and dL/dQ₁ₖ꜀ = Σ dzₖ h꜀. The
gradient flowing into the last Fourier layer is Σₖ Q₁ₖ꜀ dzₖ. Each Fourier
layer then hands its input gradient to the layer before it through chapter
156's `fo_backward`, which also writes that layer's weight gradients
straight into the right slice of the gradient array. The lift is the
projection's first step again.

<!-- source: psionic/lib/darcy.coax backward -->
```jai
// After dk_forward on the same input, write dL/dp to g given dL/dout.
// Every entry of g is overwritten.
dk_backward :: (net:DkNet,p:[]f64,n:int,input:[]f64,ws:[]f64,dout:[]f64,g:[]f64)->DkStatus {
    status:=dk_admit(net,p,n,input,ws);if status!=DkStatus.ok {return status;}
    cells:=n*n;if dout.count!=cells || g.count!=p.count {return DkStatus.shape;}
    if !dk_value_ok(dout) {return DkStatus.number;}
    w:=net.width;L:=net.layers;wc:=w*cells;proj:=dk_proj(net);hid:=net.hidden;
    z:=(2*L+1)*wc;a:=dk_out(net,cells)+cells;dz:=a+2*wc;
    s:=0.0;for t:0..<cells {s+=dout[t];}g[proj+hid*(w+2)]=s;
    for k:0..<hid {
        q2:=p[proj+hid*(w+1)+k];sq:=0.0;sb:=0.0;
        for t:0..<cells {
            v:=ws[z+k*cells+t];sq+=dout[t]*fo_act(FoAct.gelu,v);
            d:=dout[t]*q2*fo_slope(FoAct.gelu,v);ws[dz+k*cells+t]=d;sb+=d;
        }
        g[proj+hid*(w+1)+k]=sq;g[proj+hid*w+k]=sb;
        for c:0..<w {sc:=0.0;for t:0..<cells {sc+=ws[dz+k*cells+t]*ws[L*wc+c*cells+t];}g[proj+k*w+c]=sc;}
    }
    for c:0..<w {for t:0..<cells {
        v:=0.0;for k:0..<hid {v+=p[proj+k*w+c]*ws[dz+k*cells+t];}
        ws[a+c*cells+t]=v;
    }}
    // Layer l's output gradient is in buffer (L - 1 - l) mod 2 after a; its
    // input gradient goes to the other one.
    for r:0..<L {
        l:=L-1-r;i:=a+(r%2)*wc;o:=a+((r+1)%2)*wc;dy:=ws[i..<i+wc];dx:=ws[o..<o+wc];
        if fo_backward(dk_layer(net,p,l),n,n,dk_block(ws,l,wc),dk_block(ws,L+1+l,wc),dy,dx,dk_grad(net,g,l))!=FoStatus.ok {return DkStatus.number;}
    }
    a+=(L%2)*wc;
    for c:0..<w {
        sb:=0.0;for t:0..<cells {sb+=ws[a+c*cells+t];}g[3*w+c]=sb;
        for i:0..<DK_IN {si:=0.0;for t:0..<cells {si+=ws[a+c*cells+t]*input[i*cells+t];}g[c*DK_IN+i]=si;}
    }
    return DkStatus.ok;
}
```

A `#test` checks every one of these gradients against a central difference
of the loss with step 10⁻⁶, on three networks from 1 to 3 channels wide,
1 to 3 layers, on 3 × 3 and 4 × 4 grids. Every parameter of each network is
compared, and the worst disagreement must be at most 10⁻⁷ relative to
1 + |g|.

## Training

The loss is the mean squared error against 10u over the nodes. Training
walks the training set in a fixed order in minibatches of 8, averages the
batch's gradients and takes one AdamW step (β₁ = 0.9, β₂ = 0.999, ε = 10⁻⁸,
weight decay 10⁻⁴). The learning rate starts at 0.01 and halves after each
quarter of the epochs.

<!-- source: psionic/lib/darcy.coax train -->
```jai
// Mean squared error against 10 u over the nodes, and its gradient.
dk_loss :: (out:[]f64,u:[]f64,dout:[]f64)->f64 {
    s:=0.0;n:=f64(out.count);
    for t:0..<out.count {e:=out[t]-DK_U_SCALE*u[t];s+=e*e;dout[t]=2*e/n;}
    return s/n;
}
DkTrain :: struct {epochs:int;batch:int;lr:f64;decay:f64;}
// The learning rate halves after each quarter of the epochs.
dk_rate :: (cfg:DkTrain,epoch:int)->f64 {return cfg.lr*pow(0.5,f64((4*epoch)/cfg.epochs));}
// Minibatches in a fixed order; the gradient is the batch mean, then one
// AdamW step. losses[e] is epoch e's mean training loss. m and v are AdamW's
// moments, zero at the start.
dk_train :: (net:DkNet,p:[]f64,m:[]f64,v:[]f64,n:int,inputs:[]f64,targets:[]f64,cfg:DkTrain,losses:[]f64)->DkStatus {
    cells:=n*n;count:=targets.count/cells;
    if cfg.epochs<1 || cfg.batch<1 || losses.count!=cfg.epochs || count<1 || targets.count!=count*cells || inputs.count!=DK_IN*count*cells {return DkStatus.shape;}
    ws:[(2*DK_LAYERS_MAX+3)*DK_WIDTH_MAX*DK_CELLS_MAX+(2*DK_HIDDEN_MAX+1)*DK_CELLS_MAX]f64;
    g:[DK_PARAMS_MAX]f64;sum:[DK_PARAMS_MAX]f64;dout:[DK_CELLS_MAX]f64;none:[1]f32;
    P:=p.count;state:AwState;out:=dk_out(net,cells);
    for e:0..<cfg.epochs {
        total:=0.0;group:=[1]AwGroup.[AwGroup.{first=0,count=P,lr=dk_rate(cfg,e),beta1=0.9,beta2=0.999,eps=1e-8,decay=cfg.decay}];
        start:=0;
        while start<count {
            left:=count-start;size:=cast(int)((f64(cfg.batch+left)-abs(f64(cfg.batch-left)))/2);   // the smaller, without a tie
            for t:0..<P {sum[t]=0;}
            for s:start..<start+size {
                x:=inputs[s*DK_IN*cells..<(s+1)*DK_IN*cells];
                status:=dk_forward(net,p,n,x,ws[..]);if status!=DkStatus.ok {return status;}
                total+=dk_loss(ws[out..<out+cells],targets[s*cells..<(s+1)*cells],dout[0..<cells]);
                status=dk_backward(net,p,n,x,ws[..],dout[0..<cells],g[0..<P]);if status!=DkStatus.ok {return status;}
                for t:0..<P {sum[t]+=g[t]/f64(size);}
            }
            r:=aw_step(group[..],AwPolicy.{},state,p,sum[0..<P],m,v,none[0..<0],false);
            if r.status!=AwStatus.ok {return DkStatus.number;}
            state=r.state;start+=size;
        }
        losses[e]=total/f64(count);
    }
    return DkStatus.ok;
}
```

Two checks pin the loop. The first writes the same loop out with direct
`aw_step` calls, batch by batch, and requires the parameters, both moment
arrays and every epoch's loss to agree exactly, for three batch sizes and
schedules. The second uses a property of Adam's first step: from zero
moments, with bias correction, each weight moves by lr · g/(|g| + ε) after
decay. So after one full batch of two examples with lr = 0.01 and decay 0.5,
each weight must be p(1 − 0.005) − 0.01 · ḡ/(|ḡ| + 10⁻⁸), where ḡ is the
mean of the two examples' gradients computed separately.

The error reported is the relative L2 error per field,
‖u_net − u‖ / ‖u‖ over the nodes, averaged over the held-out fields.

<!-- source: psionic/lib/darcy.coax error -->
```jai
// The relative L2 error ||u_net - u|| / ||u|| on one example.
dk_error :: (net:DkNet,p:[]f64,n:int,input:[]f64,u:[]f64,ws:[]f64)->f64 {
    if dk_forward(net,p,n,input,ws)!=DkStatus.ok {return -1;}
    out:=dk_out(net,n*n);e:=0.0;s:=0.0;
    for t:0..<n*n {d:=ws[out+t]/DK_U_SCALE-u[t];e+=d*d;s+=u[t]*u[t];}
    return sqrt(e/s);
}
```

## A longer run

The acceptance threshold was fixed before the held-out seeds were used: on
the 64 held-out fields at the training grid of 16 × 16, the **mean relative
L2 error must be at most 0.05**. The validation fields reached about 0.027
during the choice of network, so 0.05 leaves room for a different set of
fields without being loose enough to pass a model that has not learned the
map.

The run trains on 128 fields for 300 epochs (4,800 AdamW steps) and takes
about a minute in a release build. Its program is
[`darcy-long.coax`](../tests/darcy-long.coax), and its output is kept in
[`darcy-long.out`](../fixtures/darcy-long.out):

```shell
coax/bin/coax run psionic/tests/darcy-long.coax --release
```

```text
training: seeds 1000 to 1127, 16 x 16, 300 epochs of 16 steps, batch 8, rate 0.01 halved each quarter
network: width 4, 3 Fourier layers, 6 modes, projection 16; 11789 parameters
  epoch 0: loss 0.07332
  epoch 50: loss 0.0001204
  epoch 100: loss 0.00005819
  epoch 150: loss 0.00004202
  epoch 200: loss 0.00003846
  epoch 250: loss 0.00003371
  epoch 299: loss 0.00003083
training relative L2 at 16 x 16: mean 0.0127
held-out (seeds 3000 to 3063) relative L2 at 16 x 16: mean 0.0164, worst 0.0313
same weights at 32 x 32: mean 0.0685, worst 0.0841
threshold: mean at 16 x 16 at most 0.05: pass
```

The mean held-out error at 16 × 16 is 0.0164, so the run passes the
threshold. The training fields reach 0.0127, so the gap between fields the
network has seen and fields it has not is small: with 11,789 parameters and
128 fields of 256 nodes it has not simply memorised the training set.

**Resolution transfer.** The same weights evaluated on the same held-out
fields sampled at 32 × 32, against the solver at 32 × 32, give a mean error of
0.0685: about four times worse than at the training grid. Chapter 156 showed
that a Fourier layer gives the same values at two resolutions when its input
is band-limited and periodic. Neither holds here: log a, x and y are not
periodic on the square (x runs from near 0 to near 1 and jumps back), so
their transforms have energy at every frequency, and the solution's zero
boundary sits at a different distance from the first node on each grid
(h = 1/17 versus 1/33). The pointwise paths, which see the grid's own nodes,
learned the 16 × 16 version of those details. Zero-shot super-resolution is
therefore partial here: the prediction at 32 × 32 is recognisably the right
field, and its error is several times what the training grid gives. Padding,
or training on several resolutions, are the usual remedies; neither is done
in this chapter.

## Refusals

The solver checks, in order: the grid size (1 to 32), the lengths of every
array, that every value is finite and at most 10⁶ in magnitude, and that
every face coefficient is at least 10⁻⁶. A coefficient of zero or less
would make the matrix singular or indefinite, and conjugate gradients would
not be solving the equation at all. Nothing is written before every check
passes.

The network checks its shape (width, layers, modes, hidden units), then the
grid, which must also hold the kept modes (2 × modes − 1 ≤ n), then the
lengths, then the values. A Fourier layer whose input grows past 10⁶ is
refused by chapter 156's checks and reported as `number`. Training refuses a
bad schedule or data shape, and a learning rate AdamW refuses.

The last six lines of the transcript show one refusal each:

- a face coefficient of 0: `coefficient`;
- a 33 × 33 grid: `size`;
- 9 modes, more than chapter 156's 8: `network`;
- 6 modes on an 8 × 8 grid, which holds at most 4: `size`;
- a NaN in the input: `number`;
- a learning rate of 2: `number`.

## How it is checked

The checks in [`darcy.coax`](../lib/darcy.coax) come before the code they
test. The oracles in [`darcy-checks.coax`](../tests/darcy-checks.coax) do not
call the solver's own operator:

- **Hand values.** One and nine nodes, as worked above, and a face of 3
  among ones: u = 1/(4 · 6).
- **A manufactured solution** at three grid sizes, with second-order
  convergence.
- **The operator node by node** on a 6 × 6 grid with uneven coefficients.
- **The iteration cap**: a cap of 0, of 1 and of one fewer than needed
  refuses with `converge` and the iterations it took; the exact number
  needed succeeds.
- **A field and a sample by hand**: the coefficients against the counter
  generator's published first draw, log a at three points, and a 5 × 5 sample
  whose u satisfies the node-by-node operator.
- **The network by hand**: the parameter layout of a small network, every
  initial weight, and a one-node network whose output is worked out with
  plain arithmetic.
- **Finite differences** for every gradient, and the two training checks
  above.

`coax mutate psionic/lib/darcy.coax --root-only` changes the library's own
code one site at a time and reruns the checks. Of 763 mutants, all 763 are
caught. The oracles, the finite-difference driver and the training replay
live in the checks file, so they are not mutated themselves.

The library is not an `#authority` file: it decides no money, admission or
policy.

## What this does not do

- It is not the PhysicsNeMo recipe. The fields here are smooth, the grids
  are 16 × 16 and 32 × 32 rather than PhysicsNeMo's larger grids, and the
  network is 4 channels wide rather than tens. No PhysicsNeMo weights or data
  are used.
- The FFT treats the field as periodic, but the pressure is zero on the
  boundary and the input channels x and y are not periodic. The pointwise
  path inside each layer absorbs part of that mismatch; padding the domain,
  as larger FNO implementations do, is not done here.
- Training runs on one CPU core in binary64. There is no GPU path, no mixed
  precision and no checkpointing.
- The model does not replace the solver. It is measured against the solver,
  on fields drawn from the same distribution it was trained on.

## Sources

- [Chapter plan](PLAN.md), Part XVI.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), "Physics begins with
  our own model".
- [Chapter 156](ch156-a-spectral-neural-operator.md): the Fourier layer.
- [Chapter 124](ch124-adamw-and-explicit-precision.md): the optimizer.
- [Chapter 109](ch109-learn-a-physical-system.md): a solver-backed learning
  pattern with held-out parameters.
