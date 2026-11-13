#!/usr/bin/env python3
# Firm reason: 2 (independent exact-rational geometry, quantization-cell and finite-segment envelope oracle; no renderer, shader, depth image or color comparison code is imported).
"""Raw F32 geometry in; complete declared boundary-region facts and bytes out.

This is an explicit conservative geometry scope, not a hardware error bound,
not a legal alternative shader result, and not a depth-winner certificate.
The mathematical input consists only of stored clip/world coordinates and a
locked measured-profile declaration. Nonfinite values anywhere in a packet
are fatal. Ineligible geometry remains outside this exclusion certificate.
"""
from collections import Counter,defaultdict
from fractions import Fraction as F
from hashlib import sha256
from pathlib import Path
import json
import struct
import sys

VERSION='exact-shared-world-fold-linf-cell-v1'
SCOPE='declared_conservative_geometry_boundary_not_hardware_error_bound'
ELIGIBILITY='exact_clip_world_shared_noncoplanar_edges_of_positive_w_in_depth_nondegenerate_guard_valid_original_triangles'

def decode_f32(word):
    exponent=(word>>23)&255;mantissa=word&0x7fffff
    if exponent==255:raise ValueError('nonfinite input word')
    sign=-1 if word>>31 else 1
    if exponent==0:return F(sign*mantissa,2**149)
    significand=sign*((1<<23)+mantissa);shift=exponent-150
    return F(significand*2**shift) if shift>=0 else F(significand,2**-shift)

def nearest(value,mode):
    if mode not in ('nearest-even','nearest-positive'):raise ValueError('unsupported rounding profile')
    low=value.numerator//value.denominator;remainder=value-low
    return low+int(remainder>F(1,2) or (remainder==F(1,2) and (mode=='nearest-positive' or low%2)))

def reachable(a,b,p,r):
    """Exact t interval for p in segment(a,b)+[-r,r]^2; no line proxy."""
    if r not in (0,1):raise ValueError('radius must be zero control or fixed one unit')
    if a==b:return None
    low,high=F(0),F(1)
    for x,y,z in zip(a,b,p):
        delta=y-x
        if delta==0:
            if not z-r<=x<=z+r:return None
        else:
            u,v=F(z-r-x,delta),F(z+r-x,delta)
            low=max(low,min(u,v));high=min(high,max(u,v))
            if low>high:return None
    return low,high

