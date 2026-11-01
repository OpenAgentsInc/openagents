# 159. Learn a scene from its views

> **Status: partly implemented.** Rays, volume rendering and its gradient, a
> ground-truth scene with exact hits, and a radiance field on a voxel grid
> fitted to rendered views run on the CPU. The coordinate networks (an MLP
> fitting an image, then a signed-distance field) and training views from
> the Aethyr renderer are not built yet.

A radiance field describes a scene as two functions of position: how much
light a point absorbs (its density, σ) and what colour it gives off. A camera
image is what you get by sending a ray through each pixel and adding up the
colour the ray meets, weighted by how much light survives to reach each
point. That sum is smooth in σ and the colours, so you can differentiate the
image with respect to the field and fit the field to pictures.

This chapter builds that loop, in [`radiance.coax`](../lib/radiance.coax):

- **rays** from a pinhole camera, one through each pixel centre;
- **volume rendering** by quadrature: samples along a ray, the transmittance
  T, the composited colour, depth and opacity, and the gradient of all three
  written by hand;
- a **ground-truth scene**: [chapter 158](ch158-a-field-around-a-shape.md)'s
  sphere and box turned into a density, plus exact ray hits that do not use
  the signed distances;
- a **radiance field on a voxel grid**: four numbers a vertex, trilinear
  interpolation, softplus for σ and a sigmoid for colour;
- **fitting** with [chapter 124](ch124-adamw-and-explicit-precision.md)'s
  AdamW on batches of rays drawn from training views, and **measurement** on
  cameras the fit never saw: image quality as PSNR, depth against the exact
  hits, and occupancy against the shapes.

Everything runs in binary64 on one CPU core, with plain sums in a fixed
order. That is the profile `psionic-radiance-v1`. An image is at most 64 × 64
pixels; a ray has at most 256 samples; a scene has at most 8 spheres and
boxes; a grid has at most 32 vertices a side; a fit uses at most 32 views and
batches of at most 4,096 rays.

The field here is a grid of stored values, in the style of Plenoxels, not a
coordinate network. The rendering, the gradient and the fit are the parts a
coordinate network would share; what is missing is listed at the end.

## Run it

The programs are [`radiance.coax`](../examples/radiance.coax) and
[`radiance-refuse.coax`](../examples/radiance-refuse.coax); the checks'
oracles are in [`radiance-checks.coax`](../tests/radiance-checks.coax).

```shell
coax/bin/coax run psionic/examples/radiance.coax
coax/bin/coax run psionic/examples/radiance-refuse.coax
coax/bin/coax test psionic/lib/radiance.coax
coax/bin/coax mutate psionic/lib/radiance.coax --root-only
```

```text
pixel (0, 0): direction (-0.4082, 0.4082, -0.8165)
pixel (1, 0): direction (0.4082, 0.4082, -0.8165)
pixel (0, 1): direction (-0.4082, -0.4082, -0.8165)
pixel (1, 1): direction (0.4082, -0.4082, -0.8165)
two samples: rgb (0.3934693402873666, 0.3834004995642036, 0.22313016014842985), depth 0.38591770974499434, opacity 0.7768698398515701
d red / d sigma: (0.3032653298563167, 0.0), d red / d red_k: (0.3934693402873666, 0.3834004995642036)
slab of sigma 2, thickness 1: opacity 0.8646647167633873 against 1 - exp(-2) = 0.8646647167633873
one ray: loss 0.2738; 108 gradients against central differences, all within 1e-8: true
hit from (0, 0, 3) straight down -z: 2.7171572875253807
hit from (-0.35, 0, 3) straight down -z: 2.5499999999999994
hit from (0.45, 3, 0) straight down -y: 2.8
hit from (0, 3, 0) straight down -y: 2.7171572875253807
fit: ok, 6912 parameters, 300 steps of 64 rays
  step 0: loss 0.0334
  step 100: loss 0.0047
  step 200: loss 0.001
  step 299: loss 0.0003
training views: PSNR 31.99 dB
held-out views: PSNR 26.11 dB (untrained 15.25 dB, all background 14.03 dB)
held-out depth error 0.1131 over 33 pixels (ground truth's own 0.03 over 33)
occupancy: 330 of 4096 points inside the shapes; the field agrees on 3766 at sigma 20, 3798 at sigma 1
```

The example compiles, runs its checks and fits in about five seconds in a
debug build. Each part is worked through below.

## The types

<!-- source: psionic/lib/radiance.coax types -->
```jai
RfStatus :: enum u8 {ok;camera;sampling;number;shape;limit;scene;}
// A pinhole camera at eye looking at target, with up giving the roll. The
// focal length is in pixels: a ray through a pixel centre at column i, row j
// leaves along x right + y up + forward, x = (i + 0.5 - width/2) / focal,
// y = (height/2 - j - 0.5) / focal.
RfCamera :: struct {eye:[3]f64;target:[3]f64;up:[3]f64;width:int;height:int;focal:f64;}
RfRay :: struct {status:RfStatus;origin:[3]f64;dir:[3]f64;}
// count samples at the midpoints of equal steps from near to far.
RfSampling :: struct {near:f64;far:f64;count:int;}
// depth is the sum of w_k t_k; opacity is 1 - T, the share the samples absorbed.
RfPixel :: struct {status:RfStatus;rgb:[3]f64;depth:f64;opacity:f64;}
// Spheres and boxes with one colour each (three numbers a shape). The density
// at a point is density / (1 + exp(d / width)), d the nearest shape's signed
// distance; the colour is that shape's.
RfScene :: struct {shapes:[]FdShape;colors:[]f64;density:f64;width:f64;}
// side^3 vertices evenly spaced from low to high; four numbers a vertex:
// raw density (softplus gives sigma) and raw red, green, blue (sigmoid).
RfGrid :: struct {side:int;low:[3]f64;high:[3]f64;}
RfFit :: struct {steps:int;batch:int;lr:f64;seed:u64;}
RfResult :: struct {status:RfStatus;written:int;}
```

<!-- source: psionic/lib/radiance.coax bounds -->
```jai
RF_PIXELS_MAX :: 64;
RF_SAMPLES_MAX :: 256;
RF_SHAPES_MAX :: 8;
RF_SIDE_MAX :: 32;
RF_VIEWS_MAX :: 32;
RF_BATCH_MAX :: 4096;
RF_STEPS_MAX :: 100000;
RF_PARALLEL :: 0.000000000001;   // sin^2 of the angle between forward and up below this: no roll
```

## A ray through each pixel

A camera sits at `eye` and looks at `target`. Forward f is the unit vector
from eye to target. Right r is f × up, normalised, and the camera's own up u
is r × f, so (r, u, f) are three perpendicular unit vectors. A pixel at
column i and row j has its centre at

x = (i + 0.5 − width/2) / focal, y = (height/2 − j − 0.5) / focal

in units of the focal length, and its ray leaves the eye along
x r + y u + f, normalised. Rows count down from the top, so y falls as j
grows.

**By hand.** A 2 × 2 camera at (0, 0, 3) looking at the origin with up
(0, 1, 0) and focal length 1. Forward is (0, 0, −1); right is
(0, 0, −1) × (0, 1, 0) = (1, 0, 0); up is (1, 0, 0) × (0, 0, −1) = (0, 1, 0).
Pixel (0, 0) has x = (0.5 − 1)/1 = −0.5 and y = (1 − 0.5)/1 = 0.5, so its
direction is (−0.5, 0.5, −1)/√1.5 = (−0.4082, 0.4082, −0.8165): up and to
the left. The other three pixels flip one or both signs.

