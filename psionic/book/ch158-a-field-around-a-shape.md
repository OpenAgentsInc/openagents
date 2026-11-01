# 158. A field around a shape

Models that predict flow around a car or an aircraft read the shape as
geometry, not as an image. DoMINO, which [chapter 160](ch160-execute-domino.md) plans to
run, takes a surface mesh and asks of every point in space how far it is from
the surface and on which side, then gathers features from the points near
each query. This chapter builds those operations and checks each one against
an oracle that does not share its code.

It builds five things, in [`field.coax`](../lib/field.coax):

- **signed distances** for a sphere, a box and a capsule in closed form, and
  for a closed triangle mesh, with an outward gradient;
- **mesh admission**: a mesh is measured and refused if it is malformed, flat,
  open, turned inconsistently or inside out;
- **samples**: points on a mesh's surface, uniform by area, and points in a
  box;
- **neighbourhoods**: radius search and k-nearest neighbours on a uniform
  grid, equal to brute-force search to the last index, ties included;
- a **point convolution** with mean or max aggregation, and its gradients by
  hand.

Everything runs in binary64 on one CPU core, with plain sums in a fixed
order. That is the profile `psionic-field-v1`. A mesh holds at most 1,024
vertices and 1,024 triangles; a point set at most 4,096 points; a query
returns at most 64 neighbours; a grid has at most 32 cells along an axis; a
layer has at most 16 input and 16 output channels.

The samplers draw from the counter-based generator `cr_draw` in
[`counter-rng.coax`](../lib/counter-rng.coax), the one
[chapter 53](ch053-choosing-the-next-token.md) samples tokens with, so a seed
fixes every point. The Aethyr engine's spatial
queries ([Aethyr chapter 7](../../aethyr/book/ch007-ask-the-world-where-things-are.md),
[`query.coax`](../../aethyr/query.coax)) answer related questions about
boxes and capsules, but in binary32 world units and with first-hit rather
than nearest-point semantics, so this chapter writes its own binary64
geometry and uses Aethyr's design only as a model.

## Run it

The programs are [`field.coax`](../examples/field.coax) and
[`field-refuse.coax`](../examples/field-refuse.coax); the checks' oracles
and fixtures are in [`field-checks.coax`](../tests/field-checks.coax).

```shell
coax/bin/coax run psionic/examples/field.coax
coax/bin/coax run psionic/examples/field-refuse.coax
coax/bin/coax test psionic/lib/field.coax
coax/bin/coax mutate psionic/lib/field.coax --root-only
```

## The sign convention

A signed distance d(p) is the distance from p to the surface, negative inside,
zero on the surface and positive outside. Away from a few places, where two
surface points are equally near, its gradient has length 1 and points outward:
moving along it by h raises d by h. Where the field has no single direction,
at the centre of a sphere or on a capsule's axis, `grad` is zero.

<!-- source: psionic/lib/field.coax types -->
```jai
FdStatus :: enum u8 {ok;shape;limit;number;parameter;capacity;degenerate;open;inside_out;}
FdKind :: enum u8 {sphere;box;capsule;}
// A sphere: centre a, radius r. A box: low corner a, high corner b. A
// capsule: the segment from a to b, grown by r.
FdShape :: struct {kind:FdKind;a:[3]f64;b:[3]f64;r:f64;}
// d is negative inside, zero on the surface and positive outside. grad is
// the outward unit gradient, or zero where the field has none.
FdDistance :: struct {status:FdStatus;d:f64;grad:[3]f64;closest:[3]f64;triangle:int=-1;}
// Three coordinates a vertex; three vertex indices a triangle, counter-
// clockwise seen from outside.
FdMesh :: struct {vertices:[]f64;triangles:[]int;}
// A mesh that passed admission, with its surface area and enclosed volume.
FdChecked :: struct {status:FdStatus=FdStatus.shape;mesh:FdMesh;area:f64;volume:f64;}
// Points bucketed by cell: cell k holds order[start[k] ..< start[k+1]].
FdGrid :: struct {status:FdStatus=FdStatus.shape;points:[]f64;n:int;low:[3]f64;cell:f64;side:[3]int;start:[]int;order:[]int;}
// count neighbours written; visited is how many distances the search computed.
FdFound :: struct {status:FdStatus;count:int;visited:int;}
FdAggregate :: enum u8 {mean;max;}
// Each output channel: relu(w . [(p_j - p_i) / radius, f_j] + b), then the
// mean or the max over the neighbours j of point i.
FdConv :: struct {channels:int;outputs:int;radius:f64;aggregate:FdAggregate;}
FdResult :: struct {status:FdStatus;written:int;}
```

## Three shapes by hand

Take the sphere with centre (1, 0, 0) and radius 2, and the point (4, 4, 0).
The vector from the centre is v = (3, 4, 0), its length is 5, so
d = 5 − 2 = 3, and the gradient is v / 5 = (0.6, 0.8, 0).

The box from (−1, −1, −1) to (1, 1, 1) has centre 0 and half-size 1. For a
point p, let q = |p| − 1 on each axis: q is positive on an axis where p is
past the box's face.

- At (−4, −5, 0.5), q = (3, 4, −0.5). Outside, only the positive parts
  count: d = √(3² + 4²) = 5, and the gradient is (−3/5, −4/5, 0), with the
  sign of p on each axis.
- At (0, −0.75, 0.5), q = (−1, −0.25, −0.5). Every part is negative, so the
  point is inside, and the nearest face is the one with the largest q:
  d = −0.25 on the y axis, gradient (0, −1, 0), toward the face at y = −1.

The capsule from (0, 0, 0) to (0, 4, 0) with radius 1, at (0, 7, 4): the
point's position along the segment is t = 7/4, clamped to 1, so the nearest
point of the segment is (0, 4, 0). The vector from it is (0, 3, 4), so
d = 5 − 1 = 4 and the gradient is (0, 0.6, 0.8).

<!-- source: psionic/lib/field.coax shapes -->
```jai
fd_shape_check :: (s:FdShape)->FdStatus {
    if !fd_point(s.a) || !fd_point(s.b) || !fd_finite(s.r) {return FdStatus.number;}
    if s.kind==FdKind.box {for i:0..<3 {if !(s.a[i]<s.b[i]) {return FdStatus.parameter;}}return FdStatus.ok;}
    if !(s.r>0) {return FdStatus.parameter;}
    return FdStatus.ok;
}
// v from the nearest point of the core (centre or segment) to p: d = |v| - r.
fd_round :: (v:[3]f64,r:f64)->FdDistance {
    out:FdDistance;l:=fd_norm(v);out.d=l-r;
    if l>0 {out.grad=fd_div(v,l);}
    return out;
}
fd_shape_distance :: (s:FdShape,p:[3]f64)->FdDistance #export_c {
    out:FdDistance;out.status=fd_shape_check(s);
    if out.status==FdStatus.ok && !fd_point(p) {out.status=FdStatus.number;}
    if out.status!=FdStatus.ok {return out;}
    if s.kind==FdKind.sphere {out=fd_round(fd_sub(p,s.a),s.r);}
    else if s.kind==FdKind.capsule {
        ab:=fd_sub(s.b,s.a);l2:=fd_dot(ab,ab);t:=0.0;
        if l2>0 {t=fd_dot(fd_sub(p,s.a),ab)/l2;if t<0 {t=0;}if t>1 {t=1;}}
        out=fd_round(fd_sub(p,fd_add(s.a,fd_scale(ab,t))),s.r);
    } else {
        // q > 0 on an axis where p is past a face. Outside, the distance is
        // |max(q, 0)|; inside, it is the largest q (the nearest face).
        u:[3]f64;q:[3]f64;o:=0.0;
        for i:0..<3 {u[i]=p[i]-(s.a[i]+s.b[i])/2;q[i]=abs(u[i])-(s.b[i]-s.a[i])/2;if q[i]>0 {o+=q[i]*q[i];}}
        if o>0 {
            o=sqrt(o);out.d=o;
            for i:0..<3 {if q[i]>0 {out.grad[i]=ifx u[i]<0 then -q[i]/o else q[i]/o;}}
        } else {
            k:=0;if q[1]>q[k] {k=1;}if q[2]>q[k] {k=2;}
            out.d=q[k];out.grad[k]=ifx u[k]<0 then -1.0 else 1.0;
        }
    }
    return out;
}
```

