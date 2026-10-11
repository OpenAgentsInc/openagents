"""CPU arithmetic model of the documented Stage 6 attention half boundaries."""

def online_attention(torch, modeling, module, query, key, value, mask, scaling, capture):
    """Model 32-key online updates; this is not a Metal reduction-order oracle."""
    from mps_reference import require
    b,h,n,d=query.shape
    require(b==1 and d==256 and key.shape[0]==b and key.shape[2:]==(n,d), 'unexpected attention shape')
    require(h % key.shape[1]==0 and value.shape==key.shape, 'grouped attention dimensions differ')
    require(mask.shape[-2:]==(n,n), 'causal mask dimensions differ')
    visible=torch.arange(n)[None,:]<=torch.arange(n)[:,None]
    require(bool((mask[0,0][visible]==0).all()) and bool((mask[0,0][~visible]<-1e30).all()), 'only unpadded causal attention is supported')
    # Native attention_prep applies scale before casting Q, and separately casts K/V.
    q=(query*scaling).half().float();k=key.half().float();v=value.half().float()
    capture('attention.q_scaled_half',q);capture('attention.k_half',k);capture('attention.v_half',v)
    k=modeling.repeat_kv(k,module.num_key_value_groups);v=modeling.repeat_kv(v,module.num_key_value_groups)
    scores=torch.matmul(q,k.transpose(2,3)).masked_fill(~visible,float('-inf'))
    capture('attention.scores',scores,allow_negative_infinity=True)
    maximum=torch.full((b,h,n,1),float('-inf'));denominator=torch.zeros_like(maximum)
    output=torch.zeros((b,h,n,d),dtype=torch.float32);effective=torch.zeros_like(scores)
    for first in range(0,n,32):
        end=min(first+32,n);tile=scores[...,first:end];new_max=torch.maximum(maximum,tile.max(-1,keepdim=True).values)
        alpha=torch.where(new_max==maximum,torch.ones_like(maximum),torch.exp(maximum-new_max))
        p=torch.exp(tile-new_max).half().float()
        # Four serial additions per lane, then the native eight-lane XOR tree.
        padded=torch.nn.functional.pad(p,(0,32-(end-first))).reshape(b,h,n,8,4)
        lane=torch.zeros((b,h,n,8),dtype=torch.float32)
        for column in range(4):lane=lane+padded[...,column]
        for distance in (1,2,4):lane=lane+lane[...,torch.arange(8)^distance]
        denominator=denominator*alpha+lane[...,:1]
        output=output*alpha+torch.matmul(p,v[...,first:end,:])
        effective=effective*alpha;effective[...,first:end]=p
        maximum=new_max
    require(bool((denominator>0).all()),'invalid online denominator')
    output=output/denominator;effective=effective/denominator
    require(bool(torch.isfinite(output).all()) and bool(torch.isfinite(effective).all()),'nonfinite online output')
    capture('attention.probabilities',effective);capture('attention.output',output.transpose(1,2).contiguous())
    return output.transpose(1,2).contiguous(),effective


def fixtures(torch,modeling):
    """Check uniform causal means, head mapping, tails, and changing maxima."""
    from types import SimpleNamespace
    from mps_reference import require
    module=SimpleNamespace(num_key_value_groups=3);rows=[]
    for n in (1,17,33):
        q=torch.zeros((1,6,n,256));k=torch.zeros((1,2,n,256))
        v=(torch.arange(2*n*256).reshape(1,2,n,256)%31-15).float()/16
        mask=torch.zeros((1,1,n,n)).masked_fill(torch.arange(n)[None,:]>torch.arange(n)[:,None],float('-inf'))
        output,p=online_attention(torch,modeling,module,q,k,v,mask,1/16,lambda *a,**k:None)
        repeated=modeling.repeat_kv(v,3);want=torch.stack([repeated[:,:,:i+1].mean(2) for i in range(n)],dim=2).transpose(1,2)
        error=float((output-want).abs().max());require(error<=2e-7,'uniform causal average/head association differs')
        require(float((p.sum(-1)-1).abs().max())<=2e-7 and bool((p.masked_select(torch.arange(n)[None,:]>torch.arange(n)[:,None])==0).all()),'probability simplex/causality differs')
        rows.append({'n':n,'uniform_average_max_abs':error})
    for direction in (-1,1):
        n=65;q=torch.zeros((1,6,n,256));q[...,0]=16;k=torch.zeros((1,2,n,256));k[...,0]=torch.arange(n).float()*direction/4
        v=(torch.arange(2*n*256).reshape(1,2,n,256)%17-8).float()/16
        mask=torch.zeros((1,1,n,n)).masked_fill(torch.arange(n)[None,:]>torch.arange(n)[:,None],float('-inf'))
        output,p=online_attention(torch,modeling,module,q,k,v,mask,1/16,lambda *a,**k:None)
        reconstructed=torch.matmul(p,modeling.repeat_kv(v,3)).transpose(1,2)
        error=float((output-reconstructed).abs().max());require(error<=2e-6 and float((p.sum(-1)-1).abs().max())<=2e-6,'online output/effective probabilities disagree')
        rows.append({'n':n,'maxima_direction':direction,'effective_probability_reconstruction_max_abs':error})
    return {'passed':True,'cases':rows,'scope':'Arithmetic model consistency only, not full-model acceptance or bitwise Metal parity.'}
