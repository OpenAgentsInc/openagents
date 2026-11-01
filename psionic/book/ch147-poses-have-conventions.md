# 147. Poses have conventions

A pose is a set of joint rotations on a skeleton plus a root position. Two
motion files can describe the same movement with different rotation forms,
axes, units, root modes and frame rates. If a program guesses any of these,
the character moves wrongly and nothing reports an error. This chapter makes
every convention declared data, converts between conventions, and refuses a
clip that leaves one out.

Everything runs on one CPU core in binary64. The library is
[pose.coax](../lib/pose.coax). It reuses chapter 167's Hamilton quaternion
product `pfk_multiply`, its quaternion-to-matrix map `pfk_matrix` and its
finite-bits test `pfk_finite`, from
[panda-fk-math.coax](../lib/robotics/panda-fk-math.coax). A skeleton holds at
most 24 joints and a clip at most 64 frames.

## Run it

```shell
coax/bin/coax run psionic/examples/pose-conventions.coax
coax/bin/coax test psionic/lib/pose.coax
coax/bin/coax mutate psionic/lib/pose.coax --root-only
```

## What a convention declares

A `PoseConvention` has seven fields. Each enum starts with `undeclared`, so a
zero-filled convention is refused rather than read as a default.

| Field | Values | What it decides |
| --- | --- | --- |
| `up` | `pos_x` … `neg_z` | The coordinate axis that points up |
| `forward` | `pos_x` … `neg_z` | The axis a character faces at rest |
| `hand` | `right`, `left` | Whether the coordinates are right- or left-handed |
| `order` | `wxyz`, `xyzw` | Where a stored quaternion keeps its scalar part |
| `unit` | `metre`, `centimetre` | The unit of root positions and bone offsets |
| `root` | `absolute`, `delta` | Whether each frame stores the root position or its change since the previous frame |
| `rate` | 1 to 1,000 | Frames per second |

Up and forward must be different axes. Right is not stored: it follows from
up, forward and the handedness, as the next section shows.

Inside the library a quaternion is always `w, x, y, z` with the Hamilton
product, and a matrix is row-major and maps a joint's local coordinates to its
parent's. A skeleton lists joints parents first: `parent[0]` is -1 and every
other `parent[j]` lies in `0 .. j-1`. `offset[j]` is joint j's rest position in
its parent's frame. `offset[0]` is the root joint's rest position relative to
the clip's root translation, zero when the translation is the root joint
itself.

## Four ways to write one rotation

Take the knee's quarter turn about +X, 90 degrees, by hand.

- **Rotation vector.** Axis times angle: (π/2, 0, 0) ≈ (1.570796, 0, 0).
- **Quaternion.** (cos 45°, sin 45° · axis) = (0.707107, 0.707107, 0, 0).
  The quaternion −q is the same rotation.
- **Matrix.** From `pfk_matrix` with w = x = √½ and y = z = 0: row 0 is
  (1 − 2(y² + z²), 2(xy − wz), 2(xz + wy)) = (1, 0, 0); row 1 is
  (2(xy + wz), 1 − 2(x² + z²), 2(yz − wx)) = (0, 0, −1); row 2 is
  (2(xz − wy), 2(yz + wx), 1 − 2(x² + y²)) = (0, 1, 0). The +Y axis goes to
  +Z, as a positive turn about X should do.
- **6D.** The first two matrix columns, (1, 0, 0) and (0, 0, 1). A network
  can output any six numbers; Gram–Schmidt turns them back into a rotation.

A quaternion is admitted when its four parts are finite, each within ±2, and
their squares sum to 1 within 2⁻³⁰ (about 9.3e-10). The tolerance is a power
of two so that a check can sit exactly on the boundary: 1 + (2⁻¹⁵)² is exactly
1 + 2⁻³⁰. Because q and −q are one rotation, the canonical choice has w > 0;
when w is 0 the first nonzero part decides.

<!-- source: psionic/lib/pose.coax admit -->
```jai
// A unit quaternion has four finite parts whose squares sum to 1.
pose_quat_check :: (q:[4]f64)->PoseStatus {
    sum:f64=0;for x:q {status:=pose_value(x,2);if status!=PoseStatus.ok {return status;}sum+=x*x;}
    if abs(sum-1)>POSE_TOLERANCE {return PoseStatus.not_unit;}
    return PoseStatus.ok;
}
// q and -q are one rotation; the canonical one has w > 0, or, when w is 0,
// a positive first nonzero part.
pose_canonical :: (q:[4]f64)->[4]f64 {
    lead:=q[0];if lead==0 {lead=q[1];if lead==0 {lead=q[2];if lead==0 {lead=q[3];}}}
    if lead<0 {return [4]f64.[-q[0],-q[1],-q[2],-q[3]];}
    return q;
}
```

