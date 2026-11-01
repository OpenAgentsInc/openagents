# 127. Differentiate nanochat

Forward parity does not show that a model can be trained.
[Chapter 125](ch125-execute-the-pinned-nanochat-gpt.md) runs the pinned
nanochat `gpt.py` (revision `92d63d4e`, read and never executed) and matches
an independent oracle. That pass has pieces a textbook decoder lacks: smear,
per-layer residual and `x0` scalars, value embeddings with gates, rotary
positions, QK normalization scaled by 1.2, grouped-query attention with
sliding windows, a squared-ReLU MLP, backout, an untied head and a logit
softcap. Each one needs a correct gradient before anything can train it.

This chapter writes the backward pass for all of them by hand and checks it
four ways: against an independent oracle that derives no gradient by hand,
against central finite differences in every tensor, for complete parameter
coverage, and through three training steps.

## The profile

This chapter's profile is `nanochat-grad-cpu-f64-v1`:

- Binary64 on the CPU, one thread, every sum in a fixed order.
- The loss is chapter 125's: the mean next-token cross entropy over positions
  0 to n-2 of one sequence of n tokens, from the softcapped logits.
- One call, `nb_gradient`, runs the forward pass, records what the backward
  pass reads on a caller-sized tape, and writes the gradient of every
  parameter in `state_dict` order.
- The recorded forward pass equals chapter 125's to within `1e-13`, not bit
  for bit: it skips the max shift before attention's and the loss's `exp`.
  QK normalization makes every query and key head have length `1.2·sqrt(hd)`,
  so a score is at most `1.44·sqrt(hd) ≤ 11.6` in size for head widths up to
  64, and the softcap keeps every logit within 15. Neither `exp` can
  overflow, so the shift only moves rounding.
- No BF16 autocast, no torch run, and no nanochat optimizer: the training
  steps here are plain gradient descent.

## Work the new derivatives by hand

Most of the backward pass is chapter 113's: matrix products, residual sums
and attention. Five pieces are new.

**RMS normalization.** nanochat normalizes with no weight:
`y = x / r` with `r = sqrt(mean(x²) + eps)`. Its vector-Jacobian product is

```text
dx = (dy - y · (dy·y) / n) / r
```

At `x = (3, 4)`, `mean(x²) = 12.5`, so `r = 3.5355` and `y = (0.8485, 1.1314)`.
For `dy = (1, 0)`, `dy·y = 0.8485`, and

```text
dx = ((1 - 0.8485·0.8485/2), (0 - 1.1314·0.8485/2)) / 3.5355
   = (0.64, -0.48) / 3.5355 = (0.18102, -0.13576)
```

Differentiating `y1 = sqrt(2)·x1/|x|` directly gives `sqrt(2)·16/125 = 0.181019`
and `-sqrt(2)·12/125 = -0.135765`, the same numbers. The final norm, the
two norms in each block and the embedding norm all use this rule. The `eps`
matters only near zero: at `x = (0, 0)`, `r = sqrt(2^-52) = 2^-26`, so
`dx = dy / r` and `dy = (1, 0)` gives `dx = (67108864, 0)`.

**QK normalization.** Each head of q and k is RMS-normalized and then scaled
by 1.2, so its backward is the same rule with `dy` scaled by 1.2 first.

**The softcap.** The logits are `15·tanh(a/15)`. Its slope is
`1 - tanh(a/15)²`: where a logit is 9, `tanh = 0.6` and the slope is
`1 - 0.36 = 0.64`. Large logits pass little gradient back; that is the point
of the cap.

**Squared ReLU.** The MLP computes `u = relu(h)²`, with slope `2·relu(h)`: 3 at
`h = 1.5` and 0 at `h = -0.5`. The code computes `relu(h)` as `(h + |h|)/2`,
which is exact and has no branch.

**Rotary positions.** `apply_rotary_emb` turns each pair `(x1, x2)` into
`(x1 cos θ + x2 sin θ, -x1 sin θ + x2 cos θ)`, a rotation. The backward is the
transpose, the rotation the other way:
`dx1 = dy1 cos θ - dy2 sin θ`, `dx2 = dy1 sin θ + dy2 cos θ`.

