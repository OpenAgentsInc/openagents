/* File, clock and logging effects for the Coax CNN consumer. No model,
 * preprocessing, optimizer, split, checkpoint or quality arithmetic lives here. */
#define _POSIX_C_SOURCE 200809L
#include "cnn.h"
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>

_Static_assert(sizeof(float)==4 && sizeof(double)==8, "capture requires binary32/binary64 storage");
#if defined(__BYTE_ORDER__) && __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__
#error This verification capture profile is little-endian only.
#endif

typedef struct { uint8_t *data; int64_t count; } File;
static void fail(const char *message) { fprintf(stderr,"cnn: %s\n",message); exit(2); }
static void path_join(char *out,size_t size,const char *dir,const char *name) {
    int n=snprintf(out,size,"%s/%s",dir,name);if(n<0||(size_t)n>=size)fail("path too long");
}
static File read_file(const char *path) {
    FILE *f=fopen(path,"rb");if(!f)fail("cannot open input");
    if(fseek(f,0,SEEK_END))fail("cannot seek input");long n=ftell(f);
    if(n<0||n>128*1024*1024)fail("input exceeds host byte budget");rewind(f);
    uint8_t *p=malloc(n?(size_t)n:1);if(!p)fail("input allocation failed");
    if(fread(p,1,(size_t)n,f)!=(size_t)n||fclose(f))fail("cannot read input");
    return (File){p,n};
}
static File read_named(const char *dir,const char *name) {
    char path[4096];path_join(path,sizeof path,dir,name);return read_file(path);
}
static void write_file(const char *dir,const char *name,const void *data,size_t size) {
    char path[4096];path_join(path,sizeof path,dir,name);FILE *f=fopen(path,"wbx");
    if(!f)fail("output exists or cannot be created");
    if(fwrite(data,1,size,f)!=size||fflush(f)||fclose(f))fail("cannot write output");
}
static FILE *open_json(const char *dir,const char *name) {
    char path[4096];path_join(path,sizeof path,dir,name);FILE *f=fopen(path,"wx");
    if(!f)fail("JSON output exists or cannot be created");return f;
}
static double monotonic(void) { struct timespec t;if(clock_gettime(CLOCK_MONOTONIC,&t))fail("clock failed");return (double)t.tv_sec+(double)t.tv_nsec/1e9; }
void cn_progress(int64_t step,int64_t phase,double loss,int64_t correct) {
    fprintf(stderr,"CNN validation: phase %lld, step %lld, loss %.9g, correct %lld/1024\n",
            (long long)phase,(long long)step,loss,(long long)correct);fflush(stderr);
}
static coax_v_u8 bytes(File f) { return (coax_v_u8){f.data,f.count}; }

