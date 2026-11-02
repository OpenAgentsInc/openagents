# 160. Execute DoMINO

> **Status: partly implemented.**

DoMINO (Decomposable Multi-scale Iterative Neural Operator, from NVIDIA
PhysicsNeMo) predicts the flow around a shape from the shape alone. It reads
a surface mesh, encodes it on a grid of nodes around the body, gathers that
encoding near each point where it is asked for an answer, and evaluates a
small network on the point and its nearest surface neighbours. NVIDIA
publishes `domino_drivaerml`, a surface and a volume model trained on the
DrivAerML car dataset.

This chapter builds DoMINO's pieces at a small size, checks each against an
oracle that does not share its code, and trains them on a flow with an exact
answer. It does not run the published checkpoint; the last section says why
and what remains.

It builds five things, in [`domino.coax`](../lib/domino.coax), on top of
[chapter 158](ch158-a-field-around-a-shape.md)'s geometry:

- an **exact target**: potential flow past a spheroid, whose surface
  pressure has a closed form, and spheroid meshes to put in that flow;
- a **geometry encoding**: at every node of a grid around the body, the
  mesh's signed distance and, at two radii, the mean of a point feature over
  the surface points within that radius;
- a **local encoding**: at a query point, the mean node encoding within two
  larger radii;
- an **output network**: a 12-unit layer evaluated on the query and its three
  nearest surface points, blended by inverse distance, with its gradient
  written by hand;
- **training** with [chapter 124](ch124-adamw-and-explicit-precision.md)'s
  AdamW, and the **relative L2 error** of surface pressure on spheroids the
  training never saw, against a threshold fixed before the run.

Everything runs in binary64 on one CPU core, with sums in a fixed order. That
is the profile `psionic-domino-v1`. A grid has at most 8 nodes a side; a ball
query keeps at most the 64 nearest points; meshes and point sets keep
chapter 158's limits (1,024 triangles, 4,096 points).

## Run it

The program is [`domino.coax`](../examples/domino.coax); the checks' oracles
are in [`domino-checks.coax`](../tests/domino-checks.coax), and the longer
training receipt is [`domino-long.coax`](../tests/domino-long.coax).

```shell
coax/bin/coax run psionic/examples/domino.coax
coax/bin/coax test psionic/lib/domino.coax
coax/bin/coax mutate psionic/lib/domino.coax --root-only
coax/bin/coax run psionic/tests/domino-long.coax --release
```

```text
sphere: A0 = 0.6667, alpha = 0.5, Cp nose 1.0 waist -1.25
a = 1, b = 0.5: A0 = 0.3471, alpha = 0.21, Cp waist -0.4641
shape a = 0.55: area 3.1899, signed distance at node (-0.3, -0.3, -0.3) 0.0334
shape a = 0.85: area 4.4557, signed distance at node (-0.3, -0.3, -0.3) -0.0175
shape a = 1.15: area 5.7789, signed distance at node (-0.3, -0.3, -0.3) -0.0326
query 0 of shape 0: blend weights 0.966 0.0129 0.0123 0.0088
  step 0: loss 0.2037
  step 100: loss 0.0357
  step 200: loss 0.0109
  step 300: loss 0.009
  step 399: loss 0.0084
relative L2 error: training 0.1838 held-out 0.2101
an open mesh: mesh (open)
a triangle with a repeated corner: mesh (degenerate)
local radii 1.5 and 1.5: radius
a geometry radius of 0: radius
a query at x = 1.6, outside the box: bounds
a 9 x 9 x 9 grid: limit
```

The sections below explain each line.

## The pieces and their limits