The program prints the same four answers:

<!-- source: psionic/examples/field.coax shapes -->
```jai
    sphere:=FdShape.{kind=FdKind.sphere,a=[3]f64.[1,0,0],r=2};
    box:=FdShape.{kind=FdKind.box,a=[3]f64.[-1,-1,-1],b=[3]f64.[1,1,1]};
    capsule:=FdShape.{kind=FdKind.capsule,a=[3]f64.[0,0,0],b=[3]f64.[0,4,0],r=1};
    show("sphere at (4, 4, 0)",fd_shape_distance(sphere,[3]f64.[4,4,0]));
    show("box at (-4, -5, 0.5)",fd_shape_distance(box,[3]f64.[-4,-5,0.5]));
    show("box at (0, -0.75, 0.5)",fd_shape_distance(box,[3]f64.[0,-0.75,0.5]));
    show("capsule at (0, 7, 4)",fd_shape_distance(capsule,[3]f64.[0,7,4]));
    
```

```text
sphere at (4, 4, 0): d 3.0 grad (0.6, 0.8, 0.0)
box at (-4, -5, 0.5): d 5.0 grad (-0.6, -0.8, 0.0)
box at (0, -0.75, 0.5): d -0.25 grad (0.0, -1.0, 0.0)
capsule at (0, 7, 4): d 4.0 grad (0.0, 0.6, 0.8)
```

## A mesh, admitted

A triangle mesh is a list of vertices and a list of triangles, three vertex
indices each, counter-clockwise seen from outside. Admission checks, in this
order, and refuses with the first status that applies:

1. `shape`: the counts are not multiples of three, or a list is empty, or an
   index names no vertex.
2. `limit`: more than 1,024 vertices or triangles.
3. `number`: a coordinate is NaN, infinite or larger than 10⁶.
4. `degenerate`: a triangle is flat. With edges e1 and e2 from its first
   corner, |e1 × e2|² = sin²θ |e1|²|e2|², so the triangle is flat when
   |e1 × e2|² ≤ 2⁻⁶⁰ |e1|²|e2|². A repeated corner or a zero-length edge
   gives 0 ≤ 0 and is flat too.
5. `open`: some directed edge u → v does not appear exactly once with its
   reverse v → u exactly once. That one rule catches a hole, a triangle
   turned over, and an edge shared by three triangles.
6. `inside_out`: the enclosed volume is not positive. By the divergence
   theorem the volume is the sum over triangles of a · (b × c) / 6; a closed
   mesh turned the wrong way gives a negative sum, a doubled sheet gives 0.

<!-- source: psionic/lib/field.coax mesh -->
```jai
fd_corner :: (m:FdMesh,t:int,k:int)->[3]f64 {return fd_at(m.vertices,m.triangles[3*t+k]);}
// sin^2 of the angle at a, times |ab|^2 |ac|^2, is |ab x ac|^2.
fd_flat :: (a:[3]f64,b:[3]f64,c:[3]f64)->bool {
    e1:=fd_sub(b,a);e2:=fd_sub(c,a);n:=fd_cross(e1,e2);
    return fd_dot(n,n)<=FD_FLAT*fd_dot(e1,e1)*fd_dot(e2,e2);
}
fd_mesh_admit :: (m:FdMesh)->FdChecked #export_c {
    out:FdChecked;nv:=m.vertices.count/3;nt:=m.triangles.count/3;
    if nv==0 || nt==0 || m.vertices.count%3!=0 || m.triangles.count%3!=0 {return out;}
    if nv>FD_VERTICES_MAX || nt>FD_TRIANGLES_MAX {out.status=FdStatus.limit;return out;}
    for x:m.vertices {if !fd_finite(x) {out.status=FdStatus.number;return out;}}
    for k:m.triangles {if k<0 || k>=nv {return out;}}
    for t:0..<nt {if fd_flat(fd_corner(m,t,0),fd_corner(m,t,1),fd_corner(m,t,2)) {out.status=FdStatus.degenerate;return out;}}
    // Closed and consistently turned: each directed edge u -> v appears
    // once, and so does v -> u, in the triangle across it.
    for t:0..<nt {for e:0..<3 {
        u:=m.triangles[3*t+e];v:=m.triangles[3*t+(e+1)%3];same:=0;back:=0;
        for s:0..<nt {for f:0..<3 {
            x:=m.triangles[3*s+f];y:=m.triangles[3*s+(f+1)%3];
            if x==u && y==v {same+=1;}if x==v && y==u {back+=1;}
        }}
        if same!=1 || back!=1 {out.status=FdStatus.open;return out;}
    }}
    // Area, and volume by the divergence theorem: the sum of a . (b x c) / 6.
    area:=0.0;volume:=0.0;
    for t:0..<nt {
        a:=fd_corner(m,t,0);b:=fd_corner(m,t,1);c:=fd_corner(m,t,2);
        area+=fd_norm(fd_cross(fd_sub(b,a),fd_sub(c,a)))/2;volume+=fd_dot(a,fd_cross(b,c))/6;
    }
    if !(volume>0) {out.status=FdStatus.inside_out;return out;}
    out.status=FdStatus.ok;out.mesh=m;out.area=area;out.volume=volume;return out;
}
```

For the cube from −0.5 to 0.5, each of the 12 triangles is half of a unit
square, so the area is 12 × 0.5 = 6. Each term a · (b × c) / 6 is the signed
volume of the tetrahedron from the origin to the triangle: a third of the
triangle's area times the height of its plane above the origin. The two
triangles of one face have area 1 together and height 0.5, so each face adds
1 × 0.5 / 3 = 1/6, and six faces give 1, the cube's volume.

## The nearest point

The unsigned distance to the mesh is the smallest distance to any of its
triangles. The nearest point of one triangle is found by Ericson's method:
the dot products of p's offsets with the two edges decide which of seven
regions p projects into, three corners, three edges or the face, and each
region has its own closed form. The first triangle wins a tie.

<!-- source: psionic/lib/field.coax closest -->
```jai
// The point of triangle abc nearest p (Ericson, Real-Time Collision
// Detection, 5.1.5): test the corner regions, then the edges, then the face.
fd_closest_triangle :: (p:[3]f64,a:[3]f64,b:[3]f64,c:[3]f64)->[3]f64 {
    ab:=fd_sub(b,a);ac:=fd_sub(c,a);ap:=fd_sub(p,a);
    d1:=fd_dot(ab,ap);d2:=fd_dot(ac,ap);
    if d1<=0 && d2<=0 {return a;}
    bp:=fd_sub(p,b);d3:=fd_dot(ab,bp);d4:=fd_dot(ac,bp);
    if d3>=0 && d4<=d3 {return b;}
    vc:=d1*d4-d3*d2;
    if vc<=0 && d1>=0 && d3<=0 {return fd_add(a,fd_scale(ab,d1/(d1-d3)));}
    cp:=fd_sub(p,c);d5:=fd_dot(ab,cp);d6:=fd_dot(ac,cp);
    if d6>=0 && d5<=d6 {return c;}
    vb:=d5*d2-d1*d6;
    if vb<=0 && d2>=0 && d6<=0 {return fd_add(a,fd_scale(ac,d2/(d2-d6)));}
    va:=d3*d6-d5*d4;
    if va<=0 && d4-d3>=0 && d5-d6>=0 {return fd_add(b,fd_scale(fd_sub(c,b),(d4-d3)/((d4-d3)+(d5-d6))));}
    s:=va+vb+vc;
    return fd_add(a,fd_add(fd_scale(ab,vb/s),fd_scale(ac,vc/s)));
}
```