### Matrix to quaternion

A matrix is admitted when MᵀM is the identity and det M is +1, each within
the tolerance. A mirror has orthonormal columns too, but its determinant is
−1. To recover the quaternion, note that 4w² = 1 + trace and
4x² = 1 + 2m₀₀ − trace, and similarly for y and z. Shepperd's method takes
the square root of the largest of the four, so it never divides by a small
number, even at a half turn where w is 0. For the quarter turn above, the
trace is 1 and the diagonal is (1, 0, 0): the trace ties m₀₀ and the w branch
wins ties, so s = 2√2, w = s/4 = √½ and x = (m₂₁ − m₁₂)/s = 2/(2√2) = √½.

<!-- source: psionic/lib/pose.coax matrix -->
```jai
// M^T M must be the identity within the tolerance, and det M must be +1
// within it too: a mirror has orthonormal columns but determinant -1.
pose_matrix_check :: (m:[9]f64)->PoseStatus {
    for x:m {status:=pose_value(x,2);if status!=PoseStatus.ok {return status;}}
    for i:0..<3 {for j:0..<3 {
        dot:=m[i]*m[j]+m[3+i]*m[3+j]+m[6+i]*m[6+j];
        if abs(dot-(ifx i==j then 1.0 else 0.0))>POSE_TOLERANCE {return PoseStatus.not_rotation;}
    }}
    if abs(pose_det3(m)-1)>POSE_TOLERANCE {return PoseStatus.not_rotation;}
    return PoseStatus.ok;
}
pose_det3 :: (m:[9]f64)->f64 {return m[0]*(m[4]*m[8]-m[5]*m[7])-m[1]*(m[3]*m[8]-m[5]*m[6])+m[2]*(m[3]*m[7]-m[4]*m[6]);}
// Shepperd's method: 4w^2 = 1 + trace and 4x^2 = 1 + 2 m00 - trace (and so
// on); take the square root of the largest of the four, so the divisor is
// never small, even at a half turn where w is 0.
pose_matrix_quat :: (m:[9]f64)->[4]f64 {
    t:=m[0]+m[4]+m[8];q:[4]f64;
    if t>=m[0] && t>=m[4] && t>=m[8] {
        s:=2*sqrt(1+t);q=[4]f64.[s/4,(m[7]-m[5])/s,(m[2]-m[6])/s,(m[3]-m[1])/s];
    } else if m[0]>=m[4] && m[0]>=m[8] {
        s:=2*sqrt(1+2*m[0]-t);q=[4]f64.[(m[7]-m[5])/s,s/4,(m[1]+m[3])/s,(m[2]+m[6])/s];
    } else if m[4]>=m[8] {
        s:=2*sqrt(1+2*m[4]-t);q=[4]f64.[(m[2]-m[6])/s,(m[1]+m[3])/s,s/4,(m[5]+m[7])/s];
    } else {
        s:=2*sqrt(1+2*m[8]-t);q=[4]f64.[(m[3]-m[1])/s,(m[2]+m[6])/s,(m[5]+m[7])/s,s/4];
    }
    return pose_canonical(pose_normalize(q));
}
```

### Rotation vector and back

Going to a quaternion needs only sine and cosine. Coming back needs an
arctangent, and Coax has none built in, so the library owns one. With w ≥ 0
and s = |(x, y, z)|, the half-angle identity gives θ/4 = atan(s / (1 + w)),
and s / (1 + w) always lies in [0, 1]. Above tan(π/8) the argument is reduced
with atan z = π/4 + atan((z − 1)/(z + 1)); then 20 terms of the odd series
suffice. Twenty is the fewest that give the same double as 30 terms at
tan(π/8), and a check holds the series to exactly that. For the quarter turn,
z = √½ / (1 + √½) = √2 − 1, which is tan(π/8) itself. In binary64 it rounds
to just below the constant, so the series runs directly and gives π/8, and
the vector is x · 4 · (π/8) / s = √½ · (π/2) / √½ = π/2 along X.