<!-- source: psionic/lib/domino.coax types -->
```jai
DmStatus :: enum u8 {ok;shape;number;radius;bounds;mesh;limit;train;}
// side^3 nodes evenly spaced from low to high, corners included.
DmBox :: struct {low:[3]f64;high:[3]f64;side:int;}
// geometry: the two radii a node gathers surface points within; local: the
// two radii a query gathers nodes within. Each pair grows.
DmRadii :: struct {geometry:[2]f64;local:[2]f64;}
// The fixed point feature: at scale s, channel c is
// tanh(w[s][c] . (x - node) / R_s + b[s][c]).
DmEncoder :: struct {w:[24]f64;b:[8]f64;}
// cause is chapter 158's status when it decided the refusal.
DmResult :: struct {status:DmStatus;cause:FdStatus;written:int;}
DmTrain :: struct {steps:int;lr:f64;decay:f64;}
```

<!-- source: psionic/lib/domino.coax bounds -->
```jai
DM_SIDE_MAX :: 8;
DM_SCALES :: 2;
DM_C :: 4;                     // channels a scale
DM_FEAT :: 9;                  // a node: signed distance, then 2 x 4 channels
DM_KEEP :: 64;                 // at most this many points or nodes a ball
DM_STENCIL :: 4;               // surface points a query reads
DM_BASIS :: 7;                 // offset / rho_0 (3), normal (3), distance / rho_0
DM_IN :: 25;                   // 7 + 2 x 9
DM_H :: 12;
DM_PARAMS :: 325;              // 12 x 25 + 12 + 12 + 1
DM_ROW :: 104;                 // a query: 4 rows of 25, then 4 blend weights
DM_DELTA :: 0.001;             // inverse-distance weights are 1 / (d + delta)
DM_STEPS_MAX :: 100000;
```

## A flow with an exact answer

DoMINO learns from simulations, and a simulation has error of its own. To
measure the network alone, this chapter uses a flow that is known exactly:
an ideal fluid moving at speed U along x past a spheroid
x²/a² + (y² + z²)/b² = 1. For an ellipsoid in such a flow, the speed on the
surface is (1 + α) U times the part of the flow tangent to the surface, so the
pressure coefficient Cp = 1 − |v|²/U² at a point with unit normal n is

Cp = 1 − (1 + α)² (1 − n_x²).

The constant α depends only on the shape: α = A₀ / (2 − A₀), where A₀ is a
shape integral. For a spheroid with e² = 1 − b²/a² it has the closed form
A₀ = (1 − e²)/e³ (ln((1 + e)/(1 − e)) − 2e), which divides zero by zero at
e = 0. Expanding the logarithm gives a series with no such point:

A₀ = 2 (1 − e²) (1/3 + e²/5 + e⁴/7 + …).

By hand, for a sphere (a = b = 0.5): e² = 0, so A₀ = 2 × 1/3 = 2/3 and
α = (2/3) / (4/3) = 1/2. The speed peaks at 3/2 U on the waist, where
n_x = 0: Cp = 1 − (3/2)² = −1.25. At the nose, n_x = 1 and Cp = 1. Sixty
degrees from the flow, n_x² = 1/4, so Cp = 1 − 9/4 × 3/4 = −11/16.

For a spheroid twice as long (a = 1, b = 0.5): e² = 3/4, e = 0.8660, and
ln(1.8660 / 0.1340) − 1.7321 = 2.6339 − 1.7321 = 0.9018; times
(1/4) / 0.6495 = 0.3849 this is A₀ = 0.3471, so α = 0.3471 / 1.6529 = 0.21
and the waist has Cp = 1 − 1.21² = −0.4641. A longer body disturbs the flow
less. These are the program's first two lines.