For the triangle (0, 0, 0), (4, 0, 0), (0, 4, 0) and the point (2, −3, 1),
ab · ap = 8 and ac · ap = −12, so the corner a is not nearest; from b,
ab · bp = −8 and ac · bp = −12, so b is not; vc = d1·d4 − d3·d2 =
8 · (−12) − (−8)(−12) = −192 ≤ 0 with d1 = 8 ≥ 0 and d3 = −8 ≤ 0, so the
point projects onto edge ab, at a + ab · 8 / 16 = (2, 0, 0).

## Inside or outside

The sign comes from the winding number: the solid angle each triangle covers,
seen from p, summed over the mesh and divided by 4π. Inside a closed,
outward-turned mesh it is 1; outside, 0. Van Oosterom and Strackee's formula
gives each triangle's solid angle Ω from its corners x, y, z relative to p:

tan(Ω/2) = x · (y × z) / (|x||y||z| + (x·y)|z| + (x·z)|y| + (y·z)|x|).

A point is inside when the winding number exceeds 0.5. Unlike casting a ray
and counting crossings, nothing goes wrong when p lines up with an edge or a
vertex.

<!-- source: psionic/lib/field.coax winding -->
```jai
// The solid angle triangle abc covers seen from p (Van Oosterom and
// Strackee), summed over the mesh and divided by 4 pi: 1 inside, 0 outside.
fd_winding :: (m:FdChecked,p:[3]f64)->f64 {
    sum:=0.0;
    for t:0..<m.mesh.triangles.count/3 {
        x:=fd_sub(fd_corner(m.mesh,t,0),p);y:=fd_sub(fd_corner(m.mesh,t,1),p);z:=fd_sub(fd_corner(m.mesh,t,2),p);
        lx:=fd_norm(x);ly:=fd_norm(y);lz:=fd_norm(z);
        num:=fd_dot(x,fd_cross(y,z));
        den:=lx*ly*lz+fd_dot(x,y)*lz+fd_dot(x,z)*ly+fd_dot(y,z)*lx;
        sum+=2*fd_atan2(num,den);
    }
    return sum/(4*FD_PI);
}
```

Coax has no arctangent, so the chapter writes one. Three halvings,
atan z = 2 atan(z / (1 + √(1 + z²))), bring |z| ≤ 1 below tan(π/32) ≈ 0.0985.
There eight terms of z − z³/3 + z⁵/5 − … reach the last bit: the ninth term
would be below 0.0985¹⁷/17 ≈ 4.5 × 10⁻¹⁹, under half a unit in the last place
of the result. `fd_atan2` places the angle in its quadrant.

<!-- source: psionic/lib/field.coax atan -->
```jai
// atan z for |z| <= 1: three halvings, atan z = 2 atan(z / (1 + sqrt(1 + z^2))),
// bring |z| below tan(pi/32) = 0.0985; then eight terms of z - z^3/3 + z^5/5 ...
fd_atan_unit :: (x:f64)->f64 {
    z:=x;for k:0..<3 {z=z/(1+sqrt(1+z*z));}
    z2:=z*z;s:=0.0;
    for k:0..<8 {j:=7-k;term:=1/f64(2*j+1);s=s*z2+(ifx j%2==1 then -term else term);}
    return 8*z*s;
}
fd_atan2 :: (y:f64,x:f64)->f64 {
    if x==0 && y==0 {return 0;}
    if abs(y)<=abs(x) {
        t:=fd_atan_unit(y/x);if x>0 {return t;}
        return ifx y>=0 then t+FD_PI else t-FD_PI;
    }
    t:=fd_atan_unit(x/y);
    return ifx y>0 then FD_PI/2-t else -FD_PI/2-t;
}
```

The distance puts the two together. On the surface it returns the triangle's
unit normal as the gradient; inside, the gradient points from p to the
nearest point, which is outward.

<!-- source: psionic/lib/field.coax distance -->
```jai
fd_mesh_distance :: (m:FdChecked,p:[3]f64)->FdDistance #export_c {
    out:FdDistance;out.status=m.status;
    if out.status==FdStatus.ok && !fd_point(p) {out.status=FdStatus.number;}
    if out.status!=FdStatus.ok {return out;}
    best:=0.0;
    for t:0..<m.mesh.triangles.count/3 {
        q:=fd_closest_triangle(p,fd_corner(m.mesh,t,0),fd_corner(m.mesh,t,1),fd_corner(m.mesh,t,2));
        v:=fd_sub(p,q);d2:=fd_dot(v,v);
        if out.triangle<0 || d2<best {best=d2;out.triangle=t;out.closest=q;}
    }
    dist:=sqrt(best);
    if dist==0 {
        a:=fd_corner(m.mesh,out.triangle,0);n:=fd_cross(fd_sub(fd_corner(m.mesh,out.triangle,1),a),fd_sub(fd_corner(m.mesh,out.triangle,2),a));
        out.grad=fd_div(n,fd_norm(n));return out;
    }
    // Inside, the field grows toward the surface: the gradient points from p to it.
    if fd_winding(m,p)>0.5 {out.d=-dist;out.grad=fd_div(fd_sub(out.closest,p),dist);}
    else {out.d=dist;out.grad=fd_div(fd_sub(p,out.closest),dist);}
    return out;
}
```

By hand, for the cube from −0.5 to 0.5:

- At (1, 0, 0), the nearest point is (0.5, 0, 0) on the face x = 0.5, so
  d = 0.5 and the gradient is (1, 0, 0).
- At (1, 1, 0), the nearest point is (0.5, 0.5, 0) on the edge between the
  faces x = 0.5 and y = 0.5, so d = √(0.25 + 0.25) = √0.5 ≈ 0.7071068 and the
  gradient is (1, 1, 0)/√2.
- At (0.25, 0, 0.375), the nearest face is z = 0.5, 0.125 away. The point is
  inside, so d = −0.125, and the gradient points from p to (0.25, 0, 0.5):
  (0, 0, 1).

<!-- source: psionic/examples/field.coax mesh -->
```jai
    v:=fd_cube(-0.5,0.5);t:=FD_CUBE;
    m:=fd_mesh_admit(FdMesh.{vertices=v[..],triangles=t[..]});
    print("cube mesh: % (area %, volume %)\n",m.status,m.area,m.volume);
    show("mesh at (1, 0, 0)",fd_mesh_distance(m,[3]f64.[1,0,0]));
    show("mesh at (1, 1, 0)",fd_mesh_distance(m,[3]f64.[1,1,0]));
    show("mesh at (0.25, 0, 0.375)",fd_mesh_distance(m,[3]f64.[0.25,0,0.375]));
    print("winding number at (0.1, 0.2, -0.3): %, at (2, 0.2, -0.3): %\n",
        fd_winding(m,[3]f64.[0.1,0.2,-0.3]),fd_winding(m,[3]f64.[2,0.2,-0.3]));
    
```

```text
cube mesh: ok (area 6.0, volume 1.0)
mesh at (1, 0, 0): d 0.5 grad (1.0, 0.0, 0.0)
mesh at (1, 1, 0): d 0.7071067811865476 grad (0.7071067811865475, 0.7071067811865475, 0.0)
mesh at (0.25, 0, 0.375): d -0.125 grad (0.0, 0.0, 1.0)
winding number at (0.1, 0.2, -0.3): 0.9999999999999997, at (2, 0.2, -0.3): -2.208718528794109e-18
```

The winding numbers miss 1 and 0 by a few units of rounding, far from the
0.5 threshold.

## Samples

Surface samples are uniform by area. One draw u, scaled by the total area,
picks the first triangle whose running area exceeds it. Two more draws r1 and
r2 place the point at the barycentric weights (1 − √r1, √r1(1 − r2), √r1 r2).
Without the square root, points would crowd the first corner: half of them
would land in the quarter of the triangle nearest it. Each sample's normal is
its triangle's unit normal. Box samples take one draw per coordinate.

