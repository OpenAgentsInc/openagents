// Firm reason: 1 (PipeWire SDK: negotiate an actual device stream and copy Coax PCM).
#include "audio-host.h"
#include <pipewire/pipewire.h>
#include <spa/param/audio/format-utils.h>
#include <string.h>
#include <stdlib.h>
struct RhAudioDevice {
 RhAudio *audio;struct pw_thread_loop *loop;struct pw_stream *stream;
 bool muted,started,paused,active;int state,format_ok,node_id;
 int64_t offers,callbacks,frames,failures,nonzero;uint32_t rate,channels,format;
};
static void state_changed(void *data,enum pw_stream_state old,enum pw_stream_state state,const char *error){
 (void)old;(void)error;RhAudioDevice *d=data;d->state=state;if(state==PW_STREAM_STATE_ERROR)d->failures++;
}
static void param_changed(void *data,uint32_t id,const struct spa_pod *param){
 RhAudioDevice *d=data;if(id!=SPA_PARAM_Format||!param)return;struct spa_audio_info_raw info={0};
 if(spa_format_audio_raw_parse(param,&info)<0){d->format_ok=0;d->failures++;return;}
 d->rate=info.rate;d->channels=info.channels;d->format=info.format;d->format_ok=info.rate==48000&&info.channels==2&&info.format==SPA_AUDIO_FORMAT_F32;
 if(!d->format_ok)d->failures++;
}
static void process(void *data){
 RhAudioDevice *d=data;d->offers++;struct pw_buffer *buffer=pw_stream_dequeue_buffer(d->stream);if(!buffer)return;
 struct spa_buffer *b=buffer->buffer;
 if(!b->n_datas||!b->datas[0].data||!b->datas[0].chunk){d->failures++;pw_stream_queue_buffer(d->stream,buffer);return;}
 struct spa_data *plane=&b->datas[0];size_t frames=plane->maxsize/(2*sizeof(float));if(buffer->requested&&frames>buffer->requested)frames=buffer->requested;
 if(!d->active||!d->format_ok){memset(plane->data,0,frames*2*sizeof(float));}
 else if(rha_render(d->audio,plane->data,frames,d->muted)){d->callbacks++;d->frames+=(int64_t)frames;}
 else d->failures++;
 if(d->muted){const float *actual=plane->data;for(size_t i=0;i<frames*2;i++)if(actual[i]!=0)d->nonzero++;}
 plane->chunk->offset=0;plane->chunk->stride=2*sizeof(float);plane->chunk->size=(uint32_t)(frames*2*sizeof(float));buffer->size=frames;
 pw_stream_queue_buffer(d->stream,buffer);
}
static const struct pw_stream_events events={PW_VERSION_STREAM_EVENTS,.state_changed=state_changed,.param_changed=param_changed,.process=process};
RhAudioDevice *rhap_open(RhAudio *audio,bool muted){
 if(!audio)return NULL;
 pw_init(NULL,NULL);RhAudioDevice *d=calloc(1,sizeof*d);if(!d){pw_deinit();return NULL;}d->audio=audio;d->muted=muted;d->paused=false;d->node_id=-1;
 d->loop=pw_thread_loop_new("Aethyr Coax audio",NULL);if(!d->loop){free(d);pw_deinit();return NULL;}
 // This locked callback deliberately does not request RT_PROCESS. Do not
 // load the unused realtime/RTKit scheduling module into this context.
 struct pw_properties *props=pw_properties_new(PW_KEY_MEDIA_TYPE,"Audio",PW_KEY_MEDIA_CATEGORY,"Playback",PW_KEY_MEDIA_ROLE,"Game",PW_KEY_NODE_NAME,"aethyr-host-audio",PW_KEY_NODE_LATENCY,"1024/48000","module.rt","false",NULL);
 d->stream=pw_stream_new_simple(pw_thread_loop_get_loop(d->loop),"Aethyr Coax audio",props,&events,d);
 if(!d->stream){rhap_close(d);return NULL;}
 uint8_t pod[1024];struct spa_pod_builder builder=SPA_POD_BUILDER_INIT(pod,sizeof pod);
 struct spa_audio_info_raw info={.format=SPA_AUDIO_FORMAT_F32,.rate=48000,.channels=2,.position={SPA_AUDIO_CHANNEL_FL,SPA_AUDIO_CHANNEL_FR}};
 const struct spa_pod *params[]={spa_format_audio_raw_build(&builder,SPA_PARAM_EnumFormat,&info)};
 int status=pw_stream_connect(d->stream,PW_DIRECTION_OUTPUT,PW_ID_ANY,PW_STREAM_FLAG_AUTOCONNECT|PW_STREAM_FLAG_MAP_BUFFERS|PW_STREAM_FLAG_INACTIVE,params,1);
 if(status<0||pw_thread_loop_start(d->loop)<0){rhap_close(d);return NULL;}return d;
}
bool rhap_start(RhAudioDevice *d){
 if(!d)return false;
 pw_thread_loop_lock(d->loop);d->started=true;d->active=!d->paused;int status=pw_stream_set_active(d->stream,d->active);pw_thread_loop_unlock(d->loop);return status>=0;
}
bool rhap_pause(RhAudioDevice *d,bool paused){
 if(!d)return false;
 pw_thread_loop_lock(d->loop);d->paused=paused;d->active=d->started&&!paused;int status=pw_stream_set_active(d->stream,d->active);pw_thread_loop_unlock(d->loop);return status>=0;
}
int64_t rhap_info(RhAudioDevice *d,int field){
 if(!d)return -1;
 pw_thread_loop_lock(d->loop);int64_t result=-1;switch(field){case 0:result=d->state;break;case 1:result=d->format_ok;break;case 2:result=d->offers;break;case 3:result=d->callbacks;break;case 4:result=d->frames;break;case 5:result=d->failures;break;case 6:result=d->muted;break;case 7:result=d->rate;break;case 8:result=d->channels;break;case 9:{uint32_t node=pw_stream_get_node_id(d->stream);result=node==PW_ID_ANY?0:node;break;}case 10:result=d->nonzero;break;}pw_thread_loop_unlock(d->loop);return result;
}
void rhap_close(RhAudioDevice *d){
 if(!d)return;
 if(d->loop){pw_thread_loop_lock(d->loop);if(d->stream)pw_stream_destroy(d->stream);pw_thread_loop_unlock(d->loop);pw_thread_loop_stop(d->loop);pw_thread_loop_destroy(d->loop);}
 free(d);pw_deinit();
}