def difference(a,b):return tuple(x-y for x,y in zip(a,b))
def cross(a,b):return (a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])
def dot(a,b):return sum(x*y for x,y in zip(a,b))
def rational(v):return [v.numerator,v.denominator]
def ceil_fraction(v):return -((-v.numerator)//v.denominator)

def projected(vertex,width,height,mode):
    x,y,z,w=vertex[:4]
    if w<=0:raise ValueError('nonpositive projection denominator')
    px=(x/w+1)*F(width,2);py=(1-y/w)*F(height,2)
    return (2*nearest(px*256,mode),2*nearest(py*256,mode)),(px,py)

def edge_pixels(a,b,width,height):
    """Row-bounded exact segment-square intersection; complete pixel census."""
    if a==b:return
    ymin=max(0,ceil_fraction(F(min(a[1],b[1])-1-256,512)))
    ymax=min(height-1,(max(a[1],b[1])+1-256)//512)
    for y in range(ymin,ymax+1):
        py=y*512+256;dy=b[1]-a[1]
        if dy==0:
            if not py-1<=a[1]<=py+1:continue
            lo,hi=F(0),F(1)
        else:
            u,v=F(py-1-a[1],dy),F(py+1-a[1],dy)
            lo=max(F(0),min(u,v));hi=min(F(1),max(u,v))
            if lo>hi:continue
        dx=b[0]-a[0];x0=a[0]+lo*dx;x1=a[0]+hi*dx
        xmin=max(0,ceil_fraction((min(x0,x1)-1-256)/512))
        xmax=min(width-1,(max(x0,x1)+1-256)//512)
        for x in range(xmin,xmax+1):
            interval=reachable(a,b,(x*512+256,py),1)
            if interval is not None:yield x,y,interval

def derive(data,width,height,mode):
    if not 1<=width<=8192 or not 1<=height<=8192 or width*height>1048576:raise ValueError('bounded viewport required')
    if mode not in ('nearest-even','nearest-positive'):raise ValueError('unsupported rounding profile')
    if not data or len(data)%168 or len(data)>20000*168:raise ValueError('complete bounded triangle packets required')
    words=struct.unpack('<'+str(len(data)//4)+'I',data)
    values=[decode_f32(word) for word in words] # includes every non-geometric input word
    vertices=[tuple(values[i:i+7]) for i in range(0,len(values),14)]
    keys=[tuple(words[i:i+7]) for i in range(0,len(words),14)]
    triangles=len(vertices)//3;status=[];screens=[];groups=defaultdict(list);packet_hashes=[]
    for primitive in range(triangles):
        tri=vertices[primitive*3:primitive*3+3];world=[v[4:7] for v in tri]
        normal=cross(difference(world[1],world[0]),difference(world[2],world[0]))
        reason='eligible';screen=None
        if normal==(0,0,0):reason='world_degenerate'
        elif any(v[3]<=0 for v in tri):reason='nonpositive_w'
        elif any(v[2]<0 or v[2]>v[3] for v in tri):reason='near_far_crossing_or_outside'
        else:
            projected_vertices=[projected(v,width,height,mode) for v in tri]
            if any(not -65536<=p[0]<=width+65536 or not -65536<=p[1]<=height+65536 for s,p in projected_vertices):reason='outside_guard'
            else:
                screen=[s for s,p in projected_vertices];a,b,c=screen
                if (b[0]-a[0])*(c[1]-a[1])==(b[1]-a[1])*(c[0]-a[0]):reason='snapped_screen_degenerate'
        status.append(reason);screens.append(screen);packet_hashes.append(sha256(data[primitive*168:(primitive+1)*168]).hexdigest())
        for local in range(3):
            indices=(primitive*3+local,primitive*3+(local+1)%3)
            key=tuple(sorted((keys[indices[0]],keys[indices[1]])))
            groups[key].append((primitive,local))
    census=dict(triangles=triangles,edge_occurrences=triangles*3,unique_endpoint_edges=len(groups),exact_shared_edges=0,
                unmatched_edges=0,shared_edges_with_ineligible_owners=0,eligible_shared_coplanar_edges=0,
                eligible_shared_fold_edges=0,degenerate_endpoint_edges=0,projected_degenerate_shared_edges=0)
    census['triangle_eligibility']=dict(sorted(Counter(status).items()))
    mask=bytearray(width*height);zero=bytearray(width*height);edges=[];witnesses=[];pixel_edges=defaultdict(list)
    for key,occurrences in sorted(groups.items()):
        owners=sorted(set(p for p,e in occurrences))
        if len(owners)<2:census['unmatched_edges']+=1;continue
        census['exact_shared_edges']+=1
        if key[0]==key[1]:census['degenerate_endpoint_edges']+=1;continue
        eligible=[p for p in owners if status[p]=='eligible']
        if len(eligible)<len(owners):census['shared_edges_with_ineligible_owners']+=1
        if len(eligible)<2:continue
        endpoint_values=[tuple(decode_f32(w) for w in k) for k in key]
        a,b=(e[4:7] for e in endpoint_values);d=difference(b,a)
        third=[]
        for owner in eligible:
            tri=vertices[owner*3:owner*3+3];third.append(next(v[4:7] for v in tri if v!=endpoint_values[0] and v!=endpoint_values[1]))
        base_normal=cross(d,difference(third[0],a));fold_witness=None
        for i in range(1,len(eligible)):
            volume=dot(base_normal,difference(third[i],a))
            if volume!=0:fold_witness={'primitives':[eligible[0],eligible[i]],'exact_triple_product':rational(volume)};break
        if fold_witness is None:census['eligible_shared_coplanar_edges']+=1;continue
        snapped=[projected(v,width,height,mode)[0] for v in endpoint_values]
        if snapped[0]==snapped[1]:census['projected_degenerate_shared_edges']+=1;continue
        census['eligible_shared_fold_edges']+=1;edge_id=len(edges)
        edge={'id':edge_id,'primitive_ids':owners,'eligible_primitive_ids':eligible,
              'ineligible_owners':{str(p):status[p] for p in owners if status[p]!='eligible'},
              'endpoint_key_words':[[f'{w:08x}' for w in k] for k in key],
              'clip_endpoints':[[rational(v) for v in e[:4]] for e in endpoint_values],
              'world_endpoints':[[rational(v) for v in e[4:7]] for e in endpoint_values],
              'snapped_xy_units_1_512':snapped,'fold_witness':fold_witness,
              'triangle_packet_sha256':{str(p):packet_hashes[p] for p in owners}}
        edges.append(edge)
        for x,y,interval in edge_pixels(*snapped,width,height):
            index=y*width+x;mask[index]=1;z=reachable(*snapped,(x*512+256,y*512+256),0)
            if z is not None:zero[index]=1
            witnesses.append({'x':x,'y':y,'edge':edge_id,'t':[rational(v) for v in interval],'zero_radius_centerline':z is not None})
            pixel_edges[index].append(edge_id)
    census['masked_pixels']=sum(mask);census['zero_radius_centerline_pixels']=sum(zero)
    census['witness_records']=len(witnesses)
    return dict(mask=bytes(mask),zero=bytes(zero),census=census,edges=edges,witnesses=witnesses,
                pixel_edges=pixel_edges,triangle_status=status)

def json_bytes(value):return (json.dumps(value,sort_keys=True,separators=(',',':'))+'\n').encode()
def artifact(path,data,**extra):
    path.write_bytes(data)
    return dict(file=path.name,sha256=sha256(data).hexdigest(),bytes=len(data),**extra)

def main():
    if len(sys.argv)!=4:raise ValueError('usage: oracle RAW_VERTICES CONFIG.json EXISTING_OUTPUT_DIR')
    raw,config_file,out=map(Path,sys.argv[1:]);config=json.loads(config_file.read_text())
    width,height,mode=config['width'],config['height'],config['profile']['rounding']
    required={'rounding':mode,'subpixel_bits':8,'xy':'guard-band','guard_pixels':65536}
    if config['profile']!=required:raise ValueError('only the declared measured eight-bit guard profile is supported')
    data=raw.read_bytes();result=derive(data,width,height,mode)
    edges=b''.join(json_bytes(row) for row in result['edges']);witnesses=b''.join(json_bytes(row) for row in result['witnesses'])
    facts=dict(schema='aethyr.render.shared-edge-envelope.v1',algorithm_version=VERSION,scope=SCOPE,
               eligibility=ELIGIBILITY,width=width,height=height,profile=required,radius={'numerator':1,'denominator':512},
               geometry={'sha256':sha256(data).hexdigest(),'bytes':len(data),'triangles':len(data)//168,'vertex_stride_bytes':56},
               mask=artifact(out/'mask.bin',result['mask'],pixels=sum(result['mask'])),
               zero_radius_control=artifact(out/'zero-radius-mask.bin',result['zero'],pixels=sum(result['zero'])),
               zero_mask_control=artifact(out/'zero-mask.bin',bytes(width*height),pixels=0),
               edges=artifact(out/'edges.jsonl',edges,records=len(result['edges'])),
               witnesses=artifact(out/'witnesses.jsonl',witnesses,records=len(result['witnesses']),pixels=sum(result['mask'])),
               census=result['census'],oracle_source={'path':str(Path(__file__).resolve()),'sha256':sha256(Path(__file__).read_bytes()).hexdigest()},
               coordinate_model='exact_stored_f32_rationals_perspective_divide_then_declared_nearest_grid',
               geometry_scope='All original triangles and exact endpoint edge occurrences are censused. Eligible shared noncoplanar groups only; no synthetic clipping edges. Ineligible and unmatched geometry remains strict. Occluded eligible edges may be conservatively included without claiming visibility.',
               unchecked='Whole RGBA packets at mask pixels are UNQUALIFIED; no color/depth/hardware-legality conclusion. Nonfinite values remain fatal outside this oracle too.',
               image_inputs=[],admitted=False)
    (out/'facts.json').write_bytes(json_bytes(facts))

if __name__=='__main__':main()