Two results follow from the one-block model's structure, and the example
shows both. With one layer, the layer's input is `x0` itself, so
`a = resid·x0 + x0_lambda·x0` and the two scalars get the same gradient,
`da·x0`. And backout takes the saved residual from layer `L//2 = 0`, which is
the final residual, so the head reads `rms((1 - backout)·x)`. RMS
normalization removes the factor, and the gradient of `backout_lambda` is 0
up to rounding. With three layers (the second oracle case) backout reads
layer 1 and its gradient is not zero.

## The pieces

<!-- source: psionic/lib/nanochat/backward.coax rms -->
```jai
// y = x/r with r = sqrt(mean(x^2) + eps). Its vector-Jacobian product:
// dx += (dy - y (dy . y)/n) / r, here for dy scaled by s (1.2 for QK norm).
nb_rms_back :: (x:[]f64,xo:int,n:int,dy:[]f64,dyo:int,s:f64,dx:[]f64,dxo:int) {
    q:=0.0;for i:0..<n {q+=x[xo+i]*x[xo+i];}r:=sqrt(q/f64(n)+NC_EPS);
    dot:=0.0;for i:0..<n {dot+=s*dy[dyo+i]*x[xo+i]/r;}
    for i:0..<n {dx[dxo+i]+=(s*dy[dyo+i]-x[xo+i]/r*dot/f64(n))/r;}
}
```

<!-- source: psionic/lib/nanochat/backward.coax pointwise -->
```jai
// The logit softcap 15*tanh(a/15) has slope 1 - tanh(a/15)^2.
nb_softcap_slope :: (a:f64)->f64 {t:=nc_tanh(a/NC_SOFTCAP);return 1-t*t;}
// relu(h) without a branch; the MLP's u = relu(h)^2 has slope 2*relu(h).
nb_relu :: (h:f64)->f64 {return (h+abs(h))/2;}
```

<!-- source: psionic/lib/nanochat/backward.coax rotary -->
```jai
// apply_rotary_emb rotates each pair (x1, x2) by -theta; the backward
// rotates the gradient by +theta: dx1 = dy1 cos - dy2 sin, dx2 = dy1 sin + dy2 cos.
nb_rotary_back :: (x:[]f64,xo:int,heads:int,hd:int,pos:int) {
    half:=hd/2;
    for h:0..<heads {for i:0..<half {
        angle:=f64(pos)*(1/exp(log(NC_BASE)*f64(2*i)/f64(hd)));co:=cos(angle);si:=sin(angle);
        a:=xo+h*hd+i;y1:=x[a];y2:=x[a+half];x[a]=y1*co-y2*si;x[a+half]=y1*si+y2*co;
    }}
}
```

The forward pass records, per layer and position: the layer input, the
scaled residual `a`, `rms(a)`, q after rotary and after QK norm, k likewise,
v with its value embedding, the gates, the attention output, the residual
after attention, its norm and the MLP's pre-activation. Attention weights
are recomputed in the backward pass rather than stored, so the tape grows
with n, not n². For the one-block model and six positions (width 24, one
key-value head of width 12) the count is: 144 raw embeddings and 144 for `x0`;
one layer of 12 × 144 + 3 × 6 × 12 + 6 = 1,950; 144 for the final residual,
96 for one position's MLP activations, 3 × 144 for backout's copy and the
head's two norms, 72 pre-softcap logits; then the backward pass's scratch,
1,104 values. That is 4,086, which the example prints and a check counts.