<!-- source: psionic/lib/pose.coax vector -->
```jai
// A rotation vector v has angle |v| and axis v/|v|. Going to a quaternion
// needs only sin and cos; sin(theta/2)/theta stays accurate for tiny theta.
pose_vector_quat :: (v:[3]f64)->[4]f64 {
    theta:=pose_norm3(v);if theta==0 {return [4]f64.[1,0,0,0];}
    k:=sin(theta/2)/theta;return [4]f64.[cos(theta/2),v[0]*k,v[1]*k,v[2]*k];
}
// Coming back needs an arctangent: with w >= 0 and s = |(x,y,z)|,
// theta/4 = atan(s/(1+w)), and s/(1+w) lies in [0, 1].
pose_quat_vector :: (q:[4]f64)->[3]f64 {
    c:=pose_canonical(q);s:=sqrt(c[1]*c[1]+c[2]*c[2]+c[3]*c[3]);if s==0 {return [3]f64.[0,0,0];}
    k:=4*pose_atan(s/(1+c[0]))/s;return [3]f64.[c[1]*k,c[2]*k,c[3]*k];
}
// atan on [0, 1]: above tan(pi/8), atan z = pi/4 + atan((z-1)/(z+1)), which
// brings the argument within tan(pi/8); then 20 terms of the odd series,
// the fewest that give the same double as 30 terms at tan(pi/8).
pose_atan :: (z:f64)->f64 {
    if z>POSE_TAN_PI_8 {return POSE_PI/4+pose_atan_series((z-1)/(z+1));}
    return pose_atan_series(z);
}
pose_atan_series :: (z:f64)->f64 {
    sum:f64=0;for back:0..<20 {k:=19-back;sign:=ifx k%2==0 then 1.0 else -1.0;sum=sign/f64(2*k+1)+z*z*sum;}
    return z*sum;
}
```

### 6D and back

<!-- source: psionic/lib/pose.coax six -->
```jai
// The 6D form keeps the first two columns of the matrix. Any six numbers map
// back to a rotation by Gram-Schmidt, unless the columns are (nearly) zero or
// parallel; then there is no rotation to recover and the input is refused.
pose_matrix_six :: (m:[9]f64)->[6]f64 {return [6]f64.[m[0],m[3],m[6],m[1],m[4],m[7]];}
pose_six_matrix :: (six:[6]f64)->PoseMatrix {
    for x:six {status:=pose_value(x,POSE_LENGTH_MAX);if status!=PoseStatus.ok {return PoseMatrix.{status=status};}}
    a1:=[3]f64.[six[0],six[1],six[2]];a2:=[3]f64.[six[3],six[4],six[5]];
    n1:=pose_norm3(a1);if n1<=POSE_TOLERANCE {return PoseMatrix.{status=PoseStatus.degenerate};}
    b1:=[3]f64.[a1[0]/n1,a1[1]/n1,a1[2]/n1];d:=pose_dot3(b1,a2);
    u:=[3]f64.[a2[0]-d*b1[0],a2[1]-d*b1[1],a2[2]-d*b1[2]];
    n2:=pose_norm3(u);if n2<=POSE_TOLERANCE {return PoseMatrix.{status=PoseStatus.degenerate};}
    b2:=[3]f64.[u[0]/n2,u[1]/n2,u[2]/n2];b3:=pose_cross3(b1,b2);
    return PoseMatrix.{status=PoseStatus.ok,value=[9]f64.[b1[0],b2[0],b3[0],b1[1],b2[1],b3[1],b1[2],b2[2],b3[2]]};
}
```

Two columns that are zero or parallel hold no rotation. They are refused as
`degenerate`, not repaired.

## Axes, units and handedness

A convention's basis matrix E has rows right, up and forward, each written in
that convention's coordinates. Facing forward with up overhead, right-handed
coordinates have right = forward × up, and left-handed coordinates have
right = up × forward. A point's right, up and forward parts are E p, so in
another convention it is E_toᵀ E_from p. Call that C.

Here is the change from glTF's convention (Y up, +Z forward, right-handed) to
a Z-up one (Z up, −Y forward, right-handed), by hand:

- glTF: up (0, 1, 0), forward (0, 0, 1), right = (0, 0, 1) × (0, 1, 0) =
  (−1, 0, 0).
- Z-up: up (0, 0, 1), forward (0, −1, 0), right = (0, −1, 0) × (0, 0, 1) =
  (−1, 0, 0).
- A point (x, y, z) has right part −x, up part y and forward part z. In Z-up
  coordinates that is −x · (−1, 0, 0) + y · (0, 0, 1) + z · (0, −1, 0) =
  (x, −z, y).

A rotation R becomes C R Cᵀ. When the handedness changes, C is a mirror
(det −1), but C R Cᵀ is still a rotation, so the library goes through the
matrix rather than multiplying quaternions. From glTF to Unity's convention
(Y up, +Z forward, left-handed), C = diag(−1, 1, 1): x flips, and a turn about
Y reverses direction. Units scale positions only: metres to centimetres is
×100.