<!-- source: psionic/lib/field.coax sample -->
```jai
fd_draw :: (r:[]CrSeed)->f64 {d:=cr_draw(r[0]);r[0]=d.next;return d.value;}
fd_sample_count :: (points:[]f64)->FdStatus {
    if points.count==0 || points.count%3!=0 {return FdStatus.shape;}
    if points.count/3>FD_POINTS_MAX {return FdStatus.limit;}
    return FdStatus.ok;
}
// Area-weighted: one draw picks a triangle, two more a point in it, uniform
// by area (the square root keeps points from crowding corner a).
fd_sample_surface :: (m:FdChecked,seed:u64,points:[]f64,normals:[]f64)->FdResult {
    out:FdResult;out.status=m.status;
    if out.status==FdStatus.ok {out.status=fd_sample_count(points);}
    if out.status==FdStatus.ok && normals.count!=points.count {out.status=FdStatus.shape;}
    if out.status!=FdStatus.ok {return out;}
    r:=[1]CrSeed.[CrSeed.{seed=seed}];nt:=m.mesh.triangles.count/3;
    for i:0..<points.count/3 {
        u:=fd_draw(r[..])*m.area;t:=nt-1;acc:=0.0;
        for s:0..<nt {
            a:=fd_corner(m.mesh,s,0);acc+=fd_norm(fd_cross(fd_sub(fd_corner(m.mesh,s,1),a),fd_sub(fd_corner(m.mesh,s,2),a)))/2;
            if u<acc {t=s;break;}
        }
        a:=fd_corner(m.mesh,t,0);b:=fd_corner(m.mesh,t,1);c:=fd_corner(m.mesh,t,2);
        s:=sqrt(fd_draw(r[..]));v:=fd_draw(r[..]);
        p:=fd_add(fd_add(fd_scale(a,1-s),fd_scale(b,s*(1-v))),fd_scale(c,s*v));
        n:=fd_cross(fd_sub(b,a),fd_sub(c,a));n=fd_div(n,fd_norm(n));
        for k:0..<3 {points[3*i+k]=p[k];normals[3*i+k]=n[k];}
    }
    out.written=points.count/3;return out;
}
fd_sample_box :: (low:[3]f64,high:[3]f64,seed:u64,points:[]f64)->FdResult {
    out:FdResult;
    if !fd_point(low) || !fd_point(high) {out.status=FdStatus.number;return out;}
    for k:0..<3 {if !(low[k]<high[k]) {out.status=FdStatus.parameter;return out;}}
    out.status=fd_sample_count(points);if out.status!=FdStatus.ok {return out;}
    r:=[1]CrSeed.[CrSeed.{seed=seed}];
    for i:0..<points.count {k:=i%3;points[i]=low[k]+(high[k]-low[k])*fd_draw(r[..]);}
    out.written=points.count/3;return out;
}
```

The seed is an input. The same seed gives the same points, and a run of one
sample gives the first sample of a run of 1,024.

<!-- source: psionic/examples/field.coax samples -->
```jai
    surface:[3072]f64;normals:[3072]f64;
    print("surface samples: %\n",fd_sample_surface(m,5,surface[..],normals[..]).written);
    worst:=0.0;for i:0..<1024 {d:=abs(fd_mesh_distance(m,fd_at(surface[..],i)).d);if d>worst {worst=d;}}
    print("largest |d| at a surface sample: %\n",worst);
    cloud:[1536]f64;print("box samples: %\n",fd_sample_box([3]f64.[-1,-1,-1],[3]f64.[1,1,1],9,cloud[..]).written);
    inside:=0;agree:=0;
    for i:0..<512 {
        p:=fd_at(cloud[..],i);mesh:=fd_mesh_distance(m,p).d<0;
        if mesh {inside+=1;}if mesh==(fd_shape_distance(FdShape.{kind=FdKind.box,a=[3]f64.[-0.5,-0.5,-0.5],b=[3]f64.[0.5,0.5,0.5]},p).d<0) {agree+=1;}
    }
    print("inside the cube: % of 512 (one eighth is 64); mesh and box signs agree on %\n",inside,agree);
    
```

```text
surface samples: 1024
largest |d| at a surface sample: 1.6711069443220838e-16
box samples: 512
inside the cube: 66 of 512 (one eighth is 64); mesh and box signs agree on 512
```

Every surface sample has |d| within 2 × 10⁻¹⁶ of zero. Of 512 points in the
box from −1 to 1, 66 fall inside the cube, whose volume is one eighth of the
box's; the expected count is 64 with a standard deviation of 7.5. On every one
of the 512 points the mesh's sign agrees with the closed-form box.

## Neighbourhoods on a grid

A radius query asks for every point within r of q; a k-nearest query for the
k nearest. Brute force computes every distance. A uniform grid computes far
fewer: it buckets the points into cubic cells of a chosen size, then visits
only the cells a query can reach.

The grid is built by a counting sort: count the points in each cell, turn the
counts into starting offsets, place the points in index order, then shift the
offsets back. Cell k then holds `order[start[k] ..< start[k+1]]`. A query
point outside the grid is clamped onto its edge cells.

<!-- source: psionic/lib/field.coax grid -->
```jai
fd_points_status :: (points:[]f64)->FdStatus {
    status:=fd_sample_count(points);if status!=FdStatus.ok {return status;}
    for x:points {if !fd_finite(x) {return FdStatus.number;}}
    return FdStatus.ok;
}
fd_grid_build :: (points:[]f64,cell:f64,start:[]int,order:[]int)->FdGrid {
    g:FdGrid;g.status=fd_points_status(points);
    if g.status==FdStatus.ok && !(cell>0 && fd_finite(cell)) {g.status=FdStatus.parameter;}
    if g.status!=FdStatus.ok {return g;}
    n:=points.count/3;g.low=fd_at(points,0);high:=g.low;
    for i:1..<n {for k:0..<3 {x:=points[3*i+k];if x<g.low[k] {g.low[k]=x;}if x>high[k] {high[k]=x;}}}
    cells:=1;
    for k:0..<3 {
        span:=floor((high[k]-g.low[k])/cell);
        if span>=FD_SIDE_MAX {g.status=FdStatus.limit;return g;}
        g.side[k]=cast(int)span+1;cells*=g.side[k];
    }
    if start.count<cells+1 || order.count<n {g.status=FdStatus.capacity;return g;}
    g.points=points;g.n=n;g.cell=cell;g.start=start;g.order=order;
    // Counting sort: count each cell, turn counts into starts, place points
    // in index order, then shift the advanced starts back by one cell.
    for k:0..<cells+1 {start[k]=0;}
    for i:0..<n {start[fd_cell_index(g,fd_cell(g,fd_at(points,i)))+1]+=1;}
    for k:0..<cells {start[k+1]+=start[k];}
    for i:0..<n {k:=fd_cell_index(g,fd_cell(g,fd_at(points,i)));order[start[k]]=i;start[k]+=1;}
    for j:0..<cells {k:=cells-j;start[k]=start[k-1];}
    start[0]=0;return g;
}
// The cell holding p, clamped into the grid: a far query lands on the edge.
fd_cell :: (g:FdGrid,p:[3]f64)->[3]int {
    c:[3]int;
    for k:0..<3 {x:=floor((p[k]-g.low[k])/g.cell);if x<0 {x=0;}if x>f64(g.side[k]-1) {x=f64(g.side[k]-1);}c[k]=cast(int)x;}
    return c;
}
fd_cell_index :: (g:FdGrid,c:[3]int)->int {return (c[2]*g.side[1]+c[1])*g.side[0]+c[0];}
```

Take five points with cell size 1: (0, 0, 0), (0.9, 0, 0), (0.1, 0.2, 0),
(2, 1, 0.5) and (0.95, 0.05, 0). Their bounding box runs from (0, 0, 0) to
(2, 1, 0.5), so the grid has ⌊2/1⌋ + 1 = 3 cells in x, ⌊1/1⌋ + 1 = 2 in y and
⌊0.5/1⌋ + 1 = 1 in z. Points 0, 1, 2 and 4 fall in cell (0, 0, 0), index 0;
point 3 falls in cell (2, 1, 0), index (0 · 2 + 1) · 3 + 2 = 5. The counts
are 4 in cell 0 and 1 in cell 5, so the starts are 0, 4, 4, 4, 4, 4, 5 and
`order` is 0, 1, 2, 4, 3.

The answer must not depend on how the points were searched, so neighbours are
ordered by squared distance and then by index. A list is kept sorted as it is
built, and a radius query that finds more than 64 points is refused rather
than cut short.