<!-- source: psionic/lib/nanochat/backward.coax forward -->
```jai
// The forward pass, keeping what the backward pass reads. It follows
// chapter 125's nc_run in binary64 except that attention and the loss skip
// the max shift, which their bounded inputs make unnecessary.
nb_record :: (c:NcConfig,p:[]f64,tokens:[]int,tape:[]f64,o:NbLayout,logits:[]f64)->f64 {
    n:=tokens.count;d:=c.width;hd:=nc_head_dim(c);kv:=nc_kv_width(c);G:=c.kv_heads;L:=c.layers;nd:=n*d;
    W:=nc_wte(c);sg:=nc_smear_gate(c);lam:=p[2*L];
    for i:0..<n {nc_rms(p,W+tokens[i]*d,d,tape,NB_RAW+i*d);for j:0..<d {tape[o.x0+i*d+j]=tape[NB_RAW+i*d+j];}}
    for i:1..<n {z:=0.0;for j:0..<NC_SMEAR {z+=p[sg+j]*tape[NB_RAW+i*d+j];}gate:=lam*nc_sigmoid(z);
        for j:0..<d {tape[o.x0+i*d+j]+=gate*tape[NB_RAW+(i-1)*d+j];}}
    for j:0..<nd {tape[o.layer+j]=tape[o.x0+j];}
    for l:0..<L {b:=nc_block(c,l);base:=o.layer+l*o.per;A:=base+NB_A*nd;N:=base+NB_N*nd;QR:=base+NB_QR*nd;QH:=base+NB_QH*nd;
        Y:=base+NB_Y*nd;B:=base+NB_B*nd;M:=base+NB_M*nd;H:=base+NB_H*nd;KR:=base+12*nd;KH:=KR+n*kv;V:=KH+n*kv;GT:=V+n*kv;
        out:=base+o.per;
        for i:0..<n {
            for j:0..<d {tape[A+i*d+j]=p[l]*tape[base+i*d+j]+p[L+l]*tape[o.x0+i*d+j];}
            nc_rms(tape,A+i*d,d,tape,N+i*d);nc_mv(p,b.q,d,d,tape,N+i*d,tape,QR+i*d);nc_mv(p,b.k,kv,d,tape,N+i*d,tape,KR+i*kv);nc_mv(p,b.v,kv,d,tape,N+i*d,tape,V+i*kv);
            if nc_has_ve(c,l) {for g:0..<G {z:=0.0;for j:0..<NC_GATE {z+=p[b.gate+g*NC_GATE+j]*tape[N+i*d+j];}gt:=3*nc_sigmoid(z);tape[GT+i*G+g]=gt;
                for x:0..<hd {tape[V+i*kv+g*hd+x]+=gt*p[b.value+tokens[i]*kv+g*hd+x];}}}
            nc_rotary(tape,QR+i*d,c.heads,hd,i);nc_rotary(tape,KR+i*kv,G,hd,i);
            for h:0..<c.heads {nc_rms(tape,QR+i*d+h*hd,hd,tape,QH+i*d+h*hd);for x:0..<hd {tape[QH+i*d+h*hd+x]*=NC_QK;}}
            for g:0..<G {nc_rms(tape,KR+i*kv+g*hd,hd,tape,KH+i*kv+g*hd);for x:0..<hd {tape[KH+i*kv+g*hd+x]*=NC_QK;}}
        }
        for i:0..<n {for h:0..<c.heads {nb_weights(c,tape,o,base,i,h,nc_window(c,l));g:=h/(c.heads/G);
            for x:0..<hd {acc:=0.0;for j:0..<i+1 {acc+=tape[o.pr+j]*tape[V+j*kv+g*hd+x];}tape[Y+i*d+h*hd+x]=acc;}}}
        for i:0..<n {
            nc_mv(p,b.proj,d,d,tape,Y+i*d,tape,o.tmp);for j:0..<d {tape[B+i*d+j]=tape[A+i*d+j]+tape[o.tmp+j];}
            nc_rms(tape,B+i*d,d,tape,M+i*d);nc_mv(p,b.fc,4*d,d,tape,M+i*d,tape,H+i*4*d);
            for f:0..<4*d {r:=nb_relu(tape[H+i*4*d+f]);tape[o.u+f]=r*r;}
            nc_mv(p,b.mlp,d,4*d,tape,o.u,tape,o.tmp);for j:0..<d {tape[out+i*d+j]=tape[B+i*d+j]+tape[o.tmp+j];}
        }
        if l==L/2 {for j:0..<nd {tape[o.xb+j]=tape[out+j];}}
    }
    back:=p[2*L+1];HD:=nc_head(c);loss:=0.0;
    for i:0..<n {for j:0..<d {tape[o.t+i*d+j]=tape[o.xl+i*d+j]-back*tape[o.xb+i*d+j];}nc_rms(tape,o.t+i*d,d,tape,o.nf+i*d);
        sum:=0.0;
        for v:0..<c.vocab {acc:=0.0;for j:0..<d {acc+=p[HD+v*d+j]*tape[o.nf+i*d+j];}tape[o.acc+i*c.vocab+v]=acc;z:=NC_SOFTCAP*nc_tanh(acc/NC_SOFTCAP);logits[i*c.vocab+v]=z;sum+=exp(z);}
        if i+1<n {loss+=log(sum)-logits[i*c.vocab+tokens[i+1]];}
    }
    return loss/f64(n-1);
}
```