<!-- source: psionic/lib/pose.coax convention -->
```jai
pose_convention_check :: (c:PoseConvention)->PoseStatus {
    if c.up==PoseAxis.undeclared || c.forward==PoseAxis.undeclared || c.hand==PoseHand.undeclared {return PoseStatus.convention;}
    if c.order==PoseOrder.undeclared || c.unit==PoseUnit.undeclared || c.root==PoseRoot.undeclared {return PoseStatus.convention;}
    // pos_x, neg_x are 1, 2; pos_y, neg_y 3, 4; pos_z, neg_z 5, 6.
    if (cast(int)c.up-1)/2==(cast(int)c.forward-1)/2 {return PoseStatus.convention;}
    if c.rate<1 || c.rate>POSE_RATE_MAX {return PoseStatus.rate;}
    return PoseStatus.ok;
}
pose_axis_vector :: (a:PoseAxis)->[3]f64 {
    v:[3]f64;k:=cast(int)a-1;v[k/2]=ifx k%2==0 then 1.0 else -1.0;return v;
}
// Rows right, up, forward, in the convention's coordinates. Right-handed
// coordinates have right = forward x up; left-handed, right = up x forward.
pose_basis :: (c:PoseConvention)->[9]f64 {
    u:=pose_axis_vector(c.up);f:=pose_axis_vector(c.forward);
    r:=ifx c.hand==PoseHand.right then pose_cross3(f,u) else pose_cross3(u,f);
    return [9]f64.[r[0],r[1],r[2],u[0],u[1],u[2],f[0],f[1],f[2]];
}
// A point's right/up/forward parts are E_from p; in the new coordinates it is
// E_to^T E_from p. C is orthogonal; det C is -1 when the handedness changes.
pose_change :: (from:PoseConvention,to:PoseConvention)->[9]f64 {
    a:=pose_basis(from);b:=pose_basis(to);c:[9]f64;
    for i:0..<3 {for j:0..<3 {c[i*3+j]=b[i]*a[j]+b[3+i]*a[3+j]+b[6+i]*a[6+j];}}
    return c;
}
pose_apply :: (m:[9]f64,v:[3]f64)->[3]f64 {
    return [3]f64.[m[0]*v[0]+m[1]*v[1]+m[2]*v[2],m[3]*v[0]+m[4]*v[1]+m[5]*v[2],m[6]*v[0]+m[7]*v[1]+m[8]*v[2]];
}
// A rotation R in the old coordinates is C R C^T in the new ones. It is a
// rotation even when C is a mirror, so it goes back to a quaternion.
pose_change_rotation :: (c:[9]f64,q:[4]f64)->[4]f64 {
    r:=pfk_matrix(q);cr:[9]f64;out:[9]f64;
    for i:0..<3 {for j:0..<3 {cr[i*3+j]=c[i*3]*r[j]+c[i*3+1]*r[3+j]+c[i*3+2]*r[6+j];}}
    for i:0..<3 {for j:0..<3 {out[i*3+j]=cr[i*3]*c[j*3]+cr[i*3+1]*c[j*3+1]+cr[i*3+2]*c[j*3+2];}}
    return pose_matrix_quat(out);
}
pose_unit_factor :: (from:PoseUnit,to:PoseUnit)->f64 {
    a:=ifx from==PoseUnit.metre then 100.0 else 1.0;b:=ifx to==PoseUnit.metre then 100.0 else 1.0;return a/b;
}
pose_read :: (order:PoseOrder,q:[4]f64)->[4]f64 {if order==PoseOrder.xyzw {return [4]f64.[q[3],q[0],q[1],q[2]];}return q;}
pose_write :: (order:PoseOrder,q:[4]f64)->[4]f64 {if order==PoseOrder.xyzw {return [4]f64.[q[1],q[2],q[3],q[0]];}return q;}
```

## Skeletons and forward kinematics

<!-- source: psionic/lib/pose.coax skeleton -->
```jai
pose_skeleton_check :: (s:PoseSkeleton)->PoseStatus {
    if s.count<1 || s.count>POSE_JOINTS {return PoseStatus.shape;}
    if s.parent[0]!= -1 {return PoseStatus.parent;}
    for j:1..<s.count {if s.parent[j]<0 || s.parent[j]>=j {return PoseStatus.parent;}}
    for j:0..<s.count {for x:s.offset[j] {status:=pose_value(x,POSE_LENGTH_MAX);if status!=PoseStatus.ok {return status;}}}
    return PoseStatus.ok;
}
pose_clip_check :: (c:PoseClip,s:PoseSkeleton)->PoseStatus {
    status:=pose_skeleton_check(s);if status!=PoseStatus.ok {return status;}
    status=pose_convention_check(c.convention);if status!=PoseStatus.ok {return status;}
    if c.frames<1 || c.frames>POSE_FRAMES {return PoseStatus.shape;}
    for k:0..<c.frames {
        for x:c.frame[k].root {status=pose_value(x,POSE_LENGTH_MAX);if status!=PoseStatus.ok {return status;}}
        for j:0..<s.count {status=pose_quat_check(c.frame[k].rotation[j]);if status!=PoseStatus.ok {return status;}}
    }
    return PoseStatus.ok;
}
```

