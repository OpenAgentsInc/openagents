/* Scalar expected values and central differences are generated independently
   by tensor_ops_oracle.py, with no call to this Coax/native implementation. */
#include "tensor-ops-memory.h"
#include "tensor-ops-oracle.h"
#include <assert.h>
#include <limits.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
#define OK COAX_ToStatus_ok
#define EXPECT(x,s) assert((x).status==COAX_ToStatus_##s)
static double max_forward,max_gradient;
static coax_v_f32 f(float *p,int64_t n){return(coax_v_f32){p,n};}
static coax_v_s64 ints(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static coax_v_t_TvStorage os(coax_t_TvStorage *p,int64_t n){return(coax_v_t_TvStorage){p,n};}
static coax_v_t_TvView vs(coax_t_TvView *p,int64_t n){return(coax_v_t_TvView){p,n};}
static coax_t_TvShape shape(int rank,int64_t a,int64_t b,int64_t c,int64_t d){return(coax_t_TvShape){rank,{{a,b,c,d}}};}
static float raw(uint32_t bits){float x;memcpy(&x,&bits,4);return x;}
static void near(float got,double expected,double tolerance,const char *name,const char *field,int index){
    double error=fabs((double)got-expected)/(1+fabs(expected));if(!isfinite(got) || error>tolerance){fprintf(stderr,"%s %s[%d] got %.9g expected %.17g scaled error %.9g > %.9g\n",name,field,index,got,expected,error,tolerance);assert(0);}
    if(tolerance==2e-6){if(error>max_forward)max_forward=error;}else if(error>max_gradient)max_gradient=error;
}
static coax_t_TvView pack(coax_t_TvStorage owner,coax_t_TvShape s,const float *logical,float *physical,int variant){
    for(int i=0;i<64;i++)physical[i]=-91;coax_t_TvView v=tv_contiguous(owner,s,0).view;int n=(int)owner.elements;
    for(int i=0;i<n;i++)physical[i]=logical[i];
    if(variant==1 && n>0 && s.rank>0 && s.dims.e[s.rank-1]>1){int width=(int)s.dims.e[s.rank-1];v.offset=width-1;v.strides.e[s.rank-1]=-1;
        for(int i=0;i<n;i++)physical[(i/width)*width+width-1-i%width]=logical[i];
    }
    if(variant==2 && n>0 && s.rank==2){int rows=(int)s.dims.e[0],cols=(int)s.dims.e[1];v.strides.e[0]=1;v.strides.e[1]=rows;
        for(int i=0;i<n;i++)physical[(i/cols)+(i%cols)*rows]=logical[i];
    }
    assert(tv_layout(owner,v).status==COAX_TvStatus_ok);return v;
}
static void oracles(void){
    for(int tag=0;tag<27;tag++){coax_string name=to_name(tag);assert(name.count==(int64_t)strlen(to_manifest_names[tag]) && !memcmp(name.data,to_manifest_names[tag],(size_t)name.count));assert(to_arity((coax_t_ToOp)tag)==to_manifest_arities[tag]);assert(to_differentiable((coax_t_ToOp)tag)==(tag!=8 && tag!=9));}
    int calls=0;
    for(size_t row=0;row<sizeof to_oracles/sizeof *to_oracles;row++)for(int variant=0;variant<3;variant++){
        const ToOracle *c=&to_oracles[row];coax_t_ToPlan p=to_plan(c->request,c->a_shape,c->b_shape,c->ni);EXPECT(p,ok);assert(p.na==c->na && p.nb==c->nb && p.n==c->n && p.aux==c->aux && p.value_dtype==COAX_ToDtype_f32 && p.aux_dtype==(c->aux?COAX_ToDtype_i64:((c->request.op==COAX_ToOp_sort || c->request.op==COAX_ToOp_topk)?COAX_ToDtype_i64:COAX_ToDtype_none)));
        assert(p.shape.rank==c->out_shape.rank);for(int j=0;j<4;j++)assert(p.shape.dims.e[j]==c->out_shape.dims.e[j]);
        coax_t_TvStorage owners[2]={tv_owner(100+(int64_t)row*10+variant*2,1,c->na).storage,tv_owner(101+(int64_t)row*10+variant*2,1,c->nb).storage};
        float a[64],b[64],a_before[64],b_before[64],out[80],scratch[80],ga[80],gb[80],sa[80],sb[80];int64_t aux[80],aux_stage[80];
        coax_t_TvView views[2]={pack(owners[0],c->a_shape,c->a,a,variant),pack(owners[1],c->b_shape,c->b,b,variant)};coax_t_TvStorage before[2];coax_t_TvView view_before[2];memcpy(before,owners,sizeof owners);memcpy(view_before,views,sizeof views);memcpy(a_before,a,sizeof a);memcpy(b_before,b,sizeof b);
        for(int j=0;j<80;j++){out[j]=-7;scratch[j]=-9;aux[j]=-11;aux_stage[j]=-13;ga[j]=-17;gb[j]=-19;sa[j]=-23;sb[j]=-29;}
        coax_v_s64 indices=ints((int64_t*)c->indices,c->ni);coax_t_ToResult r=to_forward(c->request,os(owners,2),vs(views,2),f(a,c->na),f(b,c->nb),indices,f(out,80),f(scratch,80),ints(aux,80),ints(aux_stage,80));
        if(r.status!=OK){fprintf(stderr,"%s forward status=%d variant=%d\n",c->name,r.status,variant);assert(0);}assert(r.written==c->n && r.aux_written==c->aux);
        for(int j=0;j<c->n;j++)near(out[j],c->expected[j],2e-6,c->name,"value",j);
        for(int j=0;j<c->aux;j++)assert(aux[j]==c->expected_aux[j] && aux_stage[j]==aux[j]);
        for(int j=c->n;j<80;j++)assert(out[j]==-7 && scratch[j]==-9);for(int j=c->aux;j<80;j++)assert(aux[j]==-11 && aux_stage[j]==-13);
        coax_t_ToGradient g=to_vjp(c->request,os(owners,2),vs(views,2),f(a,c->na),f(b,c->nb),indices,f((float*)c->dy,c->n),f(ga,80),f(gb,80),f(sa,80),f(sb,80));
        if(c->differentiable){if(g.status!=OK){fprintf(stderr,"%s VJP status=%d variant=%d\n",c->name,g.status,variant);assert(0);}assert(g.written_a==c->na && g.written_b==c->nb);
            for(int j=0;j<c->na;j++)near(ga[j],c->ga[j],3e-5,c->name,"dA",j);for(int j=0;j<c->nb;j++)near(gb[j],c->gb[j],3e-5,c->name,"dB",j);
            assert(!memcmp(ga,sa,(size_t)c->na*4) && !memcmp(gb,sb,(size_t)c->nb*4));
        }else{EXPECT(g,nondiff);assert(!g.written_a && !g.written_b);for(int j=0;j<80;j++)assert(ga[j]==-17 && gb[j]==-19 && sa[j]==-23 && sb[j]==-29);}
        for(int j=c->na;j<80;j++)assert(ga[j]==-17 && sa[j]==-23);for(int j=c->nb;j<80;j++)assert(gb[j]==-19 && sb[j]==-29);
        assert(!memcmp(owners,before,sizeof owners) && !memcmp(views,view_before,sizeof views) && !memcmp(a,a_before,sizeof a) && !memcmp(b,b_before,sizeof b));calls++;
    }
    printf("independent scalar/VJP oracle: %d cases x 3 layouts = %d; max scaled forward %.9g, gradient %.9g\n",(int)(sizeof to_oracles/sizeof *to_oracles),calls,max_forward,max_gradient);
}
static void failures(void){
    coax_t_TvStorage owners[2]={tv_owner(1,1,3).storage,tv_owner(2,1,3).storage};coax_t_TvShape s=shape(1,3,0,0,0);coax_t_TvView views[2]={tv_contiguous(owners[0],s,0).view,tv_contiguous(owners[1],s,0).view};
    float a[64]={1,2,3},b[64]={2,3,4},out[64],scratch[64],dy[64]={1,2,3},ga[64],gb[64],sa[64],sb[64];int64_t aux[64],stage[64],idx[1]={0};
    for(int j=0;j<64;j++){out[j]=-7;scratch[j]=-9;ga[j]=-17;gb[j]=-19;sa[j]=-23;sb[j]=-29;aux[j]=-31;stage[j]=-37;}
    coax_t_ToRequest r={.op=COAX_ToOp_add,.descending=1,.stride=1};
#define FORWARD(req,aa,bb,ii,oo,ss,qq,tt) to_forward((req),os(owners,2),vs(views,2),(aa),(bb),(ii),(oo),(ss),(qq),(tt))
#define BASIC(req) FORWARD(req,f(a,3),f(b,3),ints(idx,0),f(out,64),f(scratch,64),ints(aux,0),ints(stage,0))
    EXPECT(FORWARD(r,f(a,2),f(b,3),ints(idx,0),f(out,64),f(scratch,64),ints(aux,0),ints(stage,0)),capacity);
    EXPECT(FORWARD(r,f(a,3),f(b,3),ints(idx,0),f(out,2),f(scratch,64),ints(aux,0),ints(stage,0)),capacity);
    EXPECT(to_forward(r,os(owners,1),vs(views,2),f(a,3),f(b,3),ints(idx,0),f(out,64),f(scratch,64),ints(aux,0),ints(stage,0)),capacity);
    owners[0].epoch=2;EXPECT(BASIC(r),view);owners[0].epoch=1;r.op=255;EXPECT(BASIC(r),operation);r.op=COAX_ToOp_add;
    uint32_t bad[]={0x7f800000,0xff800000,0x7fc00001,0x7f800001,0xff800001};for(size_t i=0;i<sizeof bad/sizeof *bad;i++){a[2]=raw(bad[i]);EXPECT(BASIC(r),number);assert(out[0]==-7 && scratch[0]==-9);}a[2]=3;
    EXPECT(FORWARD(r,f(a,3),f(b,3),ints(idx,0),f(a+2,4),f(scratch,64),ints(aux,0),ints(stage,0)),alias);
    EXPECT(FORWARD(r,f(a,3),f(b,3),ints(idx,0),f(out,64),f(out+1,63),ints(aux,0),ints(stage,0)),alias);
    EXPECT(FORWARD(r,f(a,3),f(b,3),ints(idx,0),f((float*)owners,3),f(scratch,64),ints(aux,0),ints(stage,0)),alias);
    a[2]=raw(0x7f7fffff);b[2]=a[2];EXPECT(BASIC(r),arithmetic);assert(out[0]==-7 && out[2]==-7 && scratch[0]==3 && scratch[2]==-9);a[2]=3;b[2]=4;
    r.op=COAX_ToOp_divide;b[2]=0;EXPECT(BASIC(r),arithmetic);b[2]=4;
    owners[1]=tv_owner(2,1,0).storage;views[1]=tv_contiguous(owners[1],shape(1,0,0,0,0),0).view;
#define UNARY(req,ii,qq,tt) FORWARD(req,f(a,3),f(b,0),ii,f(out,64),f(scratch,64),qq,tt)
    r.op=COAX_ToOp_logarithm;a[2]=0;EXPECT(UNARY(r,ints(idx,0),ints(aux,0),ints(stage,0)),arithmetic);a[2]=3;
    r.op=COAX_ToOp_exponential;a[2]=90;EXPECT(UNARY(r,ints(idx,0),ints(aux,0),ints(stage,0)),arithmetic);a[2]=3;
    r.op=COAX_ToOp_take;idx[0]=INT64_MAX;EXPECT(UNARY(r,ints(idx,1),ints(aux,0),ints(stage,0)),index);idx[0]=-1;EXPECT(UNARY(r,ints(idx,1),ints(aux,0),ints(stage,0)),index);idx[0]=0;
    r.op=COAX_ToOp_sum;r.axis=INT64_MAX;EXPECT(UNARY(r,ints(idx,0),ints(aux,0),ints(stage,0)),parameter);r.axis=0;
    r.op=COAX_ToOp_sort;EXPECT(UNARY(r,ints(idx,0),ints((int64_t*)views,3),ints(stage,64)),alias);
    EXPECT(UNARY(r,ints(idx,0),ints(aux,2),ints(stage,64)),capacity);
    /* All gradient public buffers publish together, even on a late overflow. */
    owners[1]=tv_owner(2,1,3).storage;views[1]=tv_contiguous(owners[1],s,0).view;r.op=COAX_ToOp_multiply;a[0]=0;a[1]=0;a[2]=0;b[0]=raw(0x7f7fffff);b[1]=b[0];b[2]=b[0];
    EXPECT(to_vjp(r,os(owners,2),vs(views,2),f(a,3),f(b,3),ints(idx,0),f(dy,3),f(ga,64),f(gb,64),f(sa,64),f(sb,64)),arithmetic);assert(sa[0]==b[0] && ga[0]==-17 && gb[0]==-19 && sb[0]==-29);
    EXPECT(to_vjp(r,os(owners,2),vs(views,2),f(a,3),f(b,3),ints(idx,0),f(dy,3),f(dy,3),f(gb,64),f(sa,64),f(sb,64)),alias);
    for(int j=0;j<64;j++)assert(out[j]==-7 && ga[j]==-17 && gb[j]==-19);
    /* Cross entropy has no axis parameter: irrelevant fields must not index. */
    owners[0]=tv_owner(1,2,3).storage;views[0]=tv_contiguous(owners[0],shape(2,1,3,0,0),0).view;owners[1]=tv_owner(2,2,0).storage;views[1]=tv_contiguous(owners[1],shape(1,0,0,0,0),0).view;r.op=COAX_ToOp_cross_entropy;r.axis=INT64_MAX;a[0]=a[1]=a[2]=0;
    EXPECT(UNARY(r,ints(idx,1),ints(aux,0),ints(stage,0)),ok);near(out[0],log(3.),2e-6,"cross_entropy","ignored axis",0);
    puts("native refusals: nonfinite classes, short/foreign metadata, raw unknown tags, domains, invalid indices/axis, actual aliases and late forward/VJP publication pass");
}
static void ties_and_work(void){
    coax_t_TvStorage owners[2]={tv_owner(1,1,4).storage,tv_owner(2,1,0).storage};coax_t_TvView views[2]={tv_contiguous(owners[0],shape(4,1,1,2,2),0).view,tv_contiguous(owners[1],shape(1,0,0,0,0),0).view};
    float a[4]={3,3,1,2},none[1]={0},dy[4]={4,0,0,0},ga[4],sa[4];int64_t idx[1]={0};
    coax_t_ToRequest r={.op=COAX_ToOp_maxpool2d,.height=2,.width=2,.stride=1};
    EXPECT(to_vjp(r,os(owners,2),vs(views,2),f(a,4),f(none,0),ints(idx,0),f(dy,1),f(ga,4),f(none,0),f(sa,4),f(none,0)),ok);assert(ga[0]==4 && ga[1]==0 && ga[2]==0 && ga[3]==0);
    owners[0].elements=3;views[0]=tv_contiguous(owners[0],shape(1,3,0,0,0),0).view;a[0]=-1;a[1]=0;a[2]=2;dy[0]=1;dy[1]=2;dy[2]=3;r.op=COAX_ToOp_relu;
    EXPECT(to_vjp(r,os(owners,2),vs(views,2),f(a,3),f(none,0),ints(idx,0),f(dy,3),f(ga,4),f(none,0),f(sa,4),f(none,0)),ok);assert(ga[0]==0 && ga[1]==0 && ga[2]==3);
    a[0]=a[1]=2;a[2]=1;r.op=COAX_ToOp_maximum;
    EXPECT(to_vjp(r,os(owners,2),vs(views,2),f(a,3),f(none,0),ints(idx,0),f(dy,1),f(ga,4),f(none,0),f(sa,4),f(none,0)),ok);assert(ga[0]==1 && ga[1]==0 && ga[2]==0);
    float big_a[1024]={0},big_b[1024]={0};owners[0]=tv_owner(3,1,1024).storage;owners[1]=tv_owner(4,1,1024).storage;views[0]=tv_contiguous(owners[0],shape(2,32,32,0,0),0).view;views[1]=tv_contiguous(owners[1],shape(2,32,32,0,0),0).view;r.op=COAX_ToOp_matmul;
    EXPECT(to_vjp(r,os(owners,2),vs(views,2),f(big_a,1024),f(big_b,1024),ints(idx,0),f(none,0),f(none,0),f(none,0),f(none,0),f(none,0)),limit);
    coax_t_ToPlan p=to_plan((coax_t_ToRequest){.op=COAX_ToOp_matmul},shape(2,1024,1,0,0),shape(2,1,1024,0,0),0);assert(p.status==OK && p.n==1048576 && p.work==1048576);
    p=to_plan((coax_t_ToRequest){.op=COAX_ToOp_update},shape(1,1048576,0,0,0),shape(1,1048576,0,0,0),1048576);EXPECT(p,limit);
    puts("tie subgradients, ReLU at zero, exact forward work ceiling and independent VJP/index-update work refusals pass");
}

int main(void){oracles();failures();ties_and_work();return 0;}