<!-- source: psionic/lib/field.coax order -->
```jai
// Neighbours are ordered by squared distance, then by index: one answer for
// the grid and for brute force, ties included.
fd_before :: (da:f64,ia:int,db:f64,ib:int)->bool {return da<db || (da==db && ia<ib);}
// Insert (d, i) into the sorted list of count entries, keeping at most keep.
fd_insert :: (idx:[]int,ds:[]f64,count:int,keep:int,i:int,d:f64)->int {
    pos:=count;while pos>0 && fd_before(d,i,ds[pos-1],idx[pos-1]) {pos-=1;}
    if pos>=keep {return count;}
    last:=ifx count<keep then count else keep-1;
    j:=last;while j>pos {idx[j]=idx[j-1];ds[j]=ds[j-1];j-=1;}
    idx[pos]=i;ds[pos]=d;
    return ifx count<keep then count+1 else count;
}
fd_d2 :: (points:[]f64,i:int,q:[3]f64)->f64 {v:=fd_sub(fd_at(points,i),q);return fd_dot(v,v);}
```

A radius query visits the cells that the box from q − r to q + r touches,
widened by one cell on each side so that a point whose cell index rounded
across a boundary is still seen. With the five points, the query (0.9, 0, 0)
with r = 0.1 finds point 1 at distance 0 and point 4 at squared distance
0.05² + 0.05² = 0.005 ≤ 0.01.

<!-- source: psionic/lib/field.coax radius -->
```jai
FdHits :: struct {idx:[64]int;ds:[64]f64;count:int;found:int;visited:int;}
fd_radius_hit :: (h:[]FdHits,points:[]f64,i:int,q:[3]f64,r2:f64) {
    d:=fd_d2(points,i,q);h[0].visited+=1;
    if d<=r2 {h[0].found+=1;h[0].count=fd_insert(h[0].idx[..],h[0].ds[..],h[0].count,FD_NEIGHBORS_MAX,i,d);}
}
fd_hits_out :: (h:FdHits,out:[]int)->FdFound {
    f:=FdFound.{visited=h.visited};
    if h.found>FD_NEIGHBORS_MAX {f.status=FdStatus.limit;return f;}
    if h.count>out.count {f.status=FdStatus.capacity;return f;}
    for i:0..<h.count {out[i]=h.idx[i];}
    f.count=h.count;return f;
}
fd_query_status :: (q:[3]f64,r:f64)->FdStatus {
    if !fd_point(q) {return FdStatus.number;}
    if !(r>0 && fd_finite(r)) {return FdStatus.parameter;}
    return FdStatus.ok;
}
// Every point within r of q (distance squared at most r^2), nearest first.
// The cells searched reach one past q - r and q + r on each axis, so a point
// rounding into a neighbouring cell is still seen.
fd_radius :: (g:FdGrid,q:[3]f64,r:f64,out:[]int)->FdFound {
    f:FdFound;f.status=g.status;
    if f.status==FdStatus.ok {f.status=fd_query_status(q,r);}
    if f.status!=FdStatus.ok {return f;}
    lo:=fd_cell(g,fd_sub(q,[3]f64.[r,r,r]));hi:=fd_cell(g,fd_add(q,[3]f64.[r,r,r]));
    for k:0..<3 {if lo[k]>0 {lo[k]-=1;}if hi[k]<g.side[k]-1 {hi[k]+=1;}}
    h:[1]FdHits;
    for z:lo[2]..<hi[2]+1 {for y:lo[1]..<hi[1]+1 {for x:lo[0]..<hi[0]+1 {
        k:=fd_cell_index(g,[3]int.[x,y,z]);
        for s:g.start[k]..<g.start[k+1] {fd_radius_hit(h[..],g.points,g.order[s],q,r*r);}
    }}}
    return fd_hits_out(h[0],out);
}
fd_radius_brute :: (points:[]f64,q:[3]f64,r:f64,out:[]int)->FdFound {
    f:FdFound;f.status=fd_points_status(points);
    if f.status==FdStatus.ok {f.status=fd_query_status(q,r);}
    if f.status!=FdStatus.ok {return f;}
    h:[1]FdHits;for i:0..<points.count/3 {fd_radius_hit(h[..],points,i,q,r*r);}
    return fd_hits_out(h[0],out);
}
```

A k-nearest query visits rings of cells around q's cell, nearest first, and
stops when no unvisited point can be nearer than the k-th found. The stopping
bound keeps a full cell of margin, so floating-point rounding in a cell index
cannot hide a nearer point.

<!-- source: psionic/lib/field.coax knn -->
```jai
fd_knn_status :: (n:int,q:[3]f64,k:int,out:[]int)->FdStatus {
    if !fd_point(q) {return FdStatus.number;}
    if k>FD_NEIGHBORS_MAX {return FdStatus.limit;}
    if k<1 || k>n {return FdStatus.parameter;}
    if out.count<k {return FdStatus.capacity;}
    return FdStatus.ok;
}
// The k nearest points: visit rings of cells around q's cell, nearest ring
// first. After ring R, every point not yet visited is at least R cells from
// q, less rounding. The search stops once the k-th distance is under R - 1
// cells, a full cell of margin; reach * |reach| keeps the sign of R - 1, so
// rings 0 and 1 never stop it.
fd_knn_visit :: (h:[]FdHits,g:FdGrid,x:int,y:int,z:int,q:[3]f64,k:int) {
    if x<0 || x>=g.side[0] {return;}
    cell:=fd_cell_index(g,[3]int.[x,y,z]);
    for s:g.start[cell]..<g.start[cell+1] {
        i:=g.order[s];h[0].visited+=1;h[0].count=fd_insert(h[0].idx[..],h[0].ds[..],h[0].count,k,i,fd_d2(g.points,i,q));
    }
}
fd_knn :: (g:FdGrid,q:[3]f64,k:int,out:[]int)->FdFound {
    f:FdFound;f.status=g.status;
    if f.status==FdStatus.ok {f.status=fd_knn_status(g.n,q,k,out);}
    if f.status!=FdStatus.ok {return f;}
    c:=fd_cell(g,q);h:[1]FdHits;
    rings:=g.side[0];if g.side[1]>rings {rings=g.side[1];}if g.side[2]>rings {rings=g.side[2];}
    for ring:0..<rings {
        lo:[3]int;hi:[3]int;
        for a:0..<3 {lo[a]=c[a]-ring;if lo[a]<0 {lo[a]=0;}hi[a]=c[a]+ring;if hi[a]>g.side[a]-1 {hi[a]=g.side[a]-1;}}
        for z:lo[2]..<hi[2]+1 {for y:lo[1]..<hi[1]+1 {
            // On the ring's y or z faces every x belongs to it; inside them, only its two x ends.
            if z==c[2]-ring || z==c[2]+ring || y==c[1]-ring || y==c[1]+ring {for x:lo[0]..<hi[0]+1 {fd_knn_visit(h[..],g,x,y,z,q,k);}}
            else {fd_knn_visit(h[..],g,c[0]-ring,y,z,q,k);fd_knn_visit(h[..],g,c[0]+ring,y,z,q,k);}
        }}
        reach:=f64(ring-1)*g.cell;
        if h[0].count==k && h[0].ds[k-1]<reach*abs(reach) {break;}
    }
    for i:0..<k {out[i]=h[0].idx[i];}
    f.count=k;f.visited=h[0].visited;return f;
}

fd_knn_brute :: (points:[]f64,q:[3]f64,k:int,out:[]int)->FdFound {
    f:FdFound;f.status=fd_points_status(points);
    if f.status==FdStatus.ok {f.status=fd_knn_status(points.count/3,q,k,out);}
    if f.status!=FdStatus.ok {return f;}
    idx:[64]int;ds:[64]f64;count:=0;
    for i:0..<points.count/3 {f.visited+=1;count=fd_insert(idx[..],ds[..],count,k,i,fd_d2(points,i,q));}
    for i:0..<k {out[i]=idx[i];}
    f.count=k;return f;
}
```