<!-- source: psionic/lib/radiance.coax camera -->
```jai
rf_finite :: (x:f64)->bool {return fd_finite(x);}
rf_camera_check :: (c:RfCamera)->RfStatus {
    if !fd_point(c.eye) || !fd_point(c.target) || !fd_point(c.up) || !rf_finite(c.focal) {return RfStatus.number;}
    if c.width>RF_PIXELS_MAX || c.height>RF_PIXELS_MAX {return RfStatus.limit;}
    if c.width<1 || c.height<1 || !(c.focal>0) {return RfStatus.camera;}
    // sin^2 of the angle between forward and up, from |f x up|^2. A target
    // at the eye has no forward direction: 0 / 0 makes x NaN, which no
    // comparison passes.
    f:=fd_sub(c.target,c.eye);x:=fd_cross(fd_div(f,fd_norm(f)),c.up);
    if !(fd_dot(x,x)>RF_PARALLEL*fd_dot(c.up,c.up)) {return RfStatus.camera;}
    return RfStatus.ok;
}
// Right, up and forward, three numbers each.
rf_basis :: (c:RfCamera)->[9]f64 {
    f:=fd_sub(c.target,c.eye);f=fd_div(f,fd_norm(f));
    r:=fd_cross(f,c.up);r=fd_div(r,fd_norm(r));u:=fd_cross(r,f);
    return [9]f64.[r[0],r[1],r[2],u[0],u[1],u[2],f[0],f[1],f[2]];
}
rf_ray :: (c:RfCamera,i:int,j:int)->RfRay {
    out:RfRay;out.status=rf_camera_check(c);
    if out.status==RfStatus.ok && (i<0 || j<0 || i>=c.width || j>=c.height) {out.status=RfStatus.camera;}
    if out.status!=RfStatus.ok {return out;}
    b:=rf_basis(c);
    x:=(f64(i)+0.5-f64(c.width)/2)/c.focal;y:=(f64(c.height)/2-f64(j)-0.5)/c.focal;
    d:[3]f64;for k:0..<3 {d[k]=x*b[k]+y*b[3+k]+b[6+k];}
    out.origin=c.eye;out.dir=fd_div(d,fd_norm(d));return out;
}
```

<!-- source: psionic/examples/radiance.coax rays -->
```jai
    c:=RfCamera.{eye=[3]f64.[0,0,3],up=[3]f64.[0,1,0],width=2,height=2,focal=1};
    for j:0..<2 {for i:0..<2 {
        r:=rf_ray(c,i,j);
        print("pixel (%, %): direction (%, %, %)\n",i,j,round4(r.dir[0]),round4(r.dir[1]),round4(r.dir[2]));
    }}
    
```

A camera is refused if any number is not finite, if it is wider or taller
than 64 pixels (`limit`), if it has no pixels or a focal length that is not
positive, or if up is parallel to the view (`camera`). The last test is
|f × up|² > 10⁻¹² |up|², the squared sine of the angle between them. A target
at the eye leaves no forward direction: 0/0 makes the cross product NaN, and
no comparison with NaN passes, so the same test refuses it.

## Adding up a ray

The samples sit at the midpoints of `count` equal steps from `near` to
`far`, so step k is at t_k = near + (k + 0.5) δ with δ = (far − near)/count.
Sample k has density σ_k and colour c_k. Within its step it lets through
exp(−σ_k δ) of the light that reaches it and absorbs

α_k = 1 − exp(−σ_k δ).

The light that reaches sample k is the transmittance
T_k = exp(−σ_0 δ) · … · exp(−σ_(k−1) δ), with T_0 = 1. Sample k's weight is
w_k = T_k α_k, and the ray's colour, depth and opacity are

C = Σ w_k c_k + T_N b, D = Σ w_k t_k, opacity = 1 − T_N,

where b is the background colour and T_N is what passes every sample.

**By hand.** Two samples from 0 to 1, so δ = 0.5 and t = 0.25, 0.75, with
σ = (1, 2), colours red (1, 0, 0) and green (0, 1, 0), and a blue background
(0, 0, 1).

- Sample 0: T_0 = 1, α_0 = 1 − e^(−0.5) = 0.39347, so w_0 = 0.39347.
- Sample 1: T_1 = e^(−0.5) = 0.60653, α_1 = 1 − e^(−1) = 0.63212, so
  w_1 = 0.60653 × 0.63212 = 0.38340.
- Past both: T_2 = e^(−1.5) = 0.22313.

So C = (0.39347, 0.38340, 0.22313), D = 0.25 × 0.39347 + 0.75 × 0.38340 =
0.38592, and the opacity is 1 − 0.22313 = 0.77687.

**The gradient.** For a scalar L = g · C + h D, the colour gradient is the
weight, dL/dc_k = w_k g. Raising σ_k does two things: sample k absorbs more,
and everything after it gets less light. Collecting both,

dL/dσ_k = δ (T_(k+1) (g · c_k + h t_k) − R_(k+1)),

where R_(k+1) is what everything after sample k adds to L, background
included. One pass forward keeps every T_k; one pass backward keeps R.

By hand, for the red channel alone (g = (1, 0, 0), h = 0): the red that
follows sample 0 is zero (green sample, blue background), so
dR/dσ_0 = 0.5 × (0.60653 × 1 − 0) = 0.30327; sample 1 is green with nothing
red after it, so dR/dσ_1 = 0. The colour gradients are the weights, 0.39347
and 0.38340.

<!-- source: psionic/lib/radiance.coax composite -->
```jai
rf_sampling_check :: (s:RfSampling)->RfStatus {
    if !rf_finite(s.near) || !rf_finite(s.far) {return RfStatus.number;}
    if s.count>RF_SAMPLES_MAX {return RfStatus.limit;}
    if s.count<1 || s.near<0 || !(s.far>s.near) {return RfStatus.sampling;}
    return RfStatus.ok;
}
rf_delta :: (s:RfSampling)->f64 {return (s.far-s.near)/f64(s.count);}
rf_t :: (s:RfSampling,k:int)->f64 {return s.near+(f64(k)+0.5)*rf_delta(s);}
rf_inputs :: (s:RfSampling,sigma:[]f64,color:[]f64,bg:[3]f64)->RfStatus {
    status:=rf_sampling_check(s);if status!=RfStatus.ok {return status;}
    if sigma.count!=s.count || color.count!=3*s.count {return RfStatus.shape;}
    for x:sigma {if !(x>=0) || !rf_finite(x) {return RfStatus.number;}}
    for x:color {if !rf_finite(x) {return RfStatus.number;}}
    if !fd_point(bg) {return RfStatus.number;}
    return RfStatus.ok;
}
// Sample k absorbs alpha_k = 1 - exp(-sigma_k delta) of the light that
// reaches it, T_k; its weight is w_k = T_k alpha_k, and the light past it is
// T_(k+1) = T_k exp(-sigma_k delta). What passes every sample shows the
// background.
rf_composite :: (s:RfSampling,sigma:[]f64,color:[]f64,bg:[3]f64)->RfPixel {
    out:RfPixel;out.status=rf_inputs(s,sigma,color,bg);if out.status!=RfStatus.ok {return out;}
    d:=rf_delta(s);T:=1.0;
    for k:0..<s.count {
        e:=exp(-sigma[k]*d);w:=T*(1-e);
        for c:0..<3 {out.rgb[c]+=w*color[3*k+c];}
        out.depth+=w*rf_t(s,k);T*=e;
    }
    for c:0..<3 {out.rgb[c]+=T*bg[c];}
    out.opacity=1-T;return out;
}
// For L = g . rgb + h depth: dL/dc_k = w_k g, and
// dL/dsigma_k = delta (T_(k+1) (g . c_k + h t_k) - rest_(k+1)), where
// rest_(k+1) is what everything after sample k adds to L. One pass forward
// keeps each T_k; one pass backward keeps the rest.
rf_composite_backward :: (s:RfSampling,sigma:[]f64,color:[]f64,bg:[3]f64,g:[3]f64,h:f64,dsigma:[]f64,dcolor:[]f64)->RfStatus {
    status:=rf_inputs(s,sigma,color,bg);if status!=RfStatus.ok {return status;}
    if dsigma.count!=s.count || dcolor.count!=3*s.count {return RfStatus.shape;}
    if !fd_point(g) || !rf_finite(h) {return RfStatus.number;}
    d:=rf_delta(s);T:[RF_SAMPLES_MAX+1]f64;T[0]=1;
    for k:0..<s.count {T[k+1]=T[k]*exp(-sigma[k]*d);}
    rest:=T[s.count]*fd_dot(g,bg);
    k:=s.count-1;
    while k>=0 {
        w:=T[k]-T[k+1];
        own:=h*rf_t(s,k);for c:0..<3 {own+=g[c]*color[3*k+c];dcolor[3*k+c]=w*g[c];}
        dsigma[k]=d*(T[k+1]*own-rest);
        rest+=w*own;k-=1;
    }
    return RfStatus.ok;
}
```