Forward kinematics walks the joints parents first. Each joint's world
rotation is its parent's world rotation times its local rotation, and its
position is its parent's position plus the parent's world rotation applied to
its offset.

<!-- source: psionic/lib/pose.coax forward -->
```jai
// World rotation and position of every joint, parents first:
//   R_j = R_parent * q_j,  p_j = p_parent + R_parent offset_j.
// The root joint sits at the frame's root translation plus offset_0, its
// rest position (zero when the translation is the root joint itself).
pose_forward :: (s:PoseSkeleton,c:PoseClip,frame:int)->PoseWorld {
    out:PoseWorld;out.status=pose_clip_check(c,s);if out.status!=PoseStatus.ok {return out;}
    if frame<0 || frame>=c.frames {out.status=PoseStatus.shape;return out;}
    f:=pose_internal(c,s.count).frame[frame];
    out.rotation[0]=f.rotation[0];for i:0..<3 {out.position[0][i]=f.root[i]+s.offset[0][i];}
    for j:1..<s.count {
        p:=s.parent[j];out.rotation[j]=pfk_multiply(out.rotation[p],f.rotation[j]);
        d:=pose_rotate(out.rotation[p],s.offset[j]);
        for i:0..<3 {out.position[j][i]=out.position[p][i]+d[i];}
    }
    return out;
}
```

Trace the example's leg by hand. The hip is the root at (0, 0.9, 0.1k) metres
in frame k, the knee hangs 0.5 m below it and the foot 0.4 m below the knee.
The hip turns −45k degrees about X and the knee holds +90 degrees. A turn by
θ about X sends (0, y, z) to (0, y cos θ − z sin θ, y sin θ + z cos θ).

- Frame 0: the hip does not turn, so the knee is at (0, 0.4, 0). The knee's
  +90° sends the shin (0, −0.4, 0) to (0, 0, −0.4), so the foot is at
  (0, 0.4, −0.4).
- Frame 2: the hip's −90° sends the thigh (0, −0.5, 0) to (0, 0, 0.5), so the
  knee is at (0, 0.9, 0.2 + 0.5) = (0, 0.9, 0.7). The knee's world rotation
  is −90° + 90° = 0, so the foot is 0.4 m straight below: (0, 0.5, 0.7).

## Root modes and frame rates

A delta clip stores frame 0's root and then each frame's change. Integrating
is a running sum; the reverse is a difference, so a round trip is exact for
values whose sums are exact.

Resampling puts output frame k at source time k · from / to frames. Writing
that as a whole part i and a remainder r keeps it exact: the root is linear
between source frames i and i + 1 at t = r / to, and each rotation is
spherical: from a, turn t of the way along the short arc to b.

<!-- source: psionic/lib/pose.coax slerp -->
```jai
// Spherical interpolation as a product: from a, turn t of the way along the
// short arc to b. The canonical relative rotation is the short arc.
pose_slerp :: (a:[4]f64,b:[4]f64,t:f64)->[4]f64 {
    conjugate:=[4]f64.[a[0],-a[1],-a[2],-a[3]];v:=pose_quat_vector(pfk_multiply(conjugate,b));
    return pfk_multiply(a,pose_vector_quat([3]f64.[v[0]*t,v[1]*t,v[2]*t]));
}
```

From 30 to 60 frames per second, a 3-frame clip becomes (3 − 1) · 60/30 + 1 =
5 frames, and the odd ones fall halfway. Halfway between the hip's 0° and
−45° is −22.5°, a quaternion (cos 11.25°, −sin 11.25°, 0, 0) =
(0.980785, −0.195090, 0, 0).