The program builds a 16 × 16 × 16 grid over 4,096 points in the cube from −1
to 1, asks for the 8 nearest neighbours of 256 of them, and compares each list
with brute force.

<!-- source: psionic/examples/field.coax neighbours -->
```jai
    points:[12288]f64;fd_cloud(3,4096,points[..]);start:[32769]int;order:[4096]int;
    g:=fd_grid_build(points[..],0.125,start[..],order[..]);
    print("grid: % points, % x % x % cells\n",g.n,g.side[0],g.side[1],g.side[2]);
    a:[64]int;b:[64]int;same:=0;grid_work:=0;brute_work:=0;
    for i:0..<256 {
        q:=fd_at(points[..],i*16);
        x:=fd_knn(g,q,8,a[..]);y:=fd_knn_brute(points[..],q,8,b[..]);
        equal:=x.status==FdStatus.ok && y.status==FdStatus.ok;for j:0..<8 {if a[j]!=b[j] {equal=false;}}
        if equal {same+=1;}grid_work+=x.visited;brute_work+=y.visited;
    }
    print("8 nearest of 256 points: grid equals brute force for %, distances computed % against %\n",same,grid_work,brute_work);
    q:=[3]f64.[0.1,-0.2,0.3];
    x:=fd_radius(g,q,0.1,a[..]);y:=fd_radius_brute(points[..],q,0.1,b[..]);
    print("within 0.1 of (0.1, -0.2, 0.3): grid % (% visited), brute force % (% visited)\n",x.count,x.visited,y.count,y.visited);
    for j:0..<x.count {print("  point %, distance %\n",a[j],sqrt(fd_d2(points[..],a[j],q)));}
    
```

```text
grid: 4096 points, 16 x 16 x 16 cells
8 nearest of 256 points: grid equals brute force for 256, distances computed 55760 against 1048576
within 0.1 of (0.1, -0.2, 0.3): grid 3 (133 visited), brute force 3 (4096 visited)
  point 600, distance 0.013767311189304743
  point 2129, distance 0.07277768313210642
  point 3771, distance 0.08635929580227118
```

All 256 lists are equal, index for index, and the grid computed about 5% of
the distances. A radius query with r = 0.1 spans at most two cells of 0.125
on each axis, four with the margin, so it visits at most 4 × 4 × 4 = 64
cells; here it computed 133 distances against 4,096.

## A point convolution

A point convolution turns per-point features into new features by looking at
each point's neighbourhood. For centre i, neighbour j and output channel o:

h = b[o] + Σ_d w[o, d] (p_j[d] − p_i[d]) / radius + Σ_c w[o, 3 + c] f_j[c],

and out[i, o] is the mean or the maximum over the neighbours of relu(h). The
first three weights see where the neighbour is, relative to the centre and in
units of the radius; the rest see its features. The neighbour lists come from
a radius table, built from the grid.

<!-- source: psionic/lib/field.coax table -->
```jai
// Each point's radius neighbours, nearest first, in rows of width slots;
// unused slots hold -1.
fd_radius_table :: (g:FdGrid,r:f64,width:int,nbr:[]int,counts:[]int)->FdResult {
    out:FdResult;out.status=g.status;
    if out.status==FdStatus.ok && width>FD_NEIGHBORS_MAX {out.status=FdStatus.limit;}
    if out.status==FdStatus.ok && width<1 {out.status=FdStatus.parameter;}
    if out.status==FdStatus.ok && (nbr.count<g.n*width || counts.count<g.n) {out.status=FdStatus.capacity;}
    if out.status!=FdStatus.ok {return out;}
    // Every row is asked first, so a refusal (a bad radius, too many
    // neighbours) leaves the table untouched.
    row:[64]int;
    for i:0..<g.n {
        f:=fd_radius(g,fd_at(g.points,i),r,row[0..<width]);
        if f.status!=FdStatus.ok {out.status=f.status;return out;}
    }
    for i:0..<g.n {
        f:=fd_radius(g,fd_at(g.points,i),r,row[0..<width]);counts[i]=f.count;
        for s:0..<width {nbr[i*width+s]=ifx s<f.count then row[s] else -1;}
    }
    out.written=g.n;return out;
}
```

The gradient of a loss L = Σ dout ⊙ out follows the same terms. Each
neighbour whose h is positive passes back a share g of dout[i, o]: 1/count for
the mean, 1 for the max's chosen neighbour (the first of equal values). That
share adds g to the bias, g times the input to each weight, and g times the
weight to each feature.

<!-- source: psionic/lib/field.coax conv -->
```jai
fd_conv_check :: (c:FdConv,points:[]f64,features:[]f64,nbr:[]int,counts:[]int,width:int,w:[]f64,b:[]f64)->FdStatus {
    status:=fd_sample_count(points);if status!=FdStatus.ok {return status;}
    n:=points.count/3;
    if c.channels<0 || c.channels>FD_CHANNELS_MAX || c.outputs<1 || c.outputs>FD_CHANNELS_MAX || !(c.radius>0 && fd_finite(c.radius)) || width<1 {return FdStatus.parameter;}
    if width>FD_NEIGHBORS_MAX {return FdStatus.limit;}
    if features.count!=n*c.channels || counts.count!=n || nbr.count!=n*width || w.count!=c.outputs*(3+c.channels) || b.count!=c.outputs {return FdStatus.shape;}
    for x:points {if !fd_finite(x) {return FdStatus.number;}}
    for x:features {if !fd_finite(x) {return FdStatus.number;}}
    for x:w {if !fd_finite(x) {return FdStatus.number;}}
    for x:b {if !fd_finite(x) {return FdStatus.number;}}
    for i:0..<n {
        if counts[i]<0 || counts[i]>width {return FdStatus.shape;}
        for s:0..<counts[i] {j:=nbr[i*width+s];if j<0 || j>=n {return FdStatus.shape;}}
    }
    return FdStatus.ok;
}
// Pre-activation of output o for centre i and neighbour j.
fd_conv_h :: (c:FdConv,points:[]f64,features:[]f64,w:[]f64,b:[]f64,i:int,j:int,o:int)->f64 {
    row:=o*(3+c.channels);h:=b[o];
    for d:0..<3 {h+=w[row+d]*(points[3*j+d]-points[3*i+d])/c.radius;}
    for ch:0..<c.channels {h+=w[row+3+ch]*features[j*c.channels+ch];}
    return h;
}
// The neighbour slot whose activation is largest, the first on ties.
fd_conv_argmax :: (c:FdConv,points:[]f64,features:[]f64,nbr:[]int,counts:[]int,width:int,w:[]f64,b:[]f64,i:int,o:int)->int {
    best:=0;top:=0.0;
    for s:0..<counts[i] {h:=fd_conv_h(c,points,features,w,b,i,nbr[i*width+s],o);a:=ifx h>0 then h else 0.0;if s==0 || a>top {best=s;top=a;}}
    return best;
}
fd_conv_forward :: (c:FdConv,points:[]f64,features:[]f64,nbr:[]int,counts:[]int,width:int,w:[]f64,b:[]f64,out:[]f64)->FdResult {
    r:FdResult;r.status=fd_conv_check(c,points,features,nbr,counts,width,w,b);
    n:=points.count/3;
    if r.status==FdStatus.ok && out.count<n*c.outputs {r.status=FdStatus.capacity;}
    if r.status!=FdStatus.ok {return r;}
    for i:0..<n {for o:0..<c.outputs {
        v:=0.0;
        if counts[i]>0 && c.aggregate==FdAggregate.max {
            h:=fd_conv_h(c,points,features,w,b,i,nbr[i*width+fd_conv_argmax(c,points,features,nbr,counts,width,w,b,i,o)],o);
            if h>0 {v=h;}
        } else if counts[i]>0 {
            for s:0..<counts[i] {h:=fd_conv_h(c,points,features,w,b,i,nbr[i*width+s],o);if h>0 {v+=h;}}
            v/=f64(counts[i]);
        }
        out[i*c.outputs+o]=v;
    }}
    r.written=n*c.outputs;return r;
}
// Gradients of sum(dout * out) with respect to w, b and the features. Each
// active term (h > 0) passes its share g of dout: 1/count for the mean, 1
// for the max's chosen neighbour.
fd_conv_backward :: (c:FdConv,points:[]f64,features:[]f64,nbr:[]int,counts:[]int,width:int,w:[]f64,b:[]f64,
        dout:[]f64,dw:[]f64,db:[]f64,dfeatures:[]f64)->FdResult {
    r:FdResult;r.status=fd_conv_check(c,points,features,nbr,counts,width,w,b);n:=points.count/3;
    if r.status==FdStatus.ok && dout.count!=n*c.outputs {r.status=FdStatus.shape;}
    if r.status==FdStatus.ok {for x:dout {if !fd_finite(x) {r.status=FdStatus.number;}}}
    if r.status==FdStatus.ok && (dw.count<w.count || db.count<c.outputs || dfeatures.count<features.count) {r.status=FdStatus.capacity;}
    if r.status!=FdStatus.ok {return r;}
    for k:0..<w.count {dw[k]=0;}for k:0..<c.outputs {db[k]=0;}for k:0..<features.count {dfeatures[k]=0;}
    for i:0..<n {for o:0..<c.outputs {
        if counts[i]==0 {continue;}
        first:=0;last:=counts[i];g:=dout[i*c.outputs+o]/f64(counts[i]);
        if c.aggregate==FdAggregate.max {first=fd_conv_argmax(c,points,features,nbr,counts,width,w,b,i,o);last=first+1;g=dout[i*c.outputs+o];}
        row:=o*(3+c.channels);
        for s:first..<last {
            j:=nbr[i*width+s];
            if !(fd_conv_h(c,points,features,w,b,i,j,o)>0) {continue;}
            db[o]+=g;
            for d:0..<3 {dw[row+d]+=g*(points[3*j+d]-points[3*i+d])/c.radius;}
            for ch:0..<c.channels {dw[row+3+ch]+=g*features[j*c.channels+ch];dfeatures[j*c.channels+ch]+=g*w[row+3+ch];}
        }
    }}
    r.written=w.count+c.outputs+features.count;return r;
}
```