<!-- source: psionic/examples/radiance.coax composite -->
```jai
    s:=RfSampling.{near=0,far=1,count=2};
    sigma:=[2]f64.[1,2];color:=[6]f64.[1,0,0, 0,1,0];bg:=[3]f64.[0,0,1];
    p:=rf_composite(s,sigma[..],color[..],bg);
    print("two samples: rgb (%, %, %), depth %, opacity %\n",p.rgb[0],p.rgb[1],p.rgb[2],p.depth,p.opacity);
    ds:[2]f64;dc:[6]f64;
    st:=rf_composite_backward(s,sigma[..],color[..],bg,[3]f64.[1,0,0],0,ds[..],dc[..]);
    print("d red / d sigma: (%, %), d red / d red_k: (%, %)\n",ds[0],ds[1],dc[0],dc[3]);
    
```

**A closed form.** A slab of constant density σ and thickness d absorbs
exactly 1 − exp(−σ d) of the light, and the quadrature agrees for any step
count as long as the slab's ends fall on step boundaries, because the
product of exp(−σ δ) over the slab's steps is exp(−σ d). From 1 to 3 in 8
steps (δ = 0.25), σ = 2 on steps 2 to 5 is a slab 1 thick:

<!-- source: psionic/examples/radiance.coax slab -->
```jai
    slab:=RfSampling.{near=1,far=3,count=8};thick:[8]f64;grey:[24]f64;
    for k:0..<8 {if k>=2 && k<6 {thick[k]=2;}for q:0..<3 {grey[3*k+q]=0.5;}}
    q:=rf_composite(slab,thick[..],grey[..],[3]f64.[0,0,0]);
    print("slab of sigma 2, thickness 1: opacity % against 1 - exp(-2) = %\n",q.opacity,1-exp(-2.0));
    
```

## The ground truth

The scene is [chapter 158](ch158-a-field-around-a-shape.md)'s analytic
shapes turned into a density. With d the nearest shape's signed distance,

σ(p) = density / (1 + exp(d / width)),

which is `density` deep inside, half of it on the surface, and falls to zero
a few widths outside. The colour is the nearest shape's. The chapter's scene,
`rf_demo`, is a red sphere of radius 0.45 at (−0.35, 0, 0) and a blue box
from (0.15, −0.4, −0.3) to (0.75, 0.2, 0.3), with density 40 and width 0.02,
all inside the cube from −1 to 1.

Depth needs a reference that does not come from the density, so `rf_hit`
finds where a ray first meets a surface in closed form: a quadratic for a
sphere, and for a box the overlap of the three slabs between its faces.

**By hand.** From (0, 0, 3) straight down −z, the ray passes the sphere's
centre at a sideways distance of 0.35, so it crosses the surface
√(0.45² − 0.35²) = √0.08 = 0.28284 before the centre's depth: t = 3 − 0.28284
= 2.71716. From (−0.35, 0, 3) it goes through the centre: t = 3 − 0.45 =
2.55. From (0.45, 3, 0) down −y it lands on the box's top face, y = 0.2, at
t = 2.8.

<!-- source: psionic/lib/radiance.coax scene -->
```jai
rf_scene_check :: (sc:RfScene)->RfStatus {
    if sc.shapes.count>RF_SHAPES_MAX {return RfStatus.limit;}
    if sc.shapes.count<1 || sc.colors.count!=3*sc.shapes.count {return RfStatus.shape;}
    for x:sc.colors {if !(x>=0 && x<=1) {return RfStatus.number;}}
    if !rf_finite(sc.density) || !rf_finite(sc.width) {return RfStatus.number;}
    if !(sc.density>0) || !(sc.width>0) {return RfStatus.scene;}
    for s:sc.shapes {if s.kind==FdKind.capsule || fd_shape_check(s)!=FdStatus.ok {return RfStatus.scene;}}
    return RfStatus.ok;
}
// (sigma, r, g, b) at p.
rf_scene_at :: (sc:RfScene,p:[3]f64)->[4]f64 {
    best:=0;d:=fd_shape_distance(sc.shapes[0],p).d;
    for i:1..<sc.shapes.count {e:=fd_shape_distance(sc.shapes[i],p).d;if e<d {d=e;best=i;}}
    return [4]f64.[sc.density/(1+exp(d/sc.width)),sc.colors[3*best],sc.colors[3*best+1],sc.colors[3*best+2]];
}
// The first t > 0 where o + t d meets a shape's surface, or RF_MISS: a
// quadratic for a sphere (d a unit vector), three slabs for a box.
// Independent of the signed distances: the checks compare the two.
RF_MISS :: 1e300;
rf_hit_sphere :: (s:FdShape,o:[3]f64,d:[3]f64)->f64 {
    v:=fd_sub(o,s.a);b:=fd_dot(d,v);c:=fd_dot(v,v)-s.r*s.r;q:=b*b-c;
    if q<0 {return RF_MISS;}
    q=sqrt(q);if -b-q>0 {return -b-q;}if -b+q>0 {return -b+q;}
    return RF_MISS;
}
// The ray is inside slab k for t between its two crossings; inside the box
// for t in [lo, hi], the overlap of all three.
rf_hit_box :: (s:FdShape,o:[3]f64,d:[3]f64)->f64 {
    lo:=-RF_MISS;hi:=RF_MISS;
    for k:0..<3 {
        if d[k]==0 {if o[k]<s.a[k] || o[k]>s.b[k] {return RF_MISS;}}
        else {
            t1:=(s.a[k]-o[k])/d[k];t2:=(s.b[k]-o[k])/d[k];
            lo=fd_max(lo,fd_min(t1,t2));hi=fd_min(hi,fd_max(t1,t2));
        }
    }
    if lo>hi || !(hi>0) {return RF_MISS;}
    return ifx lo>0 then lo else hi;
}
// The nearest hit over every shape, or -1 for none.
rf_hit :: (sc:RfScene,o:[3]f64,d:[3]f64)->f64 {
    best:=RF_MISS;
    for s:sc.shapes {best=fd_min(best,ifx s.kind==FdKind.sphere then rf_hit_sphere(s,o,d) else rf_hit_box(s,o,d));}
    return ifx best<RF_MISS then best else -1.0;
}
```

<!-- source: psionic/examples/radiance.coax hits -->
```jai
    sh:[2]FdShape;co:[6]f64;sc:=rf_demo(sh[..],co[..]);
    print("hit from (0, 0, 3) straight down -z: %\n",rf_hit(sc,[3]f64.[0,0,3],[3]f64.[0,0,-1]));
    print("hit from (-0.35, 0, 3) straight down -z: %\n",rf_hit(sc,[3]f64.[-0.35,0,3],[3]f64.[0,0,-1]));
    print("hit from (0.45, 3, 0) straight down -y: %\n",rf_hit(sc,[3]f64.[0.45,3,0],[3]f64.[0,-1,0]));
    print("hit from (0, 3, 0) straight down -y: %\n",rf_hit(sc,[3]f64.[0,3,0],[3]f64.[0,-1,0]));
    
```

The checks compare `rf_hit` with chapter 158's signed distance: on 32 random
rays aimed into the shapes, the hit point is within 10⁻¹² of a surface, and
a point 0.1% short of it is outside every shape. Tangent rays, rays along a
face plane, rays through an edge and rays starting on a surface are checked
by hand.

## A field on a grid

The field is a grid of side × side × side vertices spread evenly over a box,
with four numbers at each vertex: a raw density and raw red, green and blue.
At a point inside the box, each raw value is the trilinear blend of the eight
vertices around it; then σ = softplus(raw) = log(1 + e^raw), which is always
positive, and each colour is the sigmoid 1/(1 + e^(−raw)), which stays in
(0, 1). Outside the box σ and colour are zero.