<!-- source: psionic/lib/pose.coax convert -->
```jai
// Convert a clip to another convention: axes and units on every root and
// rotation, then the frame rate, then the order and root mode.
pose_convert :: (c:PoseClip,s:PoseSkeleton,to:PoseConvention)->PoseClipResult {
    out:PoseClipResult;out.status=pose_clip_check(c,s);if out.status!=PoseStatus.ok {return out;}
    out.status=pose_convention_check(to);if out.status!=PoseStatus.ok {return out;}
    from:=c.convention;
    // Output frame k sits at source time k * from.rate / to.rate frames.
    frames:=(c.frames-1)*to.rate/from.rate+1;
    if frames>POSE_FRAMES {out.status=PoseStatus.shape;return out;}
    work:=pose_internal(c,s.count);m:=pose_change(from,to);scale:=pose_unit_factor(from.unit,to.unit);
    for k:0..<c.frames {
        p:=pose_apply(m,work.frame[k].root);for i:0..<3 {work.frame[k].root[i]=p[i]*scale;}
        for j:0..<s.count {work.frame[k].rotation[j]=pose_change_rotation(m,work.frame[k].rotation[j]);}
    }
    timed:PoseClip;timed.frames=frames;
    for k:0..<frames {
        i:=k*from.rate/to.rate;rest:=k*from.rate%to.rate;
        if rest==0 {timed.frame[k]=work.frame[i];continue;}
        t:=f64(rest)/f64(to.rate);a:=work.frame[i];b:=work.frame[i+1];
        for x:0..<3 {timed.frame[k].root[x]=a.root[x]+(b.root[x]-a.root[x])*t;}
        for j:0..<s.count {timed.frame[k].rotation[j]=pose_slerp(a.rotation[j],b.rotation[j],t);}
    }
    out.clip=pose_external(timed,s.count,to);
    return out;
}
```

## Retargeting

Two skeletons with the same joints and parents but different bone lengths
can share local rotations. The root must move in proportion, or feet that
touched the ground float or sink. The library scales the root by the ratio
of the two chains' lengths from a chosen joint (a foot, say) up to the root.
Halving every bone of the leg halves the chain, 0.45 / 0.9 = 0.5, and every
joint lands at half its distance from the origin.

<!-- source: psionic/lib/pose.coax retarget -->
```jai
// Sum of bone lengths from a joint up to the root.
pose_chain_length :: (s:PoseSkeleton,joint:int)->f64 {
    sum:f64=0;j:=joint;while j>0 {sum+=pose_norm3(s.offset[j]);j=s.parent[j];}return sum;
}
// Same joints, same parents, different bone lengths: keep the local
// rotations and scale the root by the ratio of the two chain lengths (a leg,
// so that feet that touched the ground still do).
pose_retarget :: (c:PoseClip,from:PoseSkeleton,to:PoseSkeleton,joint:int)->PoseClipResult {
    out:PoseClipResult;out.status=pose_clip_check(c,from);if out.status!=PoseStatus.ok {return out;}
    out.status=pose_skeleton_check(to);if out.status!=PoseStatus.ok {return out;}
    if to.count!=from.count {out.status=PoseStatus.shape;return out;}
    for back:0..<to.count {j:=to.count-1-back;if to.parent[j]!=from.parent[j] {out.status=PoseStatus.shape;return out;}}
    if joint<1 || joint>=to.count {out.status=PoseStatus.joint;return out;}
    a:=pose_chain_length(from,joint);b:=pose_chain_length(to,joint);
    if a<=POSE_TOLERANCE || b<=POSE_TOLERANCE {out.status=PoseStatus.degenerate;return out;}
    out.clip=c;scale:=b/a;
    for k:0..<c.frames {for i:0..<3 {out.clip.frame[k].root[i]=c.frame[k].root[i]*scale;}}
    return out;
}
```

## Run the example

The [example](../examples/pose-conventions.coax) writes every rotation form of the knee's quarter turn, runs the
leg's forward kinematics, converts the clip to the Z-up convention, retargets
it onto a leg half as long, and shows five refusals. Values are printed in
millionths or millimetres.

<!-- source: psionic/examples/pose-conventions.coax forms -->
```jai
    // The knee's rest-to-bent rotation: a quarter turn about +X.
    q:=pose_vector_quat([3]f64.[POSE_PI/2,0,0]);m:=pose_quat_matrix(q);six:=pose_matrix_six(m);v:=pose_quat_vector(q);
    print("quaternion wxyz (millionths): % % % %\n",pose_micro(q[0]),pose_micro(q[1]),pose_micro(q[2]),pose_micro(q[3]));
    print("matrix rows: % % % | % % % | % % %\n",pose_micro(m[0]),pose_micro(m[1]),pose_micro(m[2]),pose_micro(m[3]),pose_micro(m[4]),pose_micro(m[5]),pose_micro(m[6]),pose_micro(m[7]),pose_micro(m[8]));
    print("6D columns: % % % | % % %\n",pose_micro(six[0]),pose_micro(six[1]),pose_micro(six[2]),pose_micro(six[3]),pose_micro(six[4]),pose_micro(six[5]));
    print("rotation vector: % % %\n",pose_micro(v[0]),pose_micro(v[1]),pose_micro(v[2]));
    back:=pose_six_matrix(six);r:=pose_matrix_quat(back.value);
    print("6D -> matrix -> quaternion: % % % %\n",pose_micro(r[0]),pose_micro(r[1]),pose_micro(r[2]),pose_micro(r[3]));
```

