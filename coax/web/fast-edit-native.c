// OS effects for the restricted Coax development replacement policy:
// read a local offer, load a library and publish its scalar function pointer.
// No product build links this adapter. The caller is a single-thread room
// host at a safe point; the selected procedure cannot escape as a callback.
#include "fast-edit-native.h"
#include "fast-edit-offer.h"
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

extern float coax_dev_baseline(float);
extern float coax_dev_observe(float);
extern int coax_dev_identity_byte(int);
typedef float (*Scalar)(float);
static Scalar active;
static void *library;
static char directory[4096], identity[65], error[128];
static int64_t revision, tried, calls, published_calls;
static float last, last_input;
static bool probing;

float coax_dev_dispatch(float value) {
  float result = active ? active(value) : coax_dev_baseline(value);
  if(!probing){++calls;last=result;last_input=value;}
  return result;
}
bool coax_fast_start(const char *dir) {
  if (!dir || !*dir || strlen(dir) >= sizeof directory) return false;
  snprintf(directory, sizeof directory, "%s", dir);
  for (int i=0;i<64;i++) { int c=coax_dev_identity_byte(i);if(c<0||c>127)return false;identity[i]=(char)c; }
  identity[64]=0; active=NULL;revision=tried=calls=published_calls=0;last=last_input=0;error[0]=0;
  return true;
}
void coax_fast_poll(void) {
  if (!directory[0]) return;
  char offer[4200], path[4096];long long next=0;
  snprintf(offer,sizeof offer,"%s/native.current",directory);
  FILE *file=fopen(offer,"r");if(!file)return;
  bool parsed=fscanf(file,"%lld\n",&next)==1&&fgets(path,sizeof path,file)!=NULL;
  fclose(file);if(!parsed||next<=tried)return;tried=next;
  path[strcspn(path,"\r\n")]=0;
  void *candidate=dlopen(path,RTLD_NOW|RTLD_LOCAL);
  if(!candidate){snprintf(error,sizeof error,"candidate library did not load");return;}
  const char *(*candidate_id)(void)=NULL;int64_t(*candidate_revision)(void)=NULL;Scalar function=NULL;
  void *symbol=dlsym(candidate,"coax_dev_patch_identity");memcpy(&candidate_id,&symbol,sizeof candidate_id);
  symbol=dlsym(candidate,"coax_dev_patch_revision");memcpy(&candidate_revision,&symbol,sizeof candidate_revision);
  symbol=dlsym(candidate,"fast_edit_candidate");memcpy(&function,&symbol,sizeof function);
  const char *observed=candidate_id?candidate_id():"";
  if(!function||!candidate_revision||candidate_revision()!=next||
      !fe_publish_offer((int64_t)strlen(identity),strcmp(identity,observed)==0,next,revision,true)) {
    snprintf(error,sizeof error,"candidate identity or revision refused");dlclose(candidate);return;
  }
  // This call site has no active Coax frame. Publish completely before retiring
  // the previous library; a refusal above leaves both the old pointer and state.
  void *previous=library;library=candidate;active=function;revision=next;published_calls=calls;error[0]=0;
  if(previous)dlclose(previous);
}
int coax_fast_status(char *out,size_t capacity) {
  // A real call of the selected gameplay rule; counters separately show calls
  // made by the ordinary room simulation between observations.
  int64_t before=calls;probing=true;float value=coax_dev_observe(0.25f);probing=false;
  return snprintf(out,capacity,"{\"revision\":%lld,\"identity\":\"%s\",\"value\":%.9g,\"game_calls\":%lld,\"game_calls_since_publish\":%lld,\"last_input\":%.9g,\"last_value\":%.9g,\"error\":\"%s\"}",
    (long long)revision,identity,(double)value,(long long)before,(long long)(before-published_calls),(double)last_input,(double)last,error);
}
void coax_fast_close(void) { active=NULL;if(library)dlclose(library);library=NULL;directory[0]=0; }
