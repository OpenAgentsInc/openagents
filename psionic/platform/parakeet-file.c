/* Offline Parakeet file effects only. Coax owns argv admission, WAV decoding,
 * global-phase resampling, exact windows, emission timing and every writer.
 * No transcript reaches stdout before every model window has succeeded.
 * An output I/O failure after publication begins may still leave partial bytes.
 */
#define _POSIX_C_SOURCE 200809L
#define _DEFAULT_SOURCE 1
#ifdef __APPLE__
#define _DARWIN_C_SOURCE 1
#endif
#include "parakeet-file.h"
#include "psionic-parakeet.h"
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

enum { FILE_SAMPLES=327520, FILE_TOKENS=2560, FILE_TEXT=8193,
       FILE_OUTPUT=131072, FILE_TABLE=8194, FILE_STAGE=262144 };
static volatile sig_atomic_t file_cancelled;
static void file_signal(int signo) { (void)signo; file_cancelled=1; }
static void file_wipe(void *data,size_t bytes) {
  volatile unsigned char *p=data; if(p)while(bytes--)*p++=0;
}
static int file_write(FILE *file,const void *bytes,size_t n) {
  return n==0 || fwrite(bytes,1,n,file)==n;
}
static int file_emit(FILE *file,int64_t n,const uint8_t *out) {
  return n>=0 && n<=FILE_OUTPUT && file_write(file,out,(size_t)n);
}
static int file_copy(FILE *from,FILE *to,uint8_t *scratch) {
  if(file_cancelled||fflush(from)||fseek(from,0,SEEK_SET))return 0;
  for(;;){if(file_cancelled)return 0;size_t n=fread(scratch,1,FILE_OUTPUT,from);if(n&&!file_write(to,scratch,n))return 0;if(n<FILE_OUTPUT)return !ferror(from)&&!file_cancelled;}
}
// Raw writes expose positive partial writes and EINTR to the cancellation
// poll. libc fwrite may retry those internally on a full output pipe.
static int file_publish(FILE *from,uint8_t *scratch) {
  if(file_cancelled||fflush(from)||fseek(from,0,SEEK_SET))return 0;
  for(;;) {
    if(file_cancelled)return 0;
    size_t n=fread(scratch,1,FILE_OUTPUT,from),at=0;
    while(at<n) {
      if(file_cancelled)return 0;
      ssize_t wrote=write(STDOUT_FILENO,scratch+at,n-at);
      if(wrote<0){if(errno==EINTR&&!file_cancelled)continue;return 0;}
      if(wrote==0)return 0;at+=(size_t)wrote;
    }
    if(n<FILE_OUTPUT)return !ferror(from)&&!file_cancelled;
  }
}
static int file_same_stat(const struct stat *a,const struct stat *b) {
  if(a->st_dev!=b->st_dev||a->st_ino!=b->st_ino||a->st_size!=b->st_size||a->st_mtime!=b->st_mtime||a->st_ctime!=b->st_ctime)return 0;
#ifdef __APPLE__
  return a->st_mtimespec.tv_nsec==b->st_mtimespec.tv_nsec && a->st_ctimespec.tv_nsec==b->st_ctimespec.tv_nsec;
#else
  return a->st_mtim.tv_nsec==b->st_mtim.tv_nsec && a->st_ctim.tv_nsec==b->st_ctim.tv_nsec;
#endif
}
static int file_dump(psionic_parakeet *s,const char *prefix,uint8_t *scratch) {
  char path[4096];float *stage=malloc(FILE_STAGE*sizeof *stage);if(!stage)return 0;
  const char *suffixes[]={"tokens.json","features.f32","encoded.f32"};int ok=1;
  for(int i=0;i<3 && ok;i++) {
    int length=snprintf(path,sizeof path,"%s.%s",prefix,suffixes[i]);if(length<0||(size_t)length>=sizeof path){ok=0;break;}
    FILE *file=fopen(path,"wb");if(!file){ok=0;break;}
    if(i==0)ok=psionic_parakeet_json(s,(char*)scratch,FILE_OUTPUT)&&file_write(file,scratch,strlen((char*)scratch));
    else {int64_t n=psionic_parakeet_stage(s,i-1,stage,FILE_STAGE);ok=n>=0&&file_write(file,stage,(size_t)n*sizeof *stage);}
    if(fclose(file))ok=0;
  }
  file_wipe(stage,FILE_STAGE*sizeof *stage);free(stage);return ok;
}
int pkfile_main(int argc,char **argv) {
  if(argc<0||argc>128){fprintf(stderr,"too many Parakeet arguments\n");return 2;}
  coax_string args[128];for(int i=0;i<argc;i++)args[i]=(coax_string){argv[i],(int64_t)strlen(argv[i])};
  coax_t_PkFileOptions options=pkfile_options((coax_v_string){args,argc});
  if(!options.ok){fprintf(stderr,"usage: psionic-parakeet transcribe MODEL_DIR WAV [--gpu|--cpu] [--text|--json|--srt|--vtt] [--dump-prefix PATH]\n");return 2;}
  file_cancelled=0;struct sigaction action={0};action.sa_handler=file_signal;sigemptyset(&action.sa_mask);
  // No SA_RESTART: cancellation must interrupt a blocked output pipe too.
  if(sigaction(SIGINT,&action,NULL)||sigaction(SIGTERM,&action,NULL)){fprintf(stderr,"cannot install cancellation handlers\n");return 1;}
  signal(SIGPIPE,SIG_IGN);
  int status=1,fd=-1,published=0;void *mapped=MAP_FAILED;size_t bytes=0;
  psionic_parakeet *engine=NULL;FILE *spool=NULL,*text_spool=NULL;
  float *samples=NULL;double *table=NULL;uint8_t *output=NULL;
  char text[FILE_TEXT],error[512];int64_t tokens[FILE_TOKENS],frames[FILE_TOKENS],count=0;
  psionic_parakeet_info info={0};
  fd=open(argv[3],O_RDONLY);struct stat initial={0};
  if(fd<0||fstat(fd,&initial)||!S_ISREG(initial.st_mode)||initial.st_size<1||(uintmax_t)initial.st_size>INT64_MAX){fprintf(stderr,"cannot open a regular WAV file\n");status=65;goto done;}
  bytes=(size_t)initial.st_size;mapped=mmap(NULL,bytes,PROT_READ,MAP_PRIVATE,fd,0);
  if(mapped==MAP_FAILED){fprintf(stderr,"cannot map WAV\n");status=65;goto done;}
  coax_v_u8 input={(uint8_t*)mapped,(int64_t)bytes};
  coax_t_WvInfo wave=pkfile_wav_info(input);int64_t total=pkfile_length(wave),parts=pkfile_chunks(total);
  if(parts<1){fprintf(stderr,"WAV refused: complete WAV required, 320..1341521920 resampled mono16k samples, at most4096 windows\n");status=65;goto done;}
  if(!pkfile_dumps(parts,options.dump)){fprintf(stderr,"--dump-prefix requires one model window; long-file JSON exposes per-window token timing\n");status=2;goto done;}
  samples=calloc(FILE_SAMPLES,sizeof *samples);table=malloc(FILE_TABLE*sizeof *table);output=malloc(FILE_OUTPUT);
  spool=tmpfile();if(options.format==3)text_spool=tmpfile();
  if(!samples||!table||!output||!spool||(options.format==3&&!text_spool)||!pkfile_table((coax_v_f64){table,FILE_TABLE})){fprintf(stderr,"cannot allocate bounded file workspace or private output spool\n");goto done;}
  engine=psionic_parakeet_open(argv[2],options.gpu,error,sizeof error);
  if(!engine){fprintf(stderr,"%s\n",error);goto done;}
  if(!file_emit(spool,pkfile_begin(options.format,total,options.gpu,(coax_v_u8){output,FILE_OUTPUT}),output)){fprintf(stderr,"cannot begin transcript output\n");goto done;}
  int64_t completed=0,cues=0;int prior_text=0;
  for(int64_t index=0;index<parts;index++) {
    if(file_cancelled){status=130;goto done;}
    coax_t_PkFileWindow window=pkfile_window(total,index);
    if(!window.ok||window.count<1||window.count>FILE_SAMPLES||pkfile_audio(input,window.offset,(coax_v_f64){table,FILE_TABLE},(coax_v_f32){samples,window.count})!=window.count){fprintf(stderr,"WAV window conversion refused\n");status=65;goto done;}
    if(!psionic_parakeet_text(engine,samples,window.count,text,sizeof text,error,sizeof error)){fprintf(stderr,"window%lld: %s\n",(long long)index,error);goto done;}
    if(!psionic_parakeet_last_info(engine,&info)||(count=psionic_parakeet_tokens(engine,tokens,frames,FILE_TOKENS))<0){fprintf(stderr,"cannot copy successful token/frame snapshot\n");goto done;}
    if(file_cancelled){status=130;goto done;}
    if(options.dump>=0&&!file_dump(engine,argv[options.dump],output)){fprintf(stderr,"cannot write inspection snapshots\n");goto done;}
    int64_t n=pkfile_render(options.format,index,cues,total,window.offset,window.count,info.encoded_frames,(coax_v_s64){tokens,count},(coax_v_s64){frames,count},(coax_string){text,(int64_t)strlen(text)},(coax_v_u8){output,FILE_OUTPUT});
    if(!file_emit(spool,n,output)){fprintf(stderr,"window timing/text refused or output spool failed\n");goto done;}
    if(options.format==3){n=pkfile_json_text((coax_string){text,(int64_t)strlen(text)},prior_text,(coax_v_u8){output,FILE_OUTPUT});if(!file_emit(text_spool,n,output)){fprintf(stderr,"cannot spool JSON text\n");goto done;}}
    cues=pkfile_cues(cues,(coax_string){text,(int64_t)strlen(text)});prior_text=cues>0;completed++;
    if(!psionic_parakeet_clear(engine)){fprintf(stderr,"cannot clear completed utterance\n");goto done;}
    file_wipe(samples,FILE_SAMPLES*sizeof *samples);
#ifdef MADV_DONTNEED
    (void)madvise(mapped,bytes,MADV_DONTNEED);
#endif
  }
  if(!pkfile_publish(completed,parts,file_cancelled)){status=file_cancelled?130:1;goto done;}
  struct stat final={0};if(fstat(fd,&final)||!file_same_stat(&initial,&final)){fprintf(stderr,"input file changed during recognition\n");goto done;}
  if(options.format==3) {
    if(!file_emit(spool,pkfile_json_middle((coax_v_u8){output,FILE_OUTPUT}),output)||!file_copy(text_spool,spool,output)||!file_emit(spool,pkfile_json_tail(total,info.feature_frames,info.encoded_frames,(coax_v_s64){tokens,count},(coax_v_s64){frames,count},(coax_v_u8){output,FILE_OUTPUT}),output)){fprintf(stderr,"cannot finalize JSON output\n");goto done;}
  }
  if(file_cancelled){status=130;goto done;}
  published=1;
  if(!file_publish(spool,output)||file_cancelled){fprintf(stderr,"transcript output I/O failed; output may be partial\n");goto done;}
  status=0;
 done:
  if(file_cancelled)status=130;
  if(status==130&&published)close(STDOUT_FILENO);
  if(status==130)fprintf(stderr,"%s\n",published?"transcription cancelled during output; output may be partial":"transcription cancelled; no transcript published");
  if(engine)psionic_parakeet_close(engine);
  if(spool)fclose(spool);if(text_spool)fclose(text_spool);
  file_wipe(samples,FILE_SAMPLES*sizeof *samples);free(samples);free(table);free(output);
  file_wipe(text,sizeof text);file_wipe(tokens,sizeof tokens);file_wipe(frames,sizeof frames);
  if(mapped!=MAP_FAILED)munmap(mapped,bytes);if(fd>=0)close(fd);return status;
}