<!-- source: psionic/examples/pose-conventions.coax leg -->
```jai
    // A leg in Y-up, +Z forward, right-handed metres, w-first quaternions,
    // absolute roots, 30 frames per second.
    leg:PoseSkeleton;leg.count=3;leg.parent[0]= -1;leg.parent[1]=0;leg.parent[2]=1;
    leg.offset[1]=[3]f64.[0,-0.5,0];leg.offset[2]=[3]f64.[0,-0.4,0];
    clip:PoseClip;clip.frames=3;
    clip.convention=PoseConvention.{up=PoseAxis.pos_y,forward=PoseAxis.pos_z,hand=PoseHand.right,order=PoseOrder.wxyz,unit=PoseUnit.metre,root=PoseRoot.absolute,rate=30};
    for k:0..<3 {
        clip.frame[k].root=[3]f64.[0,0.9,0.1*f64(k)];
        clip.frame[k].rotation[0]=pose_vector_quat([3]f64.[-POSE_PI/4*f64(k),0,0]);
        clip.frame[k].rotation[1]=q;clip.frame[k].rotation[2]=[4]f64.[1,0,0,0];
    }
    for k:0..<3 {
        w:=pose_forward(leg,clip,k);
        print("frame %: knee (mm) % % %, foot (mm) % % %\n",k,pose_milli(w.position[1][0]),pose_milli(w.position[1][1]),pose_milli(w.position[1][2]),pose_milli(w.position[2][0]),pose_milli(w.position[2][1]),pose_milli(w.position[2][2]));
    }
```

<!-- source: psionic/examples/pose-conventions.coax convert -->
```jai
    // The same motion in Z-up, -Y forward, right-handed centimetres,
    // w-last quaternions, root deltas, 60 frames per second.
    zup:=PoseConvention.{up=PoseAxis.pos_z,forward=PoseAxis.neg_y,hand=PoseHand.right,order=PoseOrder.xyzw,unit=PoseUnit.centimetre,root=PoseRoot.delta,rate=60};
    moved:=pose_convert(clip,leg,zup);bones:=pose_convert_skeleton(leg,clip.convention,zup);
    print("converted: %, % frames\n",moved.status,moved.clip.frames);
    for k:0..<moved.clip.frames {
        d:=moved.clip.frame[k].root;h:=moved.clip.frame[k].rotation[0];
        print("frame %: root delta (mm) % % %, hip xyzw % % % %\n",k,pose_milli(d[0]/100),pose_milli(d[1]/100),pose_milli(d[2]/100),pose_micro(h[0]),pose_micro(h[1]),pose_micro(h[2]),pose_micro(h[3]));
    }
    w:=pose_forward(bones.skeleton,moved.clip,4);
    print("frame 4 foot (mm): % % %\n",pose_milli(w.position[2][0]/100),pose_milli(w.position[2][1]/100),pose_milli(w.position[2][2]/100));
```

<!-- source: psionic/examples/pose-conventions.coax retarget -->
```jai
    short:=leg;short.offset[1]=[3]f64.[0,-0.25,0];short.offset[2]=[3]f64.[0,-0.2,0];
    small:=pose_retarget(clip,leg,short,2);
    for k:0..<3 {
        w=pose_forward(short,small.clip,k);
        print("short leg frame %: root (mm) % % %, foot (mm) % % %\n",k,pose_milli(w.position[0][0]),pose_milli(w.position[0][1]),pose_milli(w.position[0][2]),pose_milli(w.position[2][0]),pose_milli(w.position[2][1]),pose_milli(w.position[2][2]));
    }
```

<!-- source: psionic/examples/pose-conventions.coax refuse -->
```jai
    undeclared:=clip;undeclared.convention.up=PoseAxis.undeclared;
    print("clip with no up axis: %\n",pose_convert(undeclared,leg,zup).status);
    stretched:=clip;stretched.frame[1].rotation[2]=[4]f64.[1,0,0,0.1];
    print("foot quaternion (1, 0, 0, 0.1): %\n",pose_forward(leg,stretched,1).status);
    print("6D with parallel columns: %\n",pose_six_matrix([6]f64.[1,0,0,2,0,0]).status);
    print("retarget on joint 3 of 3: %\n",pose_retarget(clip,leg,short,3).status);
    loop:=leg;loop.parent[1]=2;
    print("knee whose parent is the foot: %\n",pose_forward(loop,clip,0).status);
```