<!-- source: psionic/lib/domino.coax flow -->
```jai
// Potential flow along x past the spheroid x^2/a^2 + (y^2 + z^2)/b^2 = 1,
// a >= b. With e^2 = 1 - b^2/a^2,
//   A0 = 2 (1 - e^2) (1/3 + e^2/5 + e^4/7 + ...),
// the series of (1 - e^2)/e^3 (ln((1 + e)/(1 - e)) - 2e), exact at e = 0.
// The sum runs until e2^k underflows to zero.
dm_a0 :: (a:f64,b:f64)->f64 {
    e2:=1-b*b/(a*a);sum:=0.0;term:=1.0;k:=0;add:=1.0/3;
    while add!=0 {sum+=add;term*=e2;k+=1;add=term/f64(2*k+3);}
    return 2*(1-e2)*sum;
}
// The surface speed is (1 + alpha) U times the tangential part of the flow.
dm_alpha :: (a:f64,b:f64)->f64 {a0:=dm_a0(a,b);return a0/(2-a0);}
// Cp = 1 - |v|^2 / U^2 = 1 - (1 + alpha)^2 (1 - n_x^2), with n the unit
// normal of the spheroid through p, along (x/a^2, y/b^2, z/b^2).
dm_cp_normal :: (a:f64,b:f64,n:[3]f64)->f64 {
    nx2:=n[0]*n[0]/fd_dot(n,n);k:=1+dm_alpha(a,b);return 1-k*k*(1-nx2);
}
dm_cp :: (a:f64,b:f64,p:[3]f64)->f64 {return dm_cp_normal(a,b,[3]f64.[p[0]/(a*a),p[1]/(b*b),p[2]/(b*b)]);}
```

The checks compare the series with the logarithm form for
a = 0.55 … 1.50 (to 1e-14) and with the defining integral
A₀ = a b² ∫₀^∞ ds / ((a² + s)^{3/2} (b² + s)), computed by Simpson's rule
after a change of variable (to 1e-10). Potential flow has no drag
(d'Alembert's paradox): the pressure force along x integrates to zero over
the surface, which the long receipt uses as a physical check.

The spheroid meshes start from chapter 158's octahedron. Each level cuts every
triangle into four, pushes the new corners out to the unit sphere and finally
stretches x by a and y, z by b. Level L has 8 × 4^L triangles and
4^(L+1) + 2 vertices; every level passes chapter 158's admission, and its
volume approaches 4/3 π a b² from below (level 0 holds exactly 4/3 a b²).

## The geometry encoding

DoMINO first describes the shape on a grid of nodes that covers the body. A
node holds nine numbers: the mesh's signed distance (chapter 158's
`fd_mesh_distance`), then, at each of two radii R₀ < R₁, four channels of a
point feature averaged over the surface points within that radius:

f_c(node) = mean over points x within R of tanh(w_c · (x − node)/R + b_c).

The weights w and b are drawn once from the counter generator and stay fixed;
in DoMINO they are trained, and here only the output network trains. A ball
keeps at most the 64 nearest points, found by chapter 158's grid k-nearest
search and cut where they leave the radius:

<!-- source: psionic/lib/domino.coax ball -->
```jai
// The points of g within r of q, nearest first, at most 64: the k nearest
// from chapter 158's grid, cut where they leave the ball.
dm_ball :: (g:FdGrid,q:[3]f64,r:f64,out:[]int)->int {
    keep:=fd_knn(g,q,fd_imin(DM_KEEP,g.n),out).count;
    count:=0;while count<keep && fd_d2(g.points,out[count],q)<=r*r {count+=1;}
    return count;
}
```

By hand: a node at the origin, two surface points at (0.2, 0, 0) and
(0, 0, 0.4), radii 0.25 and 0.5. Take channel 0 of each scale with
w = (1, 0, 0), b = 0. At R₀ = 0.25 only the first point is inside:
tanh(0.2/0.25) = tanh(0.8) = 0.6640. At R₁ = 0.5 both are:
(tanh(0.4) + tanh(0)) / 2 = 0.3799 / 2 = 0.1900. A channel with w = 0 and
b = 0.5 reads tanh(0.5) = 0.4621 at any point, so its mean is 0.4621. A node
no point reaches reads zeros. The check `a node's encoding by hand` asserts
these values exactly.