**By hand.** On a 2-vertex grid over the unit cube, give vertex (x, y, z)
the raw density 1 + x + 2y + 4z. Trilinear interpolation reproduces any
linear function exactly, so at (0.25, 0.5, 0.75) the raw density is
1 + 0.25 + 1 + 3 = 5.25. At the corner (0, 0, 0) it is 1, and
σ = log(1 + e) = 1.3133.

<!-- source: psionic/lib/radiance.coax grid -->
```jai
rf_softplus :: (x:f64)->f64 {return ifx x>30 then x else log(1+exp(x));}
rf_sigmoid :: (x:f64)->f64 {return 1/(1+exp(-x));}
rf_grid_check :: (g:RfGrid,params:[]f64)->RfStatus {
    if !fd_point(g.low) || !fd_point(g.high) {return RfStatus.number;}
    if g.side>RF_SIDE_MAX {return RfStatus.limit;}
    if g.side<2 || params.count!=4*g.side*g.side*g.side {return RfStatus.shape;}
    for k:0..<3 {if !(g.low[k]<g.high[k]) {return RfStatus.shape;}}
    for x:params {if !rf_finite(x) {return RfStatus.number;}}
    return RfStatus.ok;
}
rf_init :: (g:RfGrid,params:[]f64,density:f64) {for i:0..<params.count {params[i]=ifx i%4==0 then density else 0.0;}}
// Where p falls among the vertices: the lower corner of its cell and the
// fractions across it. Outside the box, nothing.
RfCell :: struct {inside:bool;base:[3]int;frac:[3]f64;raw:[4]f64;}
rf_cell :: (g:RfGrid,p:[3]f64)->RfCell {
    out:RfCell;n:=f64(g.side-1);
    for k:0..<3 {
        u:=(p[k]-g.low[k])/(g.high[k]-g.low[k])*n;
        if !(u>=0 && u<=n) {return out;}
        b:=fd_imin(cast(int)floor(u),g.side-2);
        out.base[k]=b;out.frac[k]=u-f64(b);
    }
    out.inside=true;return out;
}
// The weight of corner (x, y, z), each 0 or 1, and its vertex.
rf_corner :: (g:RfGrid,c:RfCell,x:int,y:int,z:int)->int {return ((c.base[2]+z)*g.side+c.base[1]+y)*g.side+c.base[0]+x;}
rf_weight :: (c:RfCell,x:int,y:int,z:int)->f64 {
    w:=1.0;q:=[3]int.[x,y,z];
    for k:0..<3 {w*=ifx q[k]==1 then c.frac[k] else 1-c.frac[k];}
    return w;
}
rf_field_raw :: (g:RfGrid,params:[]f64,p:[3]f64)->RfCell {
    c:=rf_cell(g,p);if !c.inside {return c;}
    for z:0..<2 {for y:0..<2 {for x:0..<2 {
        w:=rf_weight(c,x,y,z);v:=rf_corner(g,c,x,y,z);
        for ch:0..<4 {c.raw[ch]+=w*params[4*v+ch];}
    }}}
    return c;
}
// (sigma, r, g, b): zero outside the grid.
rf_field_at :: (g:RfGrid,params:[]f64,p:[3]f64)->[4]f64 {
    c:=rf_field_raw(g,params,p);out:[4]f64;if !c.inside {return out;}
    out[0]=rf_softplus(c.raw[0]);for ch:1..<4 {out[ch]=rf_sigmoid(c.raw[ch]);}
    return out;
}
// Adds the gradient of (sigma, r, g, b) at p, weighted by dout, to grads.
rf_field_back :: (g:RfGrid,params:[]f64,p:[3]f64,dout:[4]f64,grads:[]f64) {
    c:=rf_field_raw(g,params,p);if !c.inside {return;}
    draw:[4]f64;draw[0]=dout[0]*rf_sigmoid(c.raw[0]);
    for ch:1..<4 {s:=rf_sigmoid(c.raw[ch]);draw[ch]=dout[ch]*s*(1-s);}
    for z:0..<2 {for y:0..<2 {for x:0..<2 {
        w:=rf_weight(c,x,y,z);v:=rf_corner(g,c,x,y,z);
        for ch:0..<4 {grads[4*v+ch]+=w*draw[ch];}
    }}}
}
```

The gradient of σ and colour with respect to the stored numbers is the chain
of two simple pieces: softplus′ = sigmoid and sigmoid′ = s(1 − s), then the
same eight trilinear weights.

## Rendering and the gradient of one ray

`rf_render` sends a ray through every pixel, samples the scene or the field
at each step, composites, and writes colour, opacity and depth. Depth here is
D / opacity: where the absorbed light stopped, on average, or 0 for a pixel
that absorbed nothing.

<!-- source: psionic/lib/radiance.coax render -->
```jai
rf_point :: (r:RfRay,s:RfSampling,k:int)->[3]f64 {return fd_add(r.origin,fd_scale(r.dir,rf_t(s,k)));}
// One ray's samples from the scene (field false) or the grid (field true).
rf_samples :: (field:bool,sc:RfScene,g:RfGrid,params:[]f64,r:RfRay,s:RfSampling,sigma:[]f64,color:[]f64) {
    for k:0..<s.count {
        p:=rf_point(r,s,k);
        x:=ifx field then rf_field_at(g,params,p) else rf_scene_at(sc,p);
        sigma[k]=x[0];for c:0..<3 {color[3*k+c]=x[1+c];}
    }
}
// Colour, depth (where the absorbed light stopped, on average; 0 if none
// was) and opacity for every pixel, row by row.
rf_render :: (field:bool,sc:RfScene,g:RfGrid,params:[]f64,cam:RfCamera,s:RfSampling,bg:[3]f64,rgb:[]f64,depth:[]f64,opacity:[]f64)->RfResult {
    out:RfResult;out.status=rf_camera_check(cam);
    if out.status==RfStatus.ok {out.status=rf_sampling_check(s);}
    n:=cam.width*cam.height;
    if out.status==RfStatus.ok && (rgb.count!=3*n || depth.count!=n || opacity.count!=n) {out.status=RfStatus.shape;}
    if out.status!=RfStatus.ok {return out;}
    sigma:[RF_SAMPLES_MAX]f64;color:[3*RF_SAMPLES_MAX]f64;
    for j:0..<cam.height {for i:0..<cam.width {
        r:=rf_ray(cam,i,j);rf_samples(field,sc,g,params,r,s,sigma[..],color[..]);
        p:=rf_composite(s,sigma[0..<s.count],color[0..<3*s.count],bg);
        if p.status!=RfStatus.ok {out.status=p.status;return out;}
        k:=j*cam.width+i;for c:0..<3 {rgb[3*k+c]=p.rgb[c];}
        depth[k]=ifx p.opacity>0 then p.depth/p.opacity else 0.0;opacity[k]=p.opacity;
    }}
    out.written=n;return out;
}
rf_render_scene :: (sc:RfScene,cam:RfCamera,s:RfSampling,bg:[3]f64,rgb:[]f64,depth:[]f64,opacity:[]f64)->RfResult {
    status:=rf_scene_check(sc);
    if status!=RfStatus.ok {return RfResult.{status=status};}
    g:RfGrid;return rf_render(false,sc,g,rgb,cam,s,bg,rgb,depth,opacity);
}
rf_render_field :: (g:RfGrid,params:[]f64,cam:RfCamera,s:RfSampling,bg:[3]f64,rgb:[]f64,depth:[]f64,opacity:[]f64)->RfResult {
    status:=rf_grid_check(g,params);
    if status!=RfStatus.ok {return RfResult.{status=status};}
    sc:RfScene;return rf_render(true,sc,g,params,cam,s,bg,rgb,depth,opacity);
}
```

`rf_ray_grad` is one ray's squared colour error against a target, and, when
`scale` is not zero, adds `scale` times its gradient to `grads`: the
compositing gradient first, then each sample's share back through the
softplus, the sigmoid and the trilinear weights.