## The backward pass

The head comes first: cross entropy, softcap, the untied `lm_head`, the final
norm and backout. Backout subtracts `backout_lambda` times layer `L//2`'s
output, so that output's gradient is kept aside until the backward pass
reaches the layer.

<!-- source: psionic/lib/nanochat/backward.coax head -->
```jai
    // Cross entropy over the n-1 predicted positions: dz = (softmax(z) - onehot)/(n-1).
    // Then the softcap's slope, the untied head, the final RMS norm and backout.
    back:=p[2*L+1];HD:=nc_head(c);
    for i:0..<n-1 {sum:=0.0;for v:0..<c.vocab {sum+=exp(logits[i*c.vocab+v]);}
        for j:0..<d {tape[o.dt+j]=0;tape[o.dn+j]=0;}
        for v:0..<c.vocab {dz:=exp(logits[i*c.vocab+v])/sum;if v==tokens[i+1] {dz-=1;}
            da:=dz/f64(n-1)*nb_softcap_slope(tape[o.acc+i*c.vocab+v]);
            for j:0..<d {grad[HD+v*d+j]+=da*tape[o.nf+i*d+j];tape[o.dn+j]+=da*p[HD+v*d+j];}}
        nb_rms_back(tape,o.t+i*d,d,tape,o.dn,1,tape,o.dt);
        for j:0..<d {tape[o.dx+i*d+j]+=tape[o.dt+j];tape[o.dxb+i*d+j]-=back*tape[o.dt+j];grad[2*L+1]-=tape[o.dt+j]*tape[o.xb+i*d+j];}
    }
```

Then each layer from the last, first the MLP and the attention output
projection:

<!-- source: psionic/lib/nanochat/backward.coax mlp -->
```jai
        // x' = b + W2 relu(W1 rms(b))^2, then b = a + Wproj y.
        for i:0..<n {
            for f:0..<4*d {r:=nb_relu(tape[H+i*4*d+f]);tape[o.u+f]=r*r;tape[o.du+f]=0;}
            nb_mv_back(p,grad,b.mlp,d,4*d,tape,o.u,tape,o.dx+i*d,tape,o.du);
            for f:0..<4*d {tape[o.du+f]*=2*nb_relu(tape[H+i*4*d+f]);}
            for j:0..<d {tape[o.dm+j]=0;}nb_mv_back(p,grad,b.fc,4*d,d,tape,M+i*d,tape,o.du,tape,o.dm);
            nb_rms_back(tape,B+i*d,d,tape,o.dm,1,tape,o.dx+i*d);
            nb_mv_back(p,grad,b.proj,d,d,tape,Y+i*d,tape,o.dx+i*d,tape,o.dy+i*d);
        }
```

Attention follows chapter 113, with nanochat's window rule: a key j is
visible from query i when `i - j <= window`. The same function computes the
weights in both passes, so the windows cannot disagree.