<!-- source: psionic/lib/domino.coax encode -->
```jai
// The point feature at node q: at each scale, the mean over the surface
// points within R_s of tanh(w . (x - q) / R_s + b), or 0 if none is.
dm_features_in :: (g:FdGrid,enc:DmEncoder,q:[3]f64,radii:[2]f64,f:[]f64) {
    idx:[64]int;
    for s:0..<DM_SCALES {
        count:=dm_ball(g,q,radii[s],idx[..]);
        for c:0..<DM_C {
            o:=s*DM_C+c;sum:=0.0;
            for j:0..<count {
                u:=fd_div(fd_sub(fd_at(g.points,idx[j]),q),radii[s]);
                sum+=dm_tanh(enc.b[o]+enc.w[3*o]*u[0]+enc.w[3*o+1]*u[1]+enc.w[3*o+2]*u[2]);
            }
            f[o]=ifx count==0 then 0.0 else sum/f64(count);
        }
    }
}
dm_point_features :: (enc:DmEncoder,cloud:[]f64,q:[3]f64,radii:[2]f64,f:[]f64)->FdStatus {
    start:[32769]int;order:[4096]int;g:=fd_grid_build(cloud,radii[1],start[..],order[..]);
    if g.status==FdStatus.ok {dm_features_in(g,enc,q,radii,f);}
    return g.status;
}
// Node i's nine numbers: the mesh's signed distance, then the point
// feature's two scales of four channels.
dm_encode :: (m:FdChecked,cloud:[]f64,box:DmBox,radii:DmRadii,enc:DmEncoder,out:[]f64)->DmResult {
    r:DmResult;
    if m.status!=FdStatus.ok {r.status=DmStatus.mesh;r.cause=m.status;return r;}
    r.status=dm_radii_check(box,radii);if r.status!=DmStatus.ok {return r;}
    nodes:=box.side*box.side*box.side;
    if out.count!=nodes*DM_FEAT {r.status=DmStatus.shape;return r;}
    start:[32769]int;order:[4096]int;g:=fd_grid_build(cloud,radii.geometry[1],start[..],order[..]);
    r.cause=g.status;r.status=dm_from(g.status);
    if g.status!=FdStatus.ok {return r;}
    for i:0..<nodes {
        p:=dm_node(box,i);out[i*DM_FEAT]=fd_mesh_distance(m,p).d;
        dm_features_in(g,enc,p,radii.geometry,out[i*DM_FEAT+1..<(i+1)*DM_FEAT]);
    }
    r.status=DmStatus.ok;r.written=nodes;return r;
}
```

The checks compare every node's nine numbers with chapter 158's distance and
with a direct loop that scans every point and picks the nearest 64 by
repeated minimum, instead of the grid.

## The local encoding and the stencil

A query is a point on the surface with its normal. Its row block holds 104
numbers. The local encoding is the mean node encoding within two radii
ρ₀ < ρ₁ of the query (18 numbers). The stencil is the query itself and its
three nearest surface points; each stencil point gets a row of 25 inputs:
its offset from the query over ρ₀, its normal, its distance over ρ₀, and the
query's local encoding. The last four numbers blend the stencil: point j
weighs (1/(d_j + δ)) / Σ, with δ = 0.001. The query, at distance 0, weighs
1/δ = 1,000 before normalizing, so it dominates. In the program, query 0's
weights are 0.966, 0.0129, 0.0123 and 0.0088.