<!-- source: psionic/lib/radiance.coax fit -->
```jai
// One ray's squared colour error against target; when scale is not zero,
// adds scale times its gradient to grads. Inputs are checked by the caller.
rf_ray_grad :: (g:RfGrid,params:[]f64,r:RfRay,s:RfSampling,bg:[3]f64,target:[3]f64,scale:f64,grads:[]f64)->f64 {
    sigma:[RF_SAMPLES_MAX]f64;color:[3*RF_SAMPLES_MAX]f64;sc:RfScene;n:=s.count;
    rf_samples(true,sc,g,params,r,s,sigma[..],color[..]);
    p:=rf_composite(s,sigma[0..<n],color[0..<3*n],bg);
    loss:=0.0;e:[3]f64;for c:0..<3 {e[c]=p.rgb[c]-target[c];loss+=e[c]*e[c];}
    if scale==0 {return loss;}
    ds:[RF_SAMPLES_MAX]f64;dc:[3*RF_SAMPLES_MAX]f64;
    status:=rf_composite_backward(s,sigma[0..<n],color[0..<3*n],bg,fd_scale(e,2*scale),0,ds[0..<n],dc[0..<3*n]);
    if status!=RfStatus.ok {return 0.0/0.0;}
    for k:0..<n {rf_field_back(g,params,rf_point(r,s,k),[4]f64.[ds[k],dc[3*k],dc[3*k+1],dc[3*k+2]],grads);}
    return loss;
}
rf_fit_check :: (g:RfGrid,params:[]f64,m:[]f64,v:[]f64,grads:[]f64,cams:[]RfCamera,images:[]f64,s:RfSampling,bg:[3]f64,cfg:RfFit,losses:[]f64)->RfStatus {
    status:=rf_grid_check(g,params);if status!=RfStatus.ok {return status;}
    if m.count!=params.count || v.count!=params.count || grads.count!=params.count {return RfStatus.shape;}
    if cams.count>RF_VIEWS_MAX || cfg.batch>RF_BATCH_MAX || cfg.steps>RF_STEPS_MAX {return RfStatus.limit;}
    if cams.count<1 || cfg.batch<1 || cfg.steps<1 || losses.count!=cfg.steps {return RfStatus.shape;}
    for c:cams {
        status=rf_camera_check(c);if status!=RfStatus.ok {return status;}
        if c.width!=cams[0].width || c.height!=cams[0].height {return RfStatus.shape;}
    }
    if images.count!=3*cams.count*cams[0].width*cams[0].height {return RfStatus.shape;}
    for x:images {if !(x>=0 && x<=1) {return RfStatus.number;}}
    status=rf_sampling_check(s);if status!=RfStatus.ok {return status;}
    if !fd_point(bg) {return RfStatus.number;}
    return RfStatus.ok;
}
// Each step draws batch rays from every view's pixels by a counter hash,
// averages the squared error per colour channel, and takes one AdamW step.
rf_fit :: (g:RfGrid,params:[]f64,m:[]f64,v:[]f64,grads:[]f64,cams:[]RfCamera,images:[]f64,s:RfSampling,bg:[3]f64,cfg:RfFit,losses:[]f64)->RfStatus {
    status:=rf_fit_check(g,params,m,v,grads,cams,images,s,bg,cfg,losses);if status!=RfStatus.ok {return status;}
    pixels:=cams[0].width*cams[0].height;rays:=cast(u64)(cams.count*pixels);scale:=1/f64(3*cfg.batch);
    group:=[1]AwGroup.[AwGroup.{first=0,count=params.count,lr=cfg.lr,beta1=0.9,beta2=0.99,eps=0.00000001,decay=0}];
    state:AwState;none:[1]f32;
    for step:0..<cfg.steps {
        for i:0..<grads.count {grads[i]=0;}
        total:=0.0;
        for b:0..<cfg.batch {
            idx:=cast(int)(cr_bits(cfg.seed,cast(u64)(step*cfg.batch+b))%rays);view:=idx/pixels;px:=idx%pixels;
            c:=cams[view];r:=rf_ray(c,px%c.width,px/c.width);t:=fd_at(images,idx);
            total+=rf_ray_grad(g,params,r,s,bg,t,scale,grads);
        }
        losses[step]=total*scale;
        a:=aw_step(group[..],AwPolicy.{},state,params,grads,m,v,none[0..<0],false);
        if a.status!=AwStatus.ok {return RfStatus.number;}
        state=a.state;
    }
    return RfStatus.ok;
}
```

The example checks every one of the 108 numbers of a 3 × 3 × 3 grid against
central differences with a step of 10⁻⁶:

<!-- source: psionic/examples/radiance.coax gradients -->
```jai
    // Every parameter of a 3 x 3 x 3 grid, one ray, central differences.
    g:=RfGrid.{side=3,low=[3]f64.[-1,-1,-1],high=[3]f64.[1,1,1]};params:[108]f64;grads:[108]f64;
    for i:0..<108 {params[i]=sin(f64(i)*1.7)*2;}
    ray:=rf_ray(RfCamera.{eye=[3]f64.[0.2,-0.1,3],up=[3]f64.[0,1,0],width=4,height=4,focal=4},1,2);
    rs:=RfSampling.{near=1.5,far=4.5,count=12};tg:=[3]f64.[0.3,0.6,0.1];
    loss:=rf_ray_grad(g,params[..],ray,rs,bg,tg,1,grads[..]);worst:=0.0;
    for i:0..<108 {
        a:=params;a[i]+=0.000001;z:=params;z[i]-=0.000001;
        fd:=(rf_ray_grad(g,a[..],ray,rs,bg,tg,0,grads[0..<0])-rf_ray_grad(g,z[..],ray,rs,bg,tg,0,grads[0..<0]))/0.000002;
        if abs(fd-grads[i])>worst {worst=abs(fd-grads[i]);}
    }
    print("one ray: loss %; 108 gradients against central differences, all within 1e-8: %\n",round4(loss),worst<=0.00000001);
    
```

## Fitting and measuring

Each training step draws `batch` rays from every view's pixels with the
counter hash `cr_bits` from [chapter 53](ch053-choosing-the-next-token.md),
averages the squared error over the rays and the three channels, and takes
one AdamW step (β₁ = 0.9, β₂ = 0.99, no weight decay). The grid starts at raw
density −2 everywhere (σ = 0.127, a faint grey fog) and colour 0 (grey).

Image quality is the peak signal-to-noise ratio,
PSNR = 10 log₁₀(1 / mean squared error) for values in [0, 1]: 20 dB is an
error of 0.1 per channel on average, 40 dB an error of 0.01. Two baselines
put the number in context: the untrained field, and an image of pure
background. Because most pixels are background here, the second already
scores about 14 dB.

Depth error is the mean |depth − exact hit| over pixels whose ray hits a
shape and that are at least half opaque. The ground-truth render's own depth
error is printed beside it: the soft density starts absorbing a little before
the exact surface, so even the truth is not at zero.

Occupancy asks a different question: at 4,096 points spread through the
grid's box, does the field say "solid" exactly where the shapes are? It is
read at σ = 20, the scene's own surface level, and at σ = 1.

<!-- source: psionic/lib/radiance.coax measure -->
```jai
rf_sq :: (a:[]f64,b:[]f64)->f64 {e:=0.0;for i:0..<a.count {d:=a[i]-b[i];e+=d*d;}return e;}
// Peak signal-to-noise ratio for values in [0, 1]: 10 log10 (count / squared error).
rf_db :: (sum:f64,count:int)->f64 {return 10*log(f64(count)/sum)/log(10.0);}
rf_psnr :: (a:[]f64,b:[]f64)->f64 {return rf_db(rf_sq(a,b),a.count);}
// Depth error summed over the pixels whose ray hits a shape exactly and
// that are at least half opaque, and how many there were. depth and
// opacity hold one number a pixel of cam, row by row, from index first.
RfDepth :: struct {sum:f64;count:int;}
rf_depth_error :: (sc:RfScene,cam:RfCamera,depth:[]f64,opacity:[]f64,first:int)->RfDepth {
    out:RfDepth;
    for i:0..<cam.width*cam.height {
        r:=rf_ray(cam,i%cam.width,i/cam.width);t:=rf_hit(sc,r.origin,r.dir);
        if opacity[first+i]>=0.5 && t!=-1.0 {out.sum+=abs(depth[first+i]-t);out.count+=1;}
    }
    return out;
}
rf_mean :: (d:RfDepth)->f64 {return ifx d.count>0 then d.sum/f64(d.count) else 0.0;}
// inside: how many points the scene's shapes contain (signed distance below
// 0). agree: how many the field calls occupied (sigma above level) exactly
// when the shapes contain them.
RfOccupancy :: struct {agree:int;inside:int;}
rf_occupancy :: (g:RfGrid,params:[]f64,sc:RfScene,points:[]f64,level:f64)->RfOccupancy {
    out:RfOccupancy;
    for i:0..<points.count/3 {
        p:=fd_at(points,i);inside:=false;
        for s:sc.shapes {if fd_shape_distance(s,p).d<0 {inside=true;}}
        if inside {out.inside+=1;}
        if (rf_field_at(g,params,p)[0]>level)==inside {out.agree+=1;}
    }
    return out;
}
```