<!-- source: psionic/lib/nanochat/backward.coax attention -->
```jai
        // y_i = sum_j P_ij v_j with P = softmax(q_i . k_j / sqrt(hd)) over the
        // window: dv_j += P_ij dy_i, ds_ij = P_ij (dP_ij - sum_k P_ik dP_ik),
        // dq_i += ds_ij k_j / sqrt(hd), dk_j += ds_ij q_i / sqrt(hd). Query
        // head h reads key-value head h/group, so the groups' gradients add.
        for j:o.dq..<o.dv+n*kv {tape[j]=0;}
        for i:0..<n {for h:0..<c.heads {g:=h/group;nb_weights(c,tape,o,base,i,h,nc_window(c,l));mean:=0.0;
            for j:0..<i+1 {dp:=0.0;for x:0..<hd {dy:=tape[o.dy+i*d+h*hd+x];dp+=dy*tape[V+j*kv+g*hd+x];tape[o.dv+j*kv+g*hd+x]+=tape[o.pr+j]*dy;}
                tape[o.dp+j]=dp;mean+=tape[o.pr+j]*dp;}
            for j:0..<i+1 {ds:=tape[o.pr+j]*(tape[o.dp+j]-mean)/sqrt(f64(hd));
                for x:0..<hd {tape[o.dq+i*d+h*hd+x]+=ds*tape[KH+j*kv+g*hd+x];tape[o.dk+j*kv+g*hd+x]+=ds*tape[QH+i*d+h*hd+x];}}
        }}
```

Then QK norm, rotary, the value-embedding gate (`3·sigmoid(z)` has slope
`gate·(1 - gate/3)`), the three projections, the pre-attention norm and the
two per-layer scalars:

<!-- source: psionic/lib/nanochat/backward.coax project -->
```jai
        // QK norm (scaled by 1.2), rotary, the value-embedding gate, then the
        // three projections and the pre-attention RMS norm.
        for i:0..<n {
            for j:0..<d {tape[o.dqr+j]=0;tape[o.dn+j]=0;}for j:0..<kv {tape[o.dkr+j]=0;}
            for h:0..<c.heads {nb_rms_back(tape,QR+i*d+h*hd,hd,tape,o.dq+i*d+h*hd,NC_QK,tape,o.dqr+h*hd);}
            for g:0..<G {nb_rms_back(tape,KR+i*kv+g*hd,hd,tape,o.dk+i*kv+g*hd,NC_QK,tape,o.dkr+g*hd);}
            nb_rotary_back(tape,o.dqr,c.heads,hd,i);nb_rotary_back(tape,o.dkr,G,hd,i);
            if nc_has_ve(c,l) {for g:0..<G {gt:=tape[GT+i*G+g];row:=b.value+tokens[i]*kv+g*hd;dgt:=0.0;
                for x:0..<hd {dv:=tape[o.dv+i*kv+g*hd+x];dgt+=dv*p[row+x];grad[row+x]+=gt*dv;}
                // gt = 3 sigmoid(z): dz = dgt * gt * (1 - gt/3).
                dz:=dgt*gt*(1-gt/3);for j:0..<NC_GATE {grad[b.gate+g*NC_GATE+j]+=dz*tape[N+i*d+j];tape[o.dn+j]+=dz*p[b.gate+g*NC_GATE+j];}}}
            nb_mv_back(p,grad,b.q,d,d,tape,N+i*d,tape,o.dqr,tape,o.dn);
            nb_mv_back(p,grad,b.k,kv,d,tape,N+i*d,tape,o.dkr,tape,o.dn);
            nb_mv_back(p,grad,b.v,kv,d,tape,N+i*d,tape,o.dv+i*kv,tape,o.dn);
            nb_rms_back(tape,A+i*d,d,tape,o.dn,1,tape,o.dx+i*d);
            // a = resid[l] x + x0[l] x0.
            for j:0..<d {da:=tape[o.dx+i*d+j];grad[l]+=da*tape[base+i*d+j];grad[L+l]+=da*tape[o.x0+i*d+j];
                tape[o.dx0+i*d+j]+=p[L+l]*da;tape[o.dx+i*d+j]=p[l]*da;}
        }
```

Last, smear and the embedding norm. Every layer reads `x0`, so its gradient
is collected across layers and joins the residual's at the bottom:

<!-- source: psionic/lib/nanochat/backward.coax embed -->
```jai
    // x_i = r_i + lam sigmoid(z_i) r_(i-1) with z_i = smear_gate . r_i[0..24],
    // r_i = rms(wte[token_i]); x0 is x, so its gradient joins x's.
    sg:=nc_smear_gate(c);lam:=p[2*L];W:=nc_wte(c);DR:=o.dq;
    for j:0..<nd {tape[o.dx+j]+=tape[o.dx0+j];tape[DR+j]=tape[o.dx+j];}
    for i:1..<n {z:=0.0;for j:0..<NC_SMEAR {z+=p[sg+j]*tape[NB_RAW+i*d+j];}s:=nc_sigmoid(z);ds:=0.0;
        for j:0..<d {dx:=tape[o.dx+i*d+j];ds+=dx*tape[NB_RAW+(i-1)*d+j];tape[DR+(i-1)*d+j]+=lam*s*dx;}
        grad[2*L]+=ds*s;dz:=ds*lam*s*(1-s);
        for j:0..<NC_SMEAR {grad[sg+j]+=dz*tape[NB_RAW+i*d+j];tape[DR+i*d+j]+=dz*p[sg+j];}
    }
    for i:0..<n {nb_rms_back(p,W+tokens[i]*d,d,tape,DR+i*d,1,grad,W+tokens[i]*d);}
```

The entry point refuses before it writes anything: a configuration chapter
125 would refuse (`config`), a parameter or gradient view of the wrong length
or fewer than two positions or more than the context (`shape`), a
non-finite or out-of-range weight (`number`), a token outside the vocabulary
(`token`), too small a tape or logits view (`capacity`), and an alias.

<!-- source: psionic/lib/nanochat/backward.coax api -->
```jai
// The mean next-token loss over tokens and its gradient for every parameter.
// grad has one value per parameter; tape holds nb_tape_count(c, n) values.
// Every refusal comes before the first write.
nb_gradient :: (c:NcConfig,p:[]f64,tokens:[]int,grad:[]f64,tape:[]f64,logits:[]f64,overlap:bool)->NbResult {
    if !nc_valid(c) {return NbResult.{status=NcStatus.config};}
    count:=nc_parameter_count(c);if p.count!=count || grad.count!=count {return NbResult.{status=NcStatus.shape};}
    for x:p {if !(x>=-NC_NUMBER && x<=NC_NUMBER) {return NbResult.{status=NcStatus.number};}}
    n:=tokens.count;if n<2 || n>c.context {return NbResult.{status=NcStatus.shape};}
    for x:tokens {if x<0 || x>=c.vocab {return NbResult.{status=NcStatus.token};}}
    o:=nb_layout(c,n);if tape.count<o.count || logits.count<n*c.vocab {return NbResult.{status=NcStatus.capacity};}
    if overlap {return NbResult.{status=NcStatus.alias};}
    loss:=nb_record(c,p,tokens,tape,o,logits);nb_backward(c,p,tokens,grad,tape,o,logits);
    return NbResult.{loss=loss,written=count};
}
```

## Run it

[The example](../examples/nanochat-gradient.coax) prints the hand-worked
derivatives, the gradient of chapter 126's one-block model (`NE_SMALL`: 12
token IDs, width 24, two query heads over one key-value head) on six tokens,
four parameters against central differences, three steps of gradient descent
and two refusals:

<!-- source: psionic/examples/nanochat-gradient.coax example -->
```jai
main :: () {
    // The hand-worked derivatives.
    x:=[2]f64.[3,4];dy:=[2]f64.[1,0];dx:[2]f64;nb_rms_back(x[..],0,2,dy[..],0,1,dx[..],0);
    print("rms backward at (3,4), dy=(1,0): % %\n",dx[0],dx[1]);
    print("softcap slope where the logit is 9: %\n",nb_softcap_slope(15*0.5*log(4.0)));
    print("squared ReLU slope at 1.5 and -0.5: % %\n",2*nb_relu(1.5),2*nb_relu(-0.5));

    // The one-block model: loss and gradient, tensor by tensor.
    c:=NB_SMALL;n:=nc_parameter_count(c);p:[10216]f64;nc_formula(c,0.5,p[..]);tokens:=[6]int.[3,4,5,6,9,3];
    g:[10216]f64;tape:[8000]f64;logits:[72]f64;need:=nb_tape_count(c,6);
    r:=nb_gradient(c,p[..],tokens[..],g[..],tape[0..<need],logits[..],false);#assert r.status==NcStatus.ok;
    print("% parameters, tape % values, loss %\n",n,need,r.loss);
    inv:[NC_TENSORS_MAX]NcTensor;k:=nc_inventory(c,inv[..]);
    for i:0..<k {t:=inv[i];s:=0.0;nonzero:=0;for e:0..<nc_elements(t) {s+=g[t.offset+e]*g[t.offset+e];if g[t.offset+e]!=0 {nonzero+=1;}}
        print("  %: % of % elements reached, gradient norm %\n",t.kind,nonzero,nc_elements(t),sqrt(s));}

    // Central differences through chapter 125's forward pass.
    b:=nc_block(c,0);picks:=[4]int.[2*c.layers+1,b.gate+3,nc_head(c)+4*24+7,b.k+100];
    for at:picks {print("  parameter %: gradient %, central difference %\n",at,g[at],nb_central(c,p[..],tokens[..],at));}

    // Three steps of plain gradient descent, rate 0.02.
    for step:0..<3 {for i:0..<n {p[i]-=0.02*g[i];}r=nb_gradient(c,p[..],tokens[..],g[..],tape[0..<need],logits[..],false);print("after step %: loss %\n",step+1,r.loss);}

    // Refusals: a tape one value short, and a token outside the vocabulary.
    r=nb_gradient(c,p[..],tokens[..],g[..],tape[0..<need-1],logits[..],false);print("refused: % (loss %)\n",r.status,r.loss);
    tokens[5]=12;r=nb_gradient(c,p[..],tokens[..],g[..],tape[0..<need],logits[..],false);print("refused: % (loss %)\n",r.status,r.loss);
}
```

```shell
coax/bin/coax run psionic/examples/nanochat-gradient.coax
```

```text
rms backward at (3,4), dy=(1,0): 0.18101933598375616 -0.13576450198781712
softcap slope where the logit is 9: 0.64
squared ReLU slope at 1.5 and -0.5: 3.0 0.0
10216 parameters, tape 4086 values, loss 3.968518078777673
  resid: 1 of 1 elements reached, gradient norm 0.5547238854915787
  x0: 1 of 1 elements reached, gradient norm 0.5547238854915787
  smear_lambda: 1 of 1 elements reached, gradient norm 0.9384955670320144
  backout: 1 of 1 elements reached, gradient norm 2.5847379792054426e-16
  wte: 120 of 1536 elements reached, gradient norm 0.8285461604706535
  q: 576 of 576 elements reached, gradient norm 0.49222699491588084
  k: 288 of 288 elements reached, gradient norm 0.5501957567118094
  v: 288 of 288 elements reached, gradient norm 1.0111452204936258
  proj: 576 of 576 elements reached, gradient norm 4.17827374410998
  gate: 12 of 12 elements reached, gradient norm 0.2975719642044178
  fc: 2304 of 2304 elements reached, gradient norm 13.003973131138064
  mlp: 2304 of 2304 elements reached, gradient norm 30.599808389769503
  head: 288 of 1536 elements reached, gradient norm 1.9967925711185905
  smear_gate: 24 of 24 elements reached, gradient norm 0.013438682417094337
  value: 60 of 768 elements reached, gradient norm 0.5634314693958872
  parameter 3: gradient -2.5847379792054426e-16, central difference 0.0
  parameter 3271: gradient -0.008990245802677743, central difference -0.008990245881079773
  parameter 7991: gradient 0.04882266530601232, central difference 0.04882266526617939
  parameter 2216: gradient -0.055362330664180406, central difference -0.055362330608765824
after step 1: loss 2.663154509005495
after step 2: loss 2.1171219469141738
after step 3: loss 1.5515579710662144
refused: capacity (loss 0.0)
refused: token (loss 0.0)
```