<!-- source: psionic/lib/domino.coax rows -->
```jai
// Query k's 104 numbers: four rows of 25, one for the query itself and one
// for each of its three nearest surface points j (offset and distance over
// rho_0, the point's normal, then the local encoding: the mean node encoding
// within rho_0 and within rho_1), then the four blend weights
// (1 / (d_j + delta)) / sum.
dm_rows :: (box:DmBox,radii:DmRadii,feat:[]f64,cloud:[]f64,normals:[]f64,queries:[]f64,qnormals:[]f64,rows:[]f64)->DmResult {
    r:DmResult;r.status=dm_radii_check(box,radii);if r.status!=DmStatus.ok {return r;}
    nodes:=box.side*box.side*box.side;
    if feat.count!=nodes*DM_FEAT {r.status=DmStatus.shape;return r;}
    r.status=DmStatus.shape;
    if normals.count!=cloud.count {return r;}
    if cloud.count<3*(DM_STENCIL-1) {r.cause=FdStatus.parameter;return r;}
    nq:=queries.count/3;
    if nq==0 || queries.count!=3*nq || qnormals.count!=queries.count || rows.count!=nq*DM_ROW {return r;}
    for k:0..<nq {
        q:=fd_at(queries,k);
        if !fd_point(q) {r.status=DmStatus.number;return r;}
        for d:0..<3 {if q[d]<box.low[d] || q[d]>box.high[d] {r.status=DmStatus.bounds;return r;}}
    }
    at:[1536]f64;for i:0..<nodes {p:=dm_node(box,i);for d:0..<3 {at[3*i+d]=p[d];}}
    ns:[32769]int;no:[512]int;ng:=fd_grid_build(at[0..<3*nodes],radii.local[1],ns[..],no[..]);
    cs:[32769]int;co:[4096]int;cg:=fd_grid_build(cloud,radii.geometry[1],cs[..],co[..]);
    r.cause=ng.status;if r.cause==FdStatus.ok {r.cause=cg.status;}
    r.status=dm_from(r.cause);if r.cause!=FdStatus.ok {return r;}
    for k:0..<nq {
        q:=fd_at(queries,k);row:=rows[k*DM_ROW..<(k+1)*DM_ROW];
        local:[18]f64;idx:[64]int;
        for t:0..<2 {
            count:=dm_ball(ng,q,radii.local[t],idx[..]);
            for c:0..<DM_FEAT {
                sum:=0.0;for j:0..<count {sum+=feat[idx[j]*DM_FEAT+c];}
                local[t*DM_FEAT+c]=ifx count==0 then 0.0 else sum/f64(count);
            }
        }
        near:=fd_knn(cg,q,DM_STENCIL-1,idx[1..<64]).count;total:=0.0;
        for j:0..<near+1 {
            x:=q;n:=fd_at(qnormals,k);
            if j>0 {x=fd_at(cloud,idx[j]);n=fd_at(normals,idx[j]);}
            u:=fd_div(fd_sub(x,q),radii.local[0]);o:=j*DM_IN;
            for d:0..<3 {row[o+d]=u[d];row[o+3+d]=n[d];}
            row[o+6]=fd_norm(u);
            for c:0..<18 {row[o+DM_BASIS+c]=local[c];}
            w:=1/(fd_norm(fd_sub(x,q))+DM_DELTA);row[DM_STENCIL*DM_IN+j]=w;total+=w;
        }
        for j:0..<DM_STENCIL {row[DM_STENCIL*DM_IN+j]/=total;}
    }
    r.status=DmStatus.ok;r.written=nq;return r;
}
```

Every query must lie inside the grid's box, corners included; a query
outside it is refused with `bounds` before anything is written. The checks
compare whole row blocks, for 61 queries, with a direct loop.

## The output network

Each stencil row x_j goes through one layer of 12 tanh units and a linear
output, and the prediction is the blend:

f_j = b₂ + Σ_h w₂[h] tanh(b₁[h] + W₁[h] · x_j),  y = Σ_j a_j f_j.

That is 12 × 25 + 12 + 12 + 1 = 325 parameters. DoMINO calls the per-point
network its basis functions and the blend its aggregation; here both are one
small layer.

By hand: one unit reads input 0 with weight 1, its output weight is 2 and the
output bias 0.5. Two stencil rows have input 0.5 and −1 and weights 3/4 and
1/4:

y = 3/4 (2 tanh 0.5 + 0.5) + 1/4 (2 tanh(−1) + 0.5)
  = 3/4 × 1.4242 + 1/4 × (−1.0232) = 1.0682 − 0.2558 = 0.8124.