By hand: two points 1 apart on x, one feature (0.5 and −1), radius 2,
weights (1, 0, 0, 2) and bias −0.25, each point its own neighbour and the
other's.

- Point 0, from itself: 0 + 2 · 0.5 − 0.25 = 0.75. From point 1:
  (1 − 0)/2 + 2 · (−1) − 0.25 = −1.75, so relu gives 0. The mean is 0.375;
  the max is 0.75.
- Point 1, from itself: 0 − 2 − 0.25 = −2.25, so 0. From point 0:
  (0 − 1)/2 + 1 − 0.25 = 0.25. The mean is 0.125; the max is 0.25.
- With dout = (0, 1) and max aggregation, only point 1's neighbour 0 is
  active, with g = 1: the bias gradient is 1, the x weight's is
  1 · (−0.5) = −0.5, the feature weight's is 1 · 0.5 = 0.5, and point 0's
  feature gets 1 · 2 = 2.

<!-- source: psionic/examples/field.coax conv -->
```jai
    two:=[6]f64.[0,0,0, 1,0,0];feat:=[2]f64.[0.5,-1];nbr:=[4]int.[0,1, 1,0];counts:=[2]int.[2,2];
    w:=[4]f64.[1,0,0,2];bias:=[1]f64.[-0.25];out:[2]f64;
    c:=FdConv.{channels=1,outputs=1,radius=2};
    r:=fd_conv_forward(c,two[..],feat[..],nbr[..],counts[..],2,w[..],bias[..],out[..]);
    print("mean: % %, %\n",r.status,out[0],out[1]);
    c.aggregate=FdAggregate.max;r=fd_conv_forward(c,two[..],feat[..],nbr[..],counts[..],2,w[..],bias[..],out[..]);
    print("max: % %, %\n",r.status,out[0],out[1]);
    dout:=[2]f64.[0,1];dw:[4]f64;db:[1]f64;df:[2]f64;
    r=fd_conv_backward(c,two[..],feat[..],nbr[..],counts[..],2,w[..],bias[..],dout[..],dw[..],db[..],df[..]);
    print("max gradient for dout (0, 1): w (%, %, %, %), b %, features (%, %)\n",dw[0],dw[1],dw[2],dw[3],db[0],df[0],df[1]);
    
```

```text
mean: ok 0.375, 0.125
max: ok 0.75, 0.25
max gradient for dout (0, 1): w (-0.5, 0.0, 0.0, 0.5), b 1.0, features (2.0, 0.0)
```

## Refusals

Every operation checks its inputs before it writes anything, and returns a
typed status. The refusal program breaks one input at a time.

<!-- source: psionic/examples/field-refuse.coax refuse -->
```jai
main :: () {
    v:=fd_cube(-0.5,0.5);t:=FD_CUBE;
    print("23 coordinates: %\n",fd_mesh_admit(FdMesh.{vertices=v[0..<23],triangles=t[..]}).status);
    w:=v;w[5]=0.0/0.0;print("a NaN vertex: %\n",fd_mesh_admit(FdMesh.{vertices=w[..],triangles=t[..]}).status);
    u:=t;u[7]=8;print("vertex index 8 of 8: %\n",fd_mesh_admit(FdMesh.{vertices=v[..],triangles=u[..]}).status);
    u=t;u[2]=0;print("a triangle 0, 2, 0: %\n",fd_mesh_admit(FdMesh.{vertices=v[..],triangles=u[..]}).status);
    print("one face missing: %\n",fd_mesh_admit(FdMesh.{vertices=v[..],triangles=t[0..<30]}).status);
    u=t;u[1]=3;u[2]=2;print("one triangle turned over: %\n",fd_mesh_admit(FdMesh.{vertices=v[..],triangles=u[..]}).status);
    u=t;for k:0..<12 {s:=u[3*k+1];u[3*k+1]=u[3*k+2];u[3*k+2]=s;}
    print("every triangle turned over: %\n",fd_mesh_admit(FdMesh.{vertices=v[..],triangles=u[..]}).status);
    m:=fd_mesh_admit(FdMesh.{vertices=v[..],triangles=t[..]});
    print("distance at (0, inf, 0): %\n",fd_mesh_distance(m,[3]f64.[0,1.0/0.0,0]).status);
    print("distance at x = 2,000,000: %\n",fd_mesh_distance(m,[3]f64.[2000000,0,0]).status);
    none:FdChecked;print("distance on a mesh never admitted: %\n",fd_mesh_distance(none,[3]f64.[0,0,0]).status);
    print("a sphere of radius 0: %\n",fd_shape_distance(FdShape.{kind=FdKind.sphere,r=0},[3]f64.[1,0,0]).status);
    print("a box with low z = high z: %\n",fd_shape_distance(FdShape.{kind=FdKind.box,b=[3]f64.[1,1,0]},[3]f64.[1,0,0]).status);

    points:=[6]f64.[-7,-7,-7,-7,-7,-7];normals:[6]f64;
    print("sampling box low x = high x: %\n",fd_sample_box([3]f64.[1,0,0],[3]f64.[1,1,1],1,points[..]).status);
    print("5 numbers for sample points: %\n",fd_sample_surface(m,1,points[0..<5],normals[0..<5]).status);
    many:[12291]f64;print("4,097 sample points: %\n",fd_sample_box([3]f64.[0,0,0],[3]f64.[1,1,1],1,many[..]).status);
    left:=0;for x:points {if x==-7 {left+=1;}}print("sample values still -7: % of 6\n",left);

    p:=[9]f64.[0,0,0, 1,1,1, 0.5,0.5,0.5];start:[40000]int;order:[3]int;
    print("cell size 0: %\n",fd_grid_build(p[..],0,start[..],order[..]).status);
    print("33 cells a side: %\n",fd_grid_build(p[..],1.0/32,start[..],order[..]).status);
    print("27 cells, 27 starts: %\n",fd_grid_build(p[..],0.5,start[0..<27],order[..]).status);
    g:=fd_grid_build(p[..],1,start[..],order[..]);out:=[4]int.[-7,-7,-7,-7];
    print("radius 0: %\n",fd_radius(g,[3]f64.[0,0,0],0,out[..]).status);
    print("3 neighbours, room for 2: %\n",fd_radius(g,[3]f64.[0,0,0],2,out[0..<2]).status);
    print("k = 4 of 3 points: %\n",fd_knn(g,[3]f64.[0,0,0],4,out[..]).status);
    print("k = 65: %\n",fd_knn(g,[3]f64.[0,0,0],65,out[..]).status);
    cloud:[12288]f64;fd_cloud(3,4096,cloud[..]);big:[32769]int;all:[4096]int;
    dense:=fd_grid_build(cloud[..],0.125,big[..],all[..]);
    print("more than 64 neighbours within 0.5: %\n",fd_radius(dense,[3]f64.[0,0,0],0.5,out[..]).status);
    print("neighbour slots still -7: % % % %\n",out[0],out[1],out[2],out[3]);

    two:=[6]f64.[0,0,0, 1,0,0];feat:=[2]f64.[0.5,-1];nbr:=[4]int.[0,1, 1,0];counts:=[2]int.[2,2];
    wt:=[4]f64.[1,0,0,2];bias:=[1]f64.[-0.25];y:=[2]f64.[-7,-7];c:=FdConv.{channels=1,outputs=1,radius=2};
    print("3 weights for 4: %\n",fd_conv_forward(c,two[..],feat[..],nbr[..],counts[..],2,wt[0..<3],bias[..],y[..]).status);
    bad:=nbr;bad[1]=2;print("neighbour index 2 of 2 points: %\n",fd_conv_forward(c,two[..],feat[..],bad[..],counts[..],2,wt[..],bias[..],y[..]).status);
    k:=counts;k[0]=3;print("3 neighbours in 2 slots: %\n",fd_conv_forward(c,two[..],feat[..],nbr[..],k[..],2,wt[..],bias[..],y[..]).status);
    c.radius=0.0/0.0;print("a NaN radius: %\n",fd_conv_forward(c,two[..],feat[..],nbr[..],counts[..],2,wt[..],bias[..],y[..]).status);
    c.radius=2;f:=feat;f[1]=1.0/0.0;print("an infinite feature: %\n",fd_conv_forward(c,two[..],f[..],nbr[..],counts[..],2,wt[..],bias[..],y[..]).status);
    print("outputs still -7: % %\n",y[0],y[1]);
}
```