The [transcript](../fixtures/nanochat-gradient.out) is exact in debug and
release. The first three lines are the hand values above. Every block matrix
is reached in full. Of the embedding tables, only the rows of the five
distinct tokens get a gradient (5 × 24 of `wte`, 5 × 12 of the value
embedding), and of the head only the 12 real rows of the 64 padded ones.
`resid` and `x0` have the same gradient and `backout` has a gradient of
`2.6e-16`, as worked out above; its central difference is exactly 0. The
other three parameters agree with their central differences to about `1e-10`,
which is the difference's own error at step `1e-5`. The loss falls from 3.97
to 1.55 in three steps of rate 0.02. The refusals are a tape one value short
(`capacity`) and token 12 in a 12-token vocabulary (`token`); neither
computes a loss.

## The checks

`psionic/tests/nanochat_grad_oracle.py` is the independent reference. It
shares no code with the Coax unit and contains no hand-derived gradient: it
runs chapter 125's Python oracle forward pass on scalar values that record
each operation with its local derivative (`+`, `-`, `*`, `/`, square, `exp`,
`log`, `sqrt`, `tanh`), then walks the record backwards once. It writes
`psionic/fixtures/nanochat/grad-goldens.coax` for two models:

| case | layers | heads (query/key-value) | tokens | parameters | recorded operations |
| --- | --- | --- | --- | --- | --- |
| `NB_SMALL` | 1, value embedding, backout at 0 | 2/1 | 6 | 10,216 | 103,056 |
| `NB_DEEP` | 3, value embeddings on 0 and 2, backout at 1 | 4/2 | 7 | 23,696 | 314,823 |

For each case it records the loss, three digests per tensor (the sum, the
sum of squares and a cosine-weighted sum of the gradient) and every 13th
gradient value; for `NB_SMALL` it also records the losses and weight digests
of three descent steps.

`psionic/lib/nanochat/backward.coax` checks, with `coax test`:

- the hand values above (including the `eps` case), and that the rotary backward is the transpose:
  `<R u, w> = <u, Rᵀ w>` at five positions;
- the window rule on three positions: window w keeps exactly the keys with
  `2 - j <= w`, and the weights sum to 1;
- the recorded forward pass against chapter 125's logits and loss, within
  `1e-13`;
- both oracle cases: the loss within `1e-13` and every digest and sample
  within `1e-11 (1 + |b|)`. Changing one digest by one part in a million,
  one sample by `1e-6` or the loss by `1e-9` fails the comparison;
- central differences with step `1e-5` at two elements of each of the 15
  tensors, within `1e-7 (1 + |g|)`;
- a two-block model, where `L//2` is 1 and not 0, so taking backout's copy
  after the wrong layer would show: its loss against chapter 125's and three
  central differences;
- the tape layout, offset by offset, against the count worked above;
- coverage on both cases: an element's gradient is nonzero exactly when the
  loss can reach it. The unreached ones are the embedding and value rows of
  absent tokens and the head's padding rows; on `NB_SMALL` that is
  59 × 24 + 59 × 12 + 52 × 24 = 3,372 zeros, and every other element is
  nonzero;
- three descent steps: losses within `1e-12` and weight digests within
  `1e-11` of the oracle's;
- every refusal, with gradient, tape and logits untouched.

Sliding windows only bite past 128 positions, so their check runs apart, in
`psionic/lib/nanochat/backward-checks.coax`: chapter 125's three-block
`NC_WINDOW` model over 140 positions, where layer 0 is `S` (128 keys). Its
loss matches chapter 125's with the window and without it, the gradient of a
layer-0 key weight differs between the two, and in both it matches a central
difference.

```shell
psionic/check.sh --only nanochat/backward --jobs 8
```

That runs the oracle and compares its output with the committed goldens,
both test files and the example in debug and release, and `coax mutate` on
`backward.coax` (1,076 of 1,076 mutants killed).

## What this does not claim

- Agreement with PyTorch autograd on the real `gpt.py`: the reference here is
  an independent binary64 forward pass with generic reverse mode, not torch.
- BF16 autocast, fused kernels or a GPU path.
- nanochat's optimizer. The steps here are plain gradient descent; chapter
  124 has AdamW and Muon, and [chapter 128](ch128-a-complete-pretraining-run.md)
  is the training run.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