<!-- source: psionic/lib/domino.coax net -->
```jai
// Parameters: W1 (12 x 25, row-major) at 0, b1 at 300, w2 at 312, b2 at 324.
// Each stencil row j gives f_j = b2 + sum_h w2[h] tanh(b1[h] + W1[h] . x_j);
// the prediction is sum_j a_j f_j with the row's blend weights a_j.
dm_init :: (p:[]f64,seed:u64) {
    for t:0..<DM_PARAMS {
        u:=2*cr_draw(CrSeed.{seed=seed,counter=cast(u64)t}).value-1;
        p[t]=0;
        if t<300 {p[t]=u/5;}
        if t>=312 && t<324 {p[t]=u/sqrt(12.0);}
    }
}
dm_encoder :: (seed:u64)->DmEncoder {
    e:DmEncoder;
    for i:0..<24 {e.w[i]=2*cr_draw(CrSeed.{seed=seed,counter=cast(u64)(1000+i)}).value-1;}
    for i:0..<8 {e.b[i]=(2*cr_draw(CrSeed.{seed=seed,counter=cast(u64)(1024+i)}).value-1)/2;}
    return e;
}
// Unit h on the stencil row that starts at row[o].
dm_hidden :: (p:[]f64,row:[]f64,o:int,h:int)->f64 {
    z:=p[300+h];for i:0..<DM_IN {z+=p[h*DM_IN+i]*row[o+i];}
    return dm_tanh(z);
}
dm_forward :: (p:[]f64,row:[]f64)->f64 {
    y:=0.0;
    for j:0..<DM_STENCIL {
        f:=p[324];
        for h:0..<DM_H {f+=p[312+h]*dm_hidden(p,row,j*DM_IN,h);}
        y+=row[DM_STENCIL*DM_IN+j]*f;
    }
    return y;
}
// Adds dy times the gradient of the prediction to g.
dm_backward :: (p:[]f64,row:[]f64,dy:f64,g:[]f64) {
    for j:0..<DM_STENCIL {
        o:=j*DM_IN;da:=dy*row[DM_STENCIL*DM_IN+j];g[324]+=da;
        for h:0..<DM_H {
            t:=dm_hidden(p,row,o,h);g[312+h]+=da*t;
            dz:=da*p[312+h]*(1-t*t);g[300+h]+=dz;
            for i:0..<DM_IN {g[h*DM_IN+i]+=dz*row[o+i];}
        }
    }
}
```

The gradient is checked against central differences on every seventh
parameter for six seeds, and against itself: two calls add up to twice one.

## Training and held-out error

The program encodes eight spheroids with b = 0.5 and a = 0.55, 0.65, …,
1.25, takes 32 area-uniform queries on each, and labels each query with the
pressure law read at the mesh's own normal. Labelling at the facet normal,
not the smooth body's, keeps the mesh's faceting from limiting the error: the
question is whether the network can learn how the pressure depends on the
shape. Four held-out spheroids, a = 0.6, 0.8, 1.0 and 1.2, use other seeds.
After 400 full-batch AdamW steps the relative L2 error,
sqrt(Σ (y − t)² / Σ t²), is 0.1838 on the training queries and 0.2101 on
the held-out ones. Queries are drawn uniformly by area, so these plain sums
are area-weighted.