```text
quaternion wxyz (millionths): 707107 707107 0 0
matrix rows: 1000000 0 0 | 0 0 -1000000 | 0 1000000 0
6D columns: 1000000 0 0 | 0 0 1000000
rotation vector: 1570796 0 0
6D -> matrix -> quaternion: 707107 707107 0 0
frame 0: knee (mm) 0 400 0, foot (mm) 0 400 -400
frame 1: knee (mm) 0 546 454, foot (mm) 0 264 171
frame 2: knee (mm) 0 900 700, foot (mm) 0 500 700
converted: ok, 5 frames
frame 0: root delta (mm) 0 0 900, hip xyzw 0 0 0 1000000
frame 1: root delta (mm) 0 -50 0, hip xyzw -195090 0 0 980785
frame 2: root delta (mm) 0 -50 0, hip xyzw -382683 0 0 923880
frame 3: root delta (mm) 0 -50 0, hip xyzw -555570 0 0 831470
frame 4: root delta (mm) 0 -50 0, hip xyzw -707107 0 0 707107
frame 4 foot (mm): 0 -700 500
short leg frame 0: root (mm) 0 450 0, foot (mm) 0 200 -200
short leg frame 1: root (mm) 0 450 50, foot (mm) 0 132 85
short leg frame 2: root (mm) 0 450 100, foot (mm) 0 250 350
clip with no up axis: convention
foot quaternion (1, 0, 0, 0.1): not_unit
6D with parallel columns: degenerate
retarget on joint 3 of 3: joint
knee whose parent is the foot: parent
```

The output matches the hand work. Frames 0 and 2 of the leg are the traced
positions. In Z-up centimetres the root's first frame is (0, 0, 90) cm, each
60 Hz delta moves −5 cm along Y, which is +Z forward in glTF, and the hip's
frame 1 is the −22.5° quaternion, stored w last. Frame 4 of the converted
clip is source frame 2, and its foot is (x, −z, y) of (0, 0.5, 0.7) m:
(0, −0.7, 0.5) m. On the half-length leg every position is half the original.

Each refusal returns a typed `PoseStatus` and no result: `convention` for an
undeclared field, `not_unit` for a quaternion whose squared length is 1.01,
`degenerate` for 6D columns with no rotation in them, `joint` for a retarget
joint outside 1 .. count − 1, and `parent` for a joint whose parent does not
come before it.

## Checks

[pose.coax](../lib/pose.coax) holds 45 checks of its own, which run in debug
and release with the 6 checks of the chapter 167 file it loads. The
independent ones are:

- **Hand-derived rotations.** The quarter turn about Z, the third of a turn
  about (1, 1, 1) that permutes the axes, the three half turns and four half
  turns about tilted axes, in every form.
- **Round trips on a fixed set.** Seven axes at nine angles each, including
  0, 10⁻¹², 10⁻⁶ and three angles within 10⁻⁹ of π: quaternion → matrix →
  quaternion, quaternion → vector → quaternion and matrix → 6D → matrix agree
  within 10⁻¹⁵, the vector's length is the angle, and q and −q give the same
  vector.
- **Shepperd's rule restated.** A separate function picks the branch from the
  largest diagonal entry, first on ties, and the library's result must match
  it bit for bit on 252 rotations and six integer matrices with exact ties.
- **A second forward kinematics.** 4×4 homogeneous matrices multiplied down
  each joint's path from the root, with no quaternion products, on a branching
  five-joint skeleton with dense rotations, in both quaternion orders.
- **Commuting conventions.** For every pair of three conventions, forward
  kinematics after converting the clip and skeleton equals C times forward
  kinematics before, scaled by the unit factor, and converting back restores
  the clip.
- **Every axis pair.** All 72 combinations of up, forward and handedness:
  the 48 valid ones give an orthonormal basis whose determinant is −1 for
  right-handed and +1 for left-handed coordinates, and the 24 with up and
  forward on one axis are refused.
- **Resampling and retargeting by hand.** 30 to 60, 20, 29 and 7 frames per
  second, and the leg on proportional and non-proportional skeletons.

Admission reads each value's bits, because the interpreter that runs the
checks has no infinities or NaNs; a separate check feeds infinity and NaN bit
patterns through that path. `coax mutate --root-only` on the unit leaves zero
of its 986 mutants alive; chapter 167's quaternion code keeps its own gate.

## What this chapter does not do

Conventions are checked at run time. Coax's typed frames
([SPATIAL.md](../../coax/SPATIAL.md)) do not yet cover rotations between
frames, so a convention is a value, not a type. Euler angles, other units and
other quaternion orders are not offered. Retargeting covers skeletons with the
same joints and parents; mapping between different joint sets is chapter
148's and 149's problem.