int main(int argc,char **argv) {
    if(argc==2&&!strcmp(argv[1],"--synthetic")) {
        if(!cn_synthetic())fail("synthetic Coax check failed");
        puts("CNN native synthetic: 2910 gradients, shared convolution and linear entries, master preservation and IEEE refusal pass");return 0;
    }
    if(argc!=5||(strcmp(argv[1],"--oracle")&&strcmp(argv[1],"--train")))
        fail("usage: cnn --synthetic | --oracle DATA_DIR MANIFEST OUTPUT_DIR | --train DATA_DIR MANIFEST OUTPUT_DIR");
    if(mkdir(argv[4],0700))fail("output directory must be new (failed runs are retained)");
    File images=read_named(argv[2],"train-images-idx3-ubyte");
    File labels=read_named(argv[2],"train-labels-idx1-ubyte");File manifest=read_file(argv[3]);
    if(!strcmp(argv[1],"--oracle")) {
        double *params=calloc(2910,sizeof *params),*gradient=calloc(2910,sizeof *gradient);
        float scores[10]={0},features[196]={0};if(!params||!gradient)fail("capture allocation failed");
        coax_t_CnFirst r=cn_oracle(bytes(images),bytes(labels),(coax_v_f64){params,2910},(coax_v_f64){gradient,2910},
                                 (coax_v_f32){scores,10},(coax_v_f32){features,196});
        write_file(argv[4],"params.f64",params,2910*sizeof *params);write_file(argv[4],"gradient.f64",gradient,2910*sizeof *gradient);
        write_file(argv[4],"scores.f32",scores,sizeof scores);write_file(argv[4],"features.f32",features,sizeof features);
        FILE *f=open_json(argv[4],"oracle.json");
        fprintf(f,"{\"status\":%u,\"loss\":%.17g,\"parameters\":%lld,\"inputs\":%lld,\"values\":%lld,\"adjoints\":%lld,\"flags\":%lld}\n",
                (unsigned)r.status,r.loss,(long long)r.parameters,(long long)r.inputs,(long long)r.values,(long long)r.adjoints,(long long)r.flags);
        if(fclose(f))fail("cannot close oracle record");free(params);free(gradient);free(images.data);free(labels.data);free(manifest.data);
        return r.status==COAX_CnStatus_ok?0:1;
    }
    File test_images=read_named(argv[2],"t10k-images-idx3-ubyte"),test_labels=read_named(argv[2],"t10k-labels-idx1-ubyte");
    double trace[52]={0};uint8_t *checkpoint=calloc(70180,1),pointer[80]={0},*final_checkpoint=calloc(70180,1),final_pointer[80]={0};
    uint8_t *export_file=calloc(32768,1),*canonical_next=calloc(70180,1),*replayed_next=calloc(70180,1);float *predictions=calloc(20480,sizeof *predictions),*initial_predictions=calloc(20480,sizeof *initial_predictions);
    int64_t *canonical_order=calloc(4096,sizeof *canonical_order),*replayed_order=calloc(4096,sizeof *replayed_order);
    if(!checkpoint||!final_checkpoint||!export_file||!predictions||!canonical_next||!replayed_next||!canonical_order||!replayed_order||!initial_predictions)fail("output allocation failed");
    double started=monotonic();
    coax_t_CnReport r=cn_experiment(bytes(images),bytes(labels),bytes(test_images),bytes(test_labels),bytes(manifest),
        (coax_v_f64){trace,52},(coax_v_u8){checkpoint,70180},(coax_v_u8){pointer,80},
        (coax_v_u8){final_checkpoint,70180},(coax_v_u8){final_pointer,80},(coax_v_u8){export_file,32768},(coax_v_f32){predictions,20480},(coax_v_u8){canonical_next,70180},(coax_v_u8){replayed_next,70180},(coax_v_s64){canonical_order,4096},(coax_v_s64){replayed_order,4096},(coax_v_f32){initial_predictions,20480});
    double seconds=monotonic()-started;
    if(r.steps>0) {
        write_file(argv[4],"validation.f64",trace,sizeof trace);write_file(argv[4],"epoch6.psck",checkpoint,70180);
        write_file(argv[4],"epoch6.pscp",pointer,sizeof pointer);write_file(argv[4],"final.psck",final_checkpoint,70180);
        write_file(argv[4],"final.pscp",final_pointer,sizeof final_pointer);write_file(argv[4],"model.safetensors",export_file,(size_t)r.export_bytes);
        write_file(argv[4],"test-scores.f32",predictions,20480*sizeof *predictions);
        write_file(argv[4],"initial-test-scores.f32",initial_predictions,20480*sizeof *initial_predictions);
        write_file(argv[4],"canonical-next.psck",canonical_next,70180);write_file(argv[4],"replayed-next.psck",replayed_next,70180);
        write_file(argv[4],"canonical-order.i64",canonical_order,4096*sizeof *canonical_order);write_file(argv[4],"replayed-order.i64",replayed_order,4096*sizeof *replayed_order);
    }
    FILE *f=open_json(argv[4],"result.json");
    fprintf(f,"{\"status\":%u,\"steps\":%lld,\"train_rows\":%lld,\"test_rows\":%lld,\"baseline_class\":%lld,\"initial_loss\":%.17g,\"final_loss\":%.17g,\"initial_correct\":%lld,\"final_correct\":%lld,\"baseline_correct\":%lld,\"checkpoint_bytes\":%lld,\"export_bytes\":%lld,\"replay_equal\":%s,\"export_equal\":%s,\"seconds\":%.9f}\n",
        (unsigned)r.status,(long long)r.steps,(long long)r.train_rows,(long long)r.test_rows,(long long)r.baseline_class,
        r.initial_loss,r.final_loss,(long long)r.initial_correct,(long long)r.final_correct,(long long)r.baseline_correct,
        (long long)r.checkpoint_bytes,(long long)r.export_bytes,r.replay_equal?"true":"false",r.export_equal?"true":"false",seconds);
    if(fclose(f))fail("cannot close experiment record");
    free(images.data);free(labels.data);free(test_images.data);free(test_labels.data);free(manifest.data);
    free(checkpoint);free(final_checkpoint);free(export_file);free(predictions);free(canonical_next);free(replayed_next);free(canonical_order);free(replayed_order);free(initial_predictions);
    return r.status==COAX_CnStatus_ok?0:1;
}