The experiment puts the cameras on a circle of radius 3 at height 1.2,
looking at the origin. Held-out cameras sit halfway between two training
cameras, at the same height.

<!-- source: psionic/lib/radiance.coax experiment -->
```jai
// count cameras on a circle of the given radius at height 1.2, every one
// looking at the origin; camera k sits at angle (k + offset) / count turns.
rf_views :: (count:int,radius:f64,size:int,offset:f64,out:[]RfCamera) {
    for k:0..<count {
        a:=(f64(k)+offset)*2*FD_PI/f64(count);
        out[k]=RfCamera.{eye=[3]f64.[radius*sin(a),1.2,radius*cos(a)],up=[3]f64.[0,1,0],width=size,height=size,focal=f64(size)};
    }
}
// The chapter's scene: a red sphere and a blue box inside the cube from -1 to 1.
rf_demo :: (shapes:[]FdShape,colors:[]f64)->RfScene {
    shapes[0]=FdShape.{kind=FdKind.sphere,a=[3]f64.[-0.35,0,0],r=0.45};
    shapes[1]=FdShape.{kind=FdKind.box,a=[3]f64.[0.15,-0.4,-0.3],b=[3]f64.[0.75,0.2,0.3]};
    c:=[6]f64.[0.9,0.25,0.15, 0.15,0.4,0.9];for k:0..<6 {colors[k]=c[k];}
    return RfScene.{shapes=shapes,colors=colors,density=40,width=0.02};
}
// views training cameras and held cameras halfway between training cameras;
// truth samples a ray for the ground-truth views, samples for the field.
RfRun :: struct {views:int;held:int;size:int;side:int;samples:int;truth:int;steps:int;batch:int;lr:f64;seed:u64;}
// PSNR in dB: of the fitted field on the training and held-out views, of
// the untrained field (grey) and of an all-background image (blank) on the
// held-out views. Depth errors are means over rf_depth_error's pixels.
// Occupancy is read at level (the scene's surface, density / 2) and at 1.
// rendered counts the pixels of the fitted field's renders.
RfReport :: struct {status:RfStatus;params:int;rendered:int;train:f64;held:f64;grey:f64;blank:f64;depth:f64;truth_depth:f64;covered:int;truth_covered:int;level:f64;agree:int;agree_low:int;inside:int;points:int;}
RF_HELD_MAX :: 8;
RF_OCCUPANCY_POINTS :: 4096;
RF_AREA :: RF_PIXELS_MAX*RF_PIXELS_MAX;
rf_run_check :: (cfg:RfRun,images:[]f64)->RfStatus {
    if cfg.views>RF_VIEWS_MAX || cfg.held>RF_HELD_MAX || cfg.size>RF_PIXELS_MAX {return RfStatus.limit;}
    if cfg.views<1 || cfg.held<1 || cfg.size<1 || images.count!=3*cfg.views*cfg.size*cfg.size {return RfStatus.shape;}
    return RfStatus.ok;
}
// Held camera k sits halfway between training cameras k views / held and the next.
rf_held :: (cfg:RfRun,out:[]RfCamera) {
    for k:0..<cfg.held {one:[1]RfCamera;rf_views(1,3,cfg.size,(f64(k*cfg.views/cfg.held)+0.5)/f64(cfg.views),one[..]);out[k]=one[0];}
}
// The scene from each camera, n pixels each, into its share of rgb, depth
// and opacity.
rf_truth :: (sc:RfScene,cams:[]RfCamera,n:int,s:RfSampling,bg:[3]f64,rgb:[]f64,depth:[]f64,opacity:[]f64)->RfStatus {
    for k:0..<cams.count {
        r:=rf_render_scene(sc,cams[k],s,bg,rgb[3*n*k..<3*n*(k+1)],depth[n*k..<n*(k+1)],opacity[n*k..<n*(k+1)]);
        if r.status!=RfStatus.ok {return r.status;}
    }
    return RfStatus.ok;
}
// The field from each camera: its squared error against want, its depth
// error, and how many pixels it rendered. The caller has checked the inputs.
RfScore :: struct {pixels:int;sq:f64;depth:RfDepth;}
rf_score :: (g:RfGrid,params:[]f64,sc:RfScene,cams:[]RfCamera,n:int,s:RfSampling,bg:[3]f64,want:[]f64)->RfScore {
    out:RfScore;rgb:[3*RF_AREA]f64;depth:[RF_AREA]f64;op:[RF_AREA]f64;
    for k:0..<cams.count {
        out.pixels+=rf_render_field(g,params,cams[k],s,bg,rgb[0..<3*n],depth[0..<n],op[0..<n]).written;
        out.sq+=rf_sq(rgb[0..<3*n],want[3*n*k..<3*n*(k+1)]);
        d:=rf_depth_error(sc,cams[k],depth[..],op[..],0);out.depth.sum+=d.sum;out.depth.count+=d.count;
    }
    return out;
}
rf_run :: (cfg:RfRun,params:[]f64,m:[]f64,v:[]f64,grads:[]f64,images:[]f64,losses:[]f64)->RfReport {
    out:RfReport;out.status=rf_run_check(cfg,images);if out.status!=RfStatus.ok {return out;}
    s:=RfSampling.{near=1.5,far=4.5,count=cfg.samples};out.status=rf_sampling_check(s);if out.status!=RfStatus.ok {return out;}
    g:=RfGrid.{side=cfg.side,low=[3]f64.[-1,-1,-1],high=[3]f64.[1,1,1]};rf_init(g,params,-2);
    out.status=rf_grid_check(g,params);if out.status!=RfStatus.ok {return out;}
    out.params=params.count;
    sh:[2]FdShape;co:[6]f64;sc:=rf_demo(sh[..],co[..]);
    cams:[RF_VIEWS_MAX]RfCamera;rf_views(cfg.views,3,cfg.size,0,cams[..]);held:[RF_HELD_MAX]RfCamera;rf_held(cfg,held[..]);
    train:=cams[0..<cfg.views];test:=held[0..<cfg.held];
    truth:=RfSampling.{near=1.5,far=4.5,count=cfg.truth};bg:[3]f64;
    // Ground truth for the training views, then for the held-out views.
    n:=cfg.size*cfg.size;depth:[RF_VIEWS_MAX*RF_AREA]f64;op:[RF_VIEWS_MAX*RF_AREA]f64;
    want:[RF_HELD_MAX*3*RF_AREA]f64;td:[RF_HELD_MAX*RF_AREA]f64;top:[RF_HELD_MAX*RF_AREA]f64;
    w:=want[0..<3*n*cfg.held];
    out.status=rf_truth(sc,train,n,truth,bg,images,depth[..],op[..]);
    if out.status==RfStatus.ok {out.status=rf_truth(sc,test,n,truth,bg,w,td[..],top[..]);}
    if out.status!=RfStatus.ok {return out;}
    tdepth:RfDepth;
    for k:0..<cfg.held {d:=rf_depth_error(sc,test[k],td[..],top[..],n*k);tdepth.sum+=d.sum;tdepth.count+=d.count;}
    blank:=0.0;for x:w {blank+=x*x;}
    // The untrained field on the held-out views, then the fit.
    grey:=rf_score(g,params,sc,test,n,s,bg,w);
    out.status=rf_fit(g,params,m,v,grads,train,images,s,bg,RfFit.{steps=cfg.steps,batch=cfg.batch,lr=cfg.lr,seed=cfg.seed},losses);
    if out.status!=RfStatus.ok {return out;}
    a:=rf_score(g,params,sc,train,n,s,bg,images);b:=rf_score(g,params,sc,test,n,s,bg,w);
    out.rendered=a.pixels+b.pixels;out.train=rf_db(a.sq,images.count);out.held=rf_db(b.sq,w.count);
    out.grey=rf_db(grey.sq,w.count);out.blank=rf_db(blank,w.count);
    out.depth=rf_mean(b.depth);out.covered=b.depth.count;
    out.truth_depth=rf_mean(tdepth);out.truth_covered=tdepth.count;
    // The scene's surface is where its density is half its peak; the field
    // is also read at sigma = 1, a level a fitted field reaches far more often.
    pts:[3*RF_OCCUPANCY_POINTS]f64;out.points=fd_sample_box(g.low,g.high,11,pts[..]).written;
    out.level=sc.density/2;o:=rf_occupancy(g,params,sc,pts[..],out.level);
    out.agree=o.agree;out.inside=o.inside;out.agree_low=rf_occupancy(g,params,sc,pts[..],1).agree;
    return out;
}
```