<!-- source: psionic/lib/domino.coax train -->
```jai
// The learning rate halves after each quarter of the steps.
dm_rate :: (cfg:DmTrain,s:int)->f64 {return cfg.lr*pow(0.5,f64((4*s)/cfg.steps));}
// Full-batch steps of chapter 124's AdamW on the mean squared error.
// losses[s] is the loss before step s. A refused AdamW step leaves the
// weights as they were.
dm_train :: (p:[]f64,m:[]f64,v:[]f64,rows:[]f64,targets:[]f64,cfg:DmTrain,losses:[]f64)->DmStatus {
    n:=targets.count;
    if cfg.steps<1 || cfg.steps>DM_STEPS_MAX || losses.count!=cfg.steps || n<1 || rows.count!=n*DM_ROW {return DmStatus.shape;}
    if p.count!=DM_PARAMS || m.count!=DM_PARAMS || v.count!=DM_PARAMS {return DmStatus.shape;}
    for x:rows {if !fd_finite(x) {return DmStatus.number;}}
    for x:targets {if !fd_finite(x) {return DmStatus.number;}}
    g:[325]f64;none:[1]f32;state:AwState;
    for s:0..<cfg.steps {
        for t:0..<DM_PARAMS {g[t]=0;}
        loss:=0.0;
        for k:0..<n {
            row:=rows[k*DM_ROW..<(k+1)*DM_ROW];e:=dm_forward(p,row)-targets[k];
            loss+=e*e/f64(n);dm_backward(p,row,2*e/f64(n),g[..]);
        }
        losses[s]=loss;
        group:=[1]AwGroup.[AwGroup.{first=0,count=DM_PARAMS,lr=dm_rate(cfg,s),beta1=0.9,beta2=0.999,eps=1e-8,decay=cfg.decay}];
        r:=aw_step(group[..],AwPolicy.{},state,p,g[..],m,v,none[0..<0],false);
        if r.status!=AwStatus.ok {return DmStatus.train;}
        state=r.state;
    }
    return DmStatus.ok;
}
```

Before the longer run first touched its held-out shapes, two thresholds were
written into it and into this chapter: the mean held-out error must be at
most 0.20, and it must be below the best **geometry-blind** fit, one constant
K for every shape in Cp = 1 − K (1 − n_x²), fitted by least squares on the
training labels. A network that ignored the encodings could do no better
than that fit.

The receipt [`domino-long.coax`](../tests/domino-long.coax) trains on 16
spheroids (a = 0.55, 0.60, …, 1.30) with 64 queries each for 3,000 steps and
measures 8 held-out spheroids (a = 0.575, 0.675, …, 1.275). Its output is
[`domino-long.out`](../fixtures/domino-long.out):

```text
16 training shapes, 64 queries each, 3000 steps
  step 0: loss 0.1723
  step 500: loss 0.0042
  step 1000: loss 0.0031
  step 1500: loss 0.0025
  step 2000: loss 0.0022
  step 2500: loss 0.0021
  step 2999: loss 0.0019
training relative L2: 0.0942; geometry-blind K = 1.5585
  held-out a = 0.575: alpha 0.4222, error 0.0982 (smooth body 0.2572), geometry-blind 0.4936, pressure drag: network -0.0422, labels -0.029
  held-out a = 0.675: alpha 0.3466, error 0.0553 (smooth body 0.1806), geometry-blind 0.3433, pressure drag: network 0.0891, labels 0.094
  held-out a = 0.775: alpha 0.2914, error 0.1383 (smooth body 0.3171), geometry-blind 0.1846, pressure drag: network 0.0526, labels 0.0386
  held-out a = 0.875: alpha 0.2497, error 0.1127 (smooth body 0.2567), geometry-blind 0.0062, pressure drag: network -0.1317, labels -0.1355
  held-out a = 0.975: alpha 0.2171, error 0.1223 (smooth body 0.2332), geometry-blind 0.1756, pressure drag: network -0.1012, labels -0.1085
  held-out a = 1.075: alpha 0.191, error 0.203 (smooth body 0.2861), geometry-blind 0.3631, pressure drag: network 0.1567, labels 0.1141
  held-out a = 1.175: alpha 0.1698, error 0.1779 (smooth body 0.2705), geometry-blind 0.5699, pressure drag: network -0.0011, labels 0.0112
  held-out a = 1.275: alpha 0.1522, error 0.2788 (smooth body 0.3567), geometry-blind 0.7287, pressure drag: network -0.2784, labels -0.1929
held-out relative L2: mean 0.1483, worst 0.2788; geometry-blind mean 0.3581; against the smooth body 0.2698
threshold: mean at most 0.2 and below the geometry-blind fit: pass
```