```text
23 coordinates: shape
a NaN vertex: number
vertex index 8 of 8: shape
a triangle 0, 2, 0: degenerate
one face missing: open
one triangle turned over: open
every triangle turned over: inside_out
distance at (0, inf, 0): number
distance at x = 2,000,000: number
distance on a mesh never admitted: shape
a sphere of radius 0: parameter
a box with low z = high z: parameter
sampling box low x = high x: parameter
5 numbers for sample points: shape
4,097 sample points: limit
sample values still -7: 6 of 6
cell size 0: parameter
33 cells a side: limit
27 cells, 27 starts: capacity
radius 0: parameter
3 neighbours, room for 2: capacity
k = 4 of 3 points: parameter
k = 65: limit
more than 64 neighbours within 0.5: limit
neighbour slots still -7: -7 -7 -7 -7
3 weights for 4: shape
neighbour index 2 of 2 points: shape
3 neighbours in 2 slots: shape
a NaN radius: parameter
an infinite feature: number
outputs still -7: -7.0 -7.0
```

## How it is checked

The checks in [`field.coax`](../lib/field.coax) come before the code they
test. Each compares the library with something that does not share its code:

- **Hand values.** The shapes, the cube mesh, the octahedron, the five-point
  grid, the neighbour table and the two-point convolution, as worked above.
- **Closed forms.** A box built as a 12-triangle mesh, stretched so its sides
  differ, has the same field as the closed-form box: on 256 random points the
  distances agree within 10⁻¹⁵ and the gradients within 10⁻¹². The
  octahedron's sign agrees with |x| + |y| + |z| < 1.
- **Brute force.** The nearest point of a triangle is never farther than any
  point of a 30-step barycentric grid over it, and at most a grid step nearer.
  No sampled surface point of the octahedron is nearer to a query than the
  mesh distance. Grid radius and k-nearest queries equal brute-force search
  index for index on 6 clouds, including flattened clouds and clouds with
  repeated points, so ties are exercised.
- **Finite differences.** The gradients of the three shapes and the
  octahedron match central differences within 10⁻⁶; the convolution's weight,
  bias and feature gradients match within 10⁻⁷, for both aggregations.
- **A direct loop.** The convolution equals a loop over every pair of points
  within the radius, written without the grid or the table.
- **A second series.** The arctangent matches Euler's series,
  atan z = Σ (2n)!!/(2n+1)!! · z²ⁿ⁺¹/(1 + z²)ⁿ⁺¹, within 7 × 10⁻¹⁶, and
  atan z + atan(1/z) = π/2.
- **Sampling statistics.** Surface samples split across the cube's faces
  within three standard deviations of equal shares, and on a slab 100 times
  thinner than it is wide, 1,001 of 1,024 samples land on the two big faces,
  where area predicts 1,004.

`coax mutate psionic/lib/field.coax --root-only` changes the library's own
code one site at a time and reruns the checks. Of 1,174 mutants, 1,122 are
caught. The 52 that survive fall into three groups:

- **Comparisons at a boundary where both branches agree.** Clamping t to 0
  with `t < 0` or `t <= 0`; Ericson's region tests, where a point on the line
  between two regions has the same nearest point from either; relu's `h > 0`,
  which adds 0 either way; a directed-edge test read in the other direction,
  which is the same test on a consistent mesh.
- **Fallbacks that are not reached.** The last triangle when the area draw
  runs past the total, a winding number of exactly 0.5, an argmax start that
  the first neighbour always overwrites.
- **The grid's rounding margins.** The extra cell around a radius query, the
  clamping of a cell index and the ring search's stopping bound only matter
  when a coordinate rounds across a cell boundary, and the checks do not
  construct such a point.

The library is not an `#authority` file: it decides no money, admission or
policy, so these survivors are reported rather than gated.

## What this does not do

- The gradient of the convolution is taken with respect to its weights, bias
  and features, not the point positions.
- Neighbourhoods use a uniform grid only. A grid is a poor fit for a cloud
  that is dense in one corner and empty elsewhere; a k-d tree or a bounding
  volume hierarchy is not built.
- A mesh distance visits every triangle, and the sign visits every triangle
  again. That is fine for 1,024 triangles and slow for a car body; chapter 160
  needs an acceleration structure before it runs DoMINO on a real mesh.
- The layer is written by hand in binary64. It is not yet an operator in
  [chapter 5](ch005-a-standard-library-of-tensor-operators.md)'s registry, and
  its gradients are not derived by
  [chapter 106](ch106-differentiate-any-tensor-program.md)'s automatic
  differentiation; they are checked against finite differences instead.

## Dependencies

- [Chapter 5](ch005-a-standard-library-of-tensor-operators.md): gather,
  reductions and indexed updates, the operations this chapter writes out by
  hand.
- [Chapter 106](ch106-differentiate-any-tensor-program.md): gradients through
  the aggregation layers.
- [Chapter 53](ch053-choosing-the-next-token.md): the counter-based
  generator the samplers draw from.
- The Aethyr engine's spatial queries
  ([Aethyr chapter 7](../../aethyr/book/ch007-ask-the-world-where-things-are.md))
  as a related design.

## Sources

- [Chapter plan](PLAN.md), Part XVI.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), DoMINO entry.
- Christer Ericson, *Real-Time Collision Detection*, section 5.1.5, the
  nearest point on a triangle.
- A. van Oosterom and J. Strackee, "The solid angle of a plane triangle",
  IEEE Transactions on Biomedical Engineering, 1983.
- Alec Jacobson, Ladislav Kavan and Olga Sorkine-Hornung, "Robust
  inside-outside segmentation using generalized winding numbers", SIGGRAPH
  2013.