The example's fit is small so that it stays fast: 8 views of 12 × 12, a
12-vertex grid (6,912 numbers), 24 samples a ray, 300 steps of 64 rays.

<!-- source: psionic/examples/radiance.coax fit -->
```jai
    cfg:=RfRun.{views=8,held=2,size=12,side=12,samples=24,truth=96,steps=300,batch=64,lr=0.05,seed=159};
    w:[6912]f64;m:[6912]f64;v:[6912]f64;dw:[6912]f64;images:[3456]f64;losses:[300]f64;
    out:=rf_run(cfg,w[..],m[..],v[..],dw[..],images[..],losses[..]);
    print("fit: %, % parameters, % steps of % rays\n",out.status,out.params,cfg.steps,cfg.batch);
    for k:0..<300 {if k%100==0 || k==299 {print("  step %: loss %\n",k,round4(losses[k]));}}
    print("training views: PSNR % dB\n",round2(out.train));
    print("held-out views: PSNR % dB (untrained % dB, all background % dB)\n",round2(out.held),round2(out.grey),round2(out.blank));
    print("held-out depth error % over % pixels (ground truth's own % over %)\n",round4(out.depth),out.covered,round4(out.truth_depth),out.truth_covered);
    print("occupancy: % of % points inside the shapes; the field agrees on % at sigma 20, % at sigma 1\n",out.inside,out.points,out.agree,out.agree_low);
    
```

It reaches 32 dB on the training views and 26 dB on the two held-out views,
against 15 dB for the untrained field.

## A longer fit

The acceptance threshold was written into this chapter and into the
program before the program first ran: on the four held-out views, **the PSNR
must be at least 20 dB**. 20 dB means the colour is off by about 0.1 per
channel on average; the all-background image scores about 14 dB, so a field
that only learned the background would fail.

The run uses 16 training views of 24 × 24, a 24-vertex grid (55,296
numbers), 48 samples a ray against ground truth rendered with 192, and 3,000
AdamW steps of 256 rays. Its program is
[`radiance-long.coax`](../tests/radiance-long.coax), and its output is kept
in [`radiance-long.out`](../fixtures/radiance-long.out):

```shell
coax/bin/coax run psionic/tests/radiance-long.coax --release
```

```text
status ok
training: 16 views of 24 x 24, ground truth 192 samples a ray; field 48 samples a ray
grid: 24 vertices a side, 55296 parameters (1327104 bytes for parameters and AdamW moments)
fit: 3000 steps of 256 rays, rate 0.05
  step 0: loss 0.0304
  step 500: loss 0.0001226
  step 1000: loss 0.00001278
  step 1500: loss 6.507e-06
  step 2000: loss 8.722e-06
  step 2500: loss 4.663e-06
  step 2999: loss 5.048e-06
training views: PSNR 52.14 dB
held-out views (4, halfway between training cameras): PSNR 38.0 dB; untrained field 15.66 dB, all-background image 14.48 dB
held-out depth error: 0.0998 over 232 pixels; the ground-truth render's own: 0.0273 over 232 pixels
occupancy: 330 of 4096 box points inside the shapes; the field agrees on 3766 at sigma 20 (the scene's surface level), 3878 at sigma 1
threshold: held-out PSNR at least 20.0 dB: pass
```

The held-out PSNR is 38.0 dB, so the run passes. The training views reach
52.1 dB; the gap of 14 dB is the cost of cameras the fit never saw.

**Memory and time.** The grid is 55,296 binary64 numbers; with AdamW's two
moments that is 1,327,104 bytes, plus 442,368 bytes for the gradient
buffer. The ground-truth images are 16 × 24 × 24 × 3 numbers (221 KB). The
whole release run took 21 to 34 seconds of wall-clock time on one core of
the 28-core Linux machine it was recorded on, depending on what else was running, including about 5 seconds to
compile and run the library's checks. The fit evaluated the field at
3,000 × 256 × 48 = 36.9 million sample points, forward and back; rendering
the four held-out views takes 4 × 576 × 48 = 110,592 field evaluations.
These times are not in the transcript, because they change from run to run.

**Good views are not good geometry.** The depth error on the held-out views
is 0.10, against 0.027 for the ground truth's own soft surface, and the
occupancy line shows why. The scene's density reaches 20 on its surface; the
fitted field never does. It does not need to: with 48 samples over a ray
length of 3, a density near 5 is already opaque within a few samples, so the
images match while every point of the grid stays below the scene's surface
level. At σ = 1 the field agrees with the shapes at 3,878 of 4,096 points,
only 112 more than calling everything empty. So images from new cameras at
38 dB say nothing reliable about where the surface is: they are not evidence
of an accurate collision surface, and the grid is not a usable game asset.
A surface for collision would come from the analytic shapes, from chapter
158's signed distances, or from a field trained with a surface loss and
checked against the shapes directly.

## Refusals

Each refusal returns a status before anything is written:

<!-- source: psionic/examples/radiance-refuse.coax refuse -->
```jai
main :: () {
    c:=RfCamera.{eye=[3]f64.[0,0,3],up=[3]f64.[0,1,0],width=2,height=2,focal=1};
    d:=c;d.focal=0;print("focal length 0: %\n",rf_camera_check(d));
    d=c;d.target=d.eye;print("target at the eye: %\n",rf_camera_check(d));
    d=c;d.up=[3]f64.[0,0,1];print("up along the view: %\n",rf_camera_check(d));
    d=c;d.width=65;print("65 pixels wide: %\n",rf_camera_check(d));
    d=c;d.eye[1]=0.0/0.0;print("a NaN eye: %\n",rf_camera_check(d));
    print("pixel (2, 0) of a 2 x 2 image: %\n",rf_ray(c,2,0).status);

    s:=RfSampling.{near=0,far=1,count=2};sigma:=[2]f64.[1,2];color:=[6]f64.[1,0,0, 0,1,0];bg:=[3]f64.[0,0,1];
    t:=s;t.count=0;print("0 samples: %\n",rf_composite(t,sigma[0..<0],color[0..<0],bg).status);
    t.count=257;print("257 samples: %\n",rf_sampling_check(t));
    t=s;t.far=0;print("far before near: %\n",rf_sampling_check(t));
    n:=sigma;n[1]=-0.5;print("a negative density: %\n",rf_composite(s,n[..],color[..],bg).status);
    n[1]=1.0/0.0;print("an infinite density: %\n",rf_composite(s,n[..],color[..],bg).status);
    three:=[3]f64.[1,2,3];print("3 densities for 2 samples: %\n",rf_composite(s,three[..],color[..],bg).status);
    nan:=[2]f64.[0.0/0.0,1];ds:=[2]f64.[-7,-7];dc:=[6]f64.[-7,-7,-7,-7,-7,-7];
    print("backward with a NaN density: %\n",rf_composite_backward(s,nan[..],color[..],bg,[3]f64.[1,1,1],0,ds[..],dc[..]));
    print("gradient slots still -7: % %\n",ds[0],dc[5]);

    sh:[2]FdShape;co:[6]f64;sc:=rf_demo(sh[..],co[..]);
    bad:=sc;bad.density=0;print("a scene with density 0: %\n",rf_scene_check(bad));
    caps:=sh;caps[1].kind=FdKind.capsule;bad=sc;bad.shapes=caps[..];print("a capsule in the scene: %\n",rf_scene_check(bad));
    bright:=co;bright[0]=1.5;bad=sc;bad.colors=bright[..];print("a colour of 1.5: %\n",rf_scene_check(bad));

    g:=RfGrid.{side=2,low=[3]f64.[-1,-1,-1],high=[3]f64.[1,1,1]};params:[32]f64;
    print("31 numbers for a 2-vertex grid: %\n",rf_grid_check(g,params[0..<31]));
    big:=g;big.side=33;print("33 vertices a side: %\n",rf_grid_check(big,params[..]));
    flat:=g;flat.high[2]=-1;print("a grid with no depth: %\n",rf_grid_check(flat,params[..]));

    cams:[1]RfCamera;cams[0]=c;img:[12]f64;m:[32]f64;v:[32]f64;grads:[32]f64;losses:[4]f64;
    rf_init(g,params[..],-1);cfg:=RfFit.{steps=4,batch=2,lr=0.1,seed=1};
    hot:=img;hot[5]=1.5;print("a target colour of 1.5: %\n",rf_fit(g,params[..],m[..],v[..],grads[..],cams[..],hot[..],s,bg,cfg,losses[..]));
    print("11 numbers for a 2 x 2 view: %\n",rf_fit(g,params[..],m[..],v[..],grads[..],cams[..],img[0..<11],s,bg,cfg,losses[..]));
    fast:=cfg;fast.lr=2;print("a learning rate of 2: %\n",rf_fit(g,params[..],m[..],v[..],grads[..],cams[..],img[..],s,bg,fast,losses[..]));
    wide:=cfg;wide.batch=4097;print("a batch of 4,097 rays: %\n",rf_fit(g,params[..],m[..],v[..],grads[..],cams[..],img[..],s,bg,wide,losses[..]));
    print("parameters still -1 and 0: % %, moments still 0: % %\n",params[0],params[1],m[0],v[0]);
}
```

```text
focal length 0: camera
target at the eye: camera
up along the view: camera
65 pixels wide: limit
a NaN eye: number
pixel (2, 0) of a 2 x 2 image: camera
0 samples: sampling
257 samples: limit
far before near: sampling
a negative density: number
an infinite density: number
3 densities for 2 samples: shape
backward with a NaN density: number
gradient slots still -7: -7.0 -7.0
a scene with density 0: scene
a capsule in the scene: scene
a colour of 1.5: number
31 numbers for a 2-vertex grid: shape
33 vertices a side: limit
a grid with no depth: shape
a target colour of 1.5: number
11 numbers for a 2 x 2 view: shape
a learning rate of 2: number
a batch of 4,097 rays: limit
parameters still -1 and 0: -1.0 0.0, moments still 0: 0.0 0.0
```

The learning rate of 2 is refused by AdamW itself, which checks its
hyperparameters before it writes; the parameters and moments are untouched.

## How it is checked

The checks in [`radiance.coax`](../lib/radiance.coax) come before the code
they test. Each compares the library with something that does not share its
code:

- **Hand values.** The rays, the two-sample composite, the scene's densities
  and colours, exact hits on both shapes, and the grid's interpolation, as
  worked above.
- **A pinhole oracle.** On 32 random cameras, a point 2.5 along a pixel's ray
  projects back to that pixel's centre within 10⁻⁹. The oracle builds its
  camera basis by Gram–Schmidt instead of cross products and divides by depth
  instead of normalising.
- **Closed forms.** A constant-density slab absorbs 1 − exp(−σ d) within
  10⁻¹⁴ on 32 random slabs, step counts and colours. Every composite also
  matches the definition written with T_k = exp(−δ Σ σ) rather than a running
  product.
- **Linear fields.** Trilinear interpolation reproduces a random linear
  function within 10⁻¹³ on grids of 2 to 5 vertices a side.
- **Finite differences.** The compositing gradient matches central
  differences within 10⁻⁸ for colour, depth and every σ, on 24 random rays of
  1 to 7 samples; the gradient of one ray's error through the whole renderer
  matches within 10⁻⁸ for every parameter of a 3-vertex grid, on 8 random
  rays.
- **Signed distances.** Exact hits lie on chapter 158's surfaces.
- **Exact numbers.** A small fit and a small end-to-end run are pinned to
  their losses, PSNRs, depth errors and counts, so a change anywhere in the
  pipeline shows.

`coax mutate psionic/lib/radiance.coax --root-only` changes the library's own
code one site at a time and reruns the checks. All 759 mutants are caught:
none survive, and none is marked equivalent.

Getting there meant removing code no input could distinguish, not adding
exemptions. The box hit takes the slab overlap with chapter 158's exact
minimum and maximum instead of swapping and comparing, and a miss is a large
sentinel folded in with the same minimum; the grid's last cell is a minimum,
not an `if`; a camera's target at its eye is refused by the same NaN test as
an up vector along the view, not a separate length check. Slices whose extra
length could not matter were replaced by whole buffers and an offset. And
refusals in the end-to-end run are checked to have measured nothing, so a
check that a later stage would repeat still shows when it is skipped.

## What this does not do

- **No coordinate network.** The draft of this chapter planned an MLP that
  fits a single image, then a signed-distance field, then a radiance field
  from positions with a frequency encoding. None of that is built; the field
  here is a stored grid. The renderer and its gradient are what an MLP field
  would plug into.
- **No Aethyr renderer.** The training views are rendered by this chapter's
  own volume renderer from analytic shapes, not by the Aethyr renderer
  ([Aethyr chapter 11](../../aethyr/book/ch011-the-wgsl-renderer.md)) from a
  Verse scene, so they have no lighting, textures or hard surfaces.
- **No view dependence.** Colour depends on position only, not on the
  direction a ray arrives from, so shiny surfaces cannot be represented.
- **Fixed samples.** Samples are evenly spaced, the same on every ray; there
  is no jitter between steps and no second, finer pass near surfaces.
- **One core, binary64.** No GPU path, and no automatic differentiation: the
  gradient is written by hand and checked against finite differences rather
  than derived by [chapter 106](ch106-differentiate-any-tensor-program.md)'s
  reverse mode.
- **Same-height cameras.** Every camera sits at height 1.2, so the held-out
  views test new angles around the scene, not views from above or below.

## Dependencies

- [Chapter 158](ch158-a-field-around-a-shape.md): signed distances for the
  ground-truth scene and the checks of `rf_hit`, and the box sampler for
  occupancy.
- [Chapter 124](ch124-adamw-and-explicit-precision.md): the AdamW step.
- [Chapter 53](ch053-choosing-the-next-token.md): the counter hash that
  draws ray batches.
- [Chapter 106](ch106-differentiate-any-tensor-program.md): reverse-mode
  differentiation, which this chapter writes by hand for now.
- The Aethyr renderer
  ([Aethyr chapter 11](../../aethyr/book/ch011-the-wgsl-renderer.md)) for
  future training views.

## Sources

- [Chapter plan](PLAN.md), Part XVI.
- Ben Mildenhall, Pratul P. Srinivasan, Matthew Tancik, Jonathan T. Barron,
  Ravi Ramamoorthi and Ren Ng, "NeRF: Representing Scenes as Neural Radiance
  Fields for View Synthesis", ECCV 2020: the compositing rule used here.
- Sara Fridovich-Keil, Alex Yu, Matthew Tancik, Qinhong Chen, Benjamin Recht
  and Angjoo Kanazawa, "Plenoxels: Radiance Fields without Neural Networks",
  CVPR 2022: a radiance field stored on a grid.
- Max Nelson, "Optical Models for Direct Volume Rendering", IEEE TVCG 1995:
  the absorption–emission model behind the quadrature.
