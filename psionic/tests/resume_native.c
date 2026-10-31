#include "resume.h"
#include "checkpoint-files.h"
#include <assert.h>
#include <dirent.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "resume-oracle.h"
static coax_t_PkDoc docs[5];static uint8_t tokens[14];static coax_t_AwGroup group[1]={{0,256,0.1,0.9,0.99,1e-8,0}};static coax_t_AwPolicy P={0,0};static coax_t_CkIds ids;
static coax_v_t_PkDoc D(void){return (coax_v_t_PkDoc){docs,5};}
static coax_t_RsRun step(coax_t_RsRun r,double *w,double *m,double *v,double *loss){coax_t_RsStep s=rs_step(D(),(coax_v_u8){tokens,14},(coax_v_t_AwGroup){group,1},P,r,(coax_v_f64){w,256},(coax_v_f64){m,256},(coax_v_f64){v,256});assert(s.status==COAX_RsStatus_ok);*loss=s.loss;return s.run;}
/* Coax builds the record and pointer; C only writes files. */
static void save(const char *dir,coax_t_RsRun r,double *w,double *m,double *v,int pointer){
 uint8_t b[6404],p[80];char name[64];coax_t_CkMeta meta={ids,{{0}},r.revision,r.cursor,r.state};
 assert(ck_write(meta,(coax_v_f64){w,256},(coax_v_f64){m,256},(coax_v_f64){v,256},(coax_v_u8){b,6404},0)==COAX_CkStatus_ok);
 coax_t_CkRead back=ck_read((coax_v_u8){b,6404},ids,(coax_v_f64){w,256},(coax_v_f64){m,256},(coax_v_f64){v,256},0);assert(back.status==COAX_CkStatus_ok);
 snprintf(name,sizeof name,"ckpt-%lld.bin",(long long)r.revision);assert(!psionic_ck_publish(dir,name,b,6404));
 if(pointer){assert(ck_pointer_write(r.revision,back.digest,(coax_v_u8){p,80})==COAX_CkStatus_ok);assert(!psionic_ck_publish(dir,"latest",p,80));}
}
/* A new process: the pointer names the record; Coax admits both. */
static coax_t_CkRead load(const char *dir,const char *override,double *w,double *m,double *v,int64_t cut){
 uint8_t p[81],b[6405];char name[64];int64_t np=psionic_ck_load(dir,"latest",p,81);assert(np>=0);
 coax_t_CkPointer q=ck_pointer_read((coax_v_u8){p,np});assert(q.status==COAX_CkStatus_ok);
 snprintf(name,sizeof name,"ckpt-%lld.bin",(long long)q.revision);int64_t nb=psionic_ck_load(dir,override?override:name,b,6405);assert(nb>=0);
 if(cut>=0)nb=cut;
 return rs_resume((coax_v_u8){p,np},(coax_v_u8){b,nb},ids,D(),(coax_v_f64){w,256},(coax_v_f64){m,256},(coax_v_f64){v,256});
}
int main(int argc,char **argv){assert(argc==2);const char *dir=argv[1];
 int lens[5]={3,2,4,2,3},id[5]={2,5,9,11,12},at=0;for(int i=0;i<5;i++){docs[i]=(coax_t_PkDoc){id[i],i<3?0:i==3?1:2,at,lens[i]};for(int j=0;j<lens[i];j++)tokens[at+j]=(uint8_t)(i*3+j);at+=lens[i];}
 ids=rs_ids(D(),(coax_v_u8){tokens,14},(coax_v_t_AwGroup){group,1},P);
 static double w[256],m[256],v[256],a[256],am[256],av[256];double L[12],x;coax_t_RsRun u={{7,0,0,0},{0,0},0},r=u;
 for(int k=0;k<12;k++){u=step(u,w,m,v,&L[k]);assert(fabs(L[k]-rs_losses[k])<=1e-13*(1+fabs(rs_losses[k])));}
 for(int k=0;k<5;k++){r=step(r,a,am,av,&x);assert(x==L[k]);}save(dir,r,a,am,av,1);
 for(int k=5;k<8;k++){r=step(r,a,am,av,&x);assert(x==L[k]);}save(dir,r,a,am,av,0); /* "crash": no pointer update */
 static double b[256],bm[256],bv[256];coax_t_CkRead got=load(dir,NULL,b,bm,bv,-1);assert(got.status==COAX_CkStatus_ok && got.meta.revision==5);
 coax_t_RsRun s={got.meta.cursor,got.meta.state,got.meta.revision};for(int k=5;k<12;k++){s=step(s,b,bm,bv,&x);assert(x==L[k]);}
 assert(!memcmp(b,w,sizeof w) && !memcmp(bm,m,sizeof m) && !memcmp(bv,v,sizeof v) && s.revision==12);
 /* The pointer still names step 5; step 8's record under step 5's name is stale. */
 static double c[256],cm[256],cv[256];c[0]=-7;assert(load(dir,"ckpt-8.bin",c,cm,cv,-1).status==COAX_CkStatus_stale && c[0]==-7);
 assert(load(dir,NULL,c,cm,cv,6403).status==COAX_CkStatus_truncated && c[0]==-7);
 uint8_t bytes[6404];assert(psionic_ck_load(dir,"ckpt-5.bin",bytes,6404)==6404);bytes[3000]^=1;assert(!psionic_ck_publish(dir,"ckpt-5.bin",bytes,6404));
 assert(load(dir,NULL,c,cm,cv,-1).status==COAX_CkStatus_corrupt && c[0]==-7);
 /* Publication leaves only whole files behind. */
 DIR *dd=opendir(dir);struct dirent *e;int files=0;while((e=readdir(dd))){if(e->d_name[0]=='.')continue;assert(!strstr(e->d_name,".tmp."));files++;}closedir(dd);assert(files==3);
 assert(psionic_ck_publish(dir,"a/b",bytes,1)==-1 && psionic_ck_load(dir,"missing",bytes,1)==-1);
 printf("resume: 12-step stream matches the independent losses; resumed from step 5 after a crash at 8, bit for bit; stale, truncated and corrupt files refuse\n");
 return 0;}