The mean held-out error is 0.1483, under the 0.20 threshold and well under
the geometry-blind fit's 0.3581, so the network reads the shape from its
encodings. The fit is better on one shape, a = 0.875, whose α happens to
match its single K; it is far worse at both ends of the range. The network
is worst at a = 1.275, the longest body, at the edge of the training range:
the node balls around its waist reach less of its length. Against the smooth
spheroid's pressure, the same predictions have error 0.2698, which is mostly
the faceting of a 128-triangle mesh. (The receipt was first run, against the
same thresholds, before a change to the order in which the mesh lists its
triangles, which moves every sample; that run gave a mean of 0.1472. The
output above is the current code's.) The drag
column is the pressure force along x over the queries' area: exactly zero
for the true flow, and nonzero for both the labels and the network because 64
points are a coarse estimate of a surface integral.

## Refusals

Each piece refuses bad input with a typed status, before it writes:

- a mesh that fails chapter 158's admission: `mesh`, with chapter 158's
  reason (`open`, `degenerate`, `inside_out`, …) carried in `cause`;
- a grid with fewer than 2 or more than 8 nodes a side, or a box that is not
  a proper box: `shape` or `limit`; a coordinate that is not finite: `number`;
- radii that are not positive, not growing, or longer than the box's
  diagonal: `radius`;
- a query outside the box: `bounds`; a non-finite query or point: `number`;
- radii so small that chapter 158's grid would need more than 32 cells a
  side: `limit`, with `cause` set to chapter 158's `limit`;
- training data that is not finite, or an AdamW step that AdamW refuses (for
  example a learning rate of 2): `number` or `train`, with the weights
  unchanged.

The program's last six lines show these; the checks cover every refusal and
the edges around it.

## What remains

- **The published checkpoint.** `nvidia/domino_drivaerml` on Hugging Face
  (revision `35b1bf1edafdaa2600d16182825890cd51c07427`) holds the surface
  and volume models only as Python pickles: `DoMINO.0.501.mdlus` and
  `checkpoint.0.501.pt`, with no safetensors file. This book does not
  unpickle downloaded files. The surface model's `DoMINO.0.501.mdlus`
  (40,384,646 bytes, SHA-256
  `ad45e9477a7c0336c07a039ba70db7ddc9a2254d4ff94104d9c297ae7d7b662b`) was
  read only as a zip archive: it holds `args.json` (PhysicsNeMo 1.3.0; 3
  input features, 4 surface outputs, 2 global features, a 128 × 64 × 64
  grid, geometry radii 0.01, 0.05 and 1.0 on the surface and 0.1, 0.5, 1.0
  and 2.5 in the volume, local radii 0.05 and 0.25, 7 surface neighbours,
  512-wide basis, aggregation and position networks) and a PyTorch archive
  with 313 tensor storages totalling 40,241,800 bytes. Tensor names, shapes
  and dtypes live in the pickle and were not read, so there is no inventory
  check or real-weight slice. A safetensors export would allow both.
- **The full architecture.** DoMINO's encoder is trained, has more radii
  and a convolutional processor over the whole grid; it has a volume model,
  positional encodings, Fourier features and parameter encodings (inlet
  velocity, air density). This chapter has a fixed encoder, two radii a
  stage, no processor and a surface model only.
- **DrivAerML.** The real dataset (about 31 TB, 500 variants, CC BY-SA 4.0)
  and its published splits are not used; the shapes here are synthetic
  spheroids in an exact flow. Wall shear stress, volume velocity and
  integrated drag on cars are not measured.

The model does not replace a solver or Verse's authoritative simulation; any
such use needs a separate consumer contract.

## Sources

- [Chapter plan](PLAN.md), Part XVI.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), DoMINO and DrivAerML
  entries.
- R. Ranade et al., *DoMINO: A Decomposable Multi-scale Iterative Neural
  Operator for Modeling Large Scale Engineering Simulations*,
  arXiv:2501.13350.
- H. Lamb, *Hydrodynamics*, §114 (the ellipsoid in a uniform stream).
