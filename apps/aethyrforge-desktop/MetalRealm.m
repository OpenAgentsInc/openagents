// Firm reason: 6 (Metal, dynamic-library and platform effect calls only).
// Coax admits products, spans, resources, pipeline descriptions and every draw.
#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#import <ImageIO/ImageIO.h>
#include <dlfcn.h>
#include <errno.h>
#include "metal-host.h"
#include "metal-producer.h"
#include "metal-sockets.h"
#include "MetalRender.h"
#include "RenderTexture.h"
#include "../../aethyr/render/tests/frame-input.h"

typedef struct {int64_t slot,generation;} MFHandle;
@interface MFBuffer:NSObject {
@public char name[32];coax_t_RenderBindingSource source;size_t capacity;
 NSMutableData *retained;NSArray<id<MTLBuffer>> *gpu;MFHandle handles[2];
 uint32_t version;BOOL observed;coax_t_RenderHostUploadState upload;
}
@end
@implementation MFBuffer @end
@interface MFMedia:NSObject {@public id<MTLTexture> texture;MFHandle handle;NSInteger width,height;NSData *rgba;BOOL failed;}
@end
@implementation MFMedia @end

@interface MFRealm:NSObject {
@public MFProducer face;void *image;uintptr_t state;
 NSMutableData *stateBytes,*vertices,*uniforms;
 NSString *directory,*error,*pageOrigin,*localOrigin;BOOL fixture,golden,still,closed,replay,wantsRecovery,clockedFrame,slotBusy[2];int slot,pending;
 uint64_t frames,draws,completedFrames,lastGpuNS,gpuEpoch,recoveries;uint32_t lastHud;int64_t recoveryReleasedLive;NSMutableArray<NSDictionary*> *effects;NSData *lastEffect;
 id<MTLDevice> device;id<MTLCommandQueue> queue;CAMetalLayer *layer;
 BOOL presentationExperiment,presentationQuantized;id<MTLTexture> presentedTexture;MFHandle presentedHandle;
 NSMutableArray<id<MTLCommandBuffer>> *inflight;
 NSMutableDictionary<NSNumber*,id<MTLTexture>> *targets;
 NSMutableDictionary<NSNumber*,NSValue*> *targetHandles;
 NSMutableDictionary<NSString*,id<MTLRenderPipelineState>> *pipelines;
 NSMutableDictionary<NSString*,id<MTLDepthStencilState>> *depthStates;
 NSMutableDictionary<NSString*,id<MTLSamplerState>> *samplers;
 NSMutableArray<MFBuffer*> *buffers;id<MTLBuffer> indices;MFHandle indexHandle;
 NSArray<id<MTLBuffer>> *hud;MFHandle hudHandles[2];NSData *packBytes,*packageBytes,*metalBytes;
 NSMutableDictionary<NSString*,MFMedia*> *mediaCache;NSData *mediaNotice,*lastMediaNotice;
 NSMutableArray<NSData*> *reflectionBytes;NSMutableDictionary<NSNumber*,id<MTLLibrary>> *libraries;
 NSMutableDictionary<NSNumber*,NSData*> *shaderSources;coax_t_HostLifecycle life;int64_t frameTime;
 coax_t_RenderHostPackageState packageState;coax_t_RenderHostEntryBank entryBank;
 coax_t_MetalPackage metal;coax_t_MetalHostInterface interfaces[14];
 coax_t_RenderHostRegistryBank registryBank;coax_t_RenderHostNamedBuffer named[48];
 RfiFrame textureInput;NSMutableArray<id<MTLTexture>> *textureImages;MFHandle textureHandles[48];
 coax_t_RenderHostNamedTexture textureNames[48];
 coax_t_PackRead pack;coax_t_RenderTextureFormat screenFormat;
 coax_t_HostTransport transport;coax_t_HostOwner socketOwners[4];
 void *socketContext;
 NSMutableData *measureRows;NSData *frameImage;uint64_t measureEpoch,inputSequence,inputNS;size_t measureDropped;
}
- (BOOL)fail:(NSString*)message;
- (BOOL)open:(NSString*)resources producer:(NSString*)producer golden:(BOOL)isGolden fixture:(BOOL)isFixture;
- (BOOL)render:(float)width height:(float)height scale:(float)scale dt:(float)dt;
- (BOOL)ensureTarget:(coax_t_RenderTarget)target width:(int64_t)width height:(int64_t)height;
- (id<MTLRenderPipelineState>)pipeline:(int)entry;
- (BOOL)bind:(id<MTLRenderCommandEncoder>)encoder entry:(int)entry op:(coax_t_RenderOp)op media:(const coax_t_RenderMediaRect*)media images:(NSDictionary<NSNumber*,MFMedia*>*)images;
- (void)message:(int32_t)kind data:(NSData*)data;
- (void)deliverMediaNotice;
- (void)shutdown;
- (BOOL)createTextures;
- (BOOL)loadReplay:(const char*)path;
- (void)releaseGPU;
- (BOOL)compileLibraries;
- (BOOL)recoverGPU;
- (BOOL)present:(id<MTLCommandBuffer>)command drawable:(id<CAMetalDrawable>)drawable width:(int64_t)width height:(int64_t)height;
- (BOOL)present:(id<MTLCommandBuffer>)command drawable:(id<CAMetalDrawable>)drawable descriptor:(coax_t_RenderPresentation)p;
@end
static __weak MFRealm *activeRealm;
static NSString *lastFailure;
static int64_t lastClosedLive=-1,lastClosedWords=-1;
typedef struct {
 uint32_t(*version)(void);bool(*uri)(const uint8_t*,size_t);bool(*poll_more)(int64_t,int64_t,int64_t);
 void*(*open)(int64_t);bool(*start)(void*,MNSocketOwner,const uint8_t*,size_t);bool(*send)(void*,MNSocketOwner,const uint8_t*,size_t);
 void(*socket_close)(void*,MNSocketOwner);bool(*next)(void*,MNSocketEvent*);void(*done)(void*);void(*close)(void*);int64_t(*info)(void*,int32_t);
} MFNetworkAPI;
static MFNetworkAPI networkAPI;static void *networkImage;
// The SDK image and its fixed idle worker pool are process-owned. Contexts
// retire independently; unloading executable code while workers stop is unsafe.
static BOOL socketSDK(MFRealm *r){
 if(r->socketContext)return YES;
 if(!networkImage){NSString *path=[r->directory stringByAppendingPathComponent:@"metal/libnetwork.dylib"];void *image=dlopen(path.fileSystemRepresentation,RTLD_NOW|RTLD_LOCAL);if(!image)return [r fail:@"Missing packaged native socket SDK"];
#define MN_LOAD(field,name) networkAPI.field=(__typeof__(networkAPI.field))dlsym(image,"mn_"name);if(!networkAPI.field){dlclose(image);memset(&networkAPI,0,sizeof networkAPI);return [r fail:@"Native socket SDK ABI is incomplete"];}
  MN_LOAD(version,"api_version") MN_LOAD(uri,"uri") MN_LOAD(poll_more,"poll_more") MN_LOAD(open,"open") MN_LOAD(start,"start") MN_LOAD(send,"send") MN_LOAD(socket_close,"socket_close") MN_LOAD(next,"next") MN_LOAD(done,"done") MN_LOAD(close,"close") MN_LOAD(info,"info")
#undef MN_LOAD
  if(networkAPI.version()!=1){dlclose(image);memset(&networkAPI,0,sizeof networkAPI);return [r fail:@"Native socket SDK ABI differs"];}networkImage=image;
 }
 r->socketContext=networkAPI.open(r->face.inbox_cap());return r->socketContext?YES:[r fail:@"Native socket storage is unavailable"];
}
static coax_v_t_HostTransport net(MFRealm *r){return(coax_v_t_HostTransport){&r->transport,1};}
static int64_t milliseconds(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return(int64_t)t.tv_sec*1000+t.tv_nsec/1000000;}
static uint64_t nanoseconds(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return(uint64_t)t.tv_sec*1000000000+(uint64_t)t.tv_nsec;}
static NSDictionary *ownerFields(coax_t_HostOwner o){return @{@"id":@(o.id),@"generation":@(o.generation),@"engine":@(o.engine)};}
static void effect(MFRealm *r,NSString *kind,coax_t_HostOwner owner,NSDictionary *fields){NSMutableDictionary *d=[ownerFields(owner)mutableCopy];d[@"kind"]=kind;[d addEntriesFromDictionary:fields];[r->effects addObject:d];}
static coax_string cs(NSString *s){return(coax_string){s.UTF8String,(int64_t)[s lengthOfBytesUsingEncoding:NSUTF8StringEncoding]};}
static coax_v_u8 cv(NSData *d){return(coax_v_u8){(uint8_t*)d.bytes,(int64_t)d.length};}
static coax_v_t_RenderRegistry ledger(MFRealm *r){return aethyr_render_registry_storage((coax_v_t_RenderHostRegistryBank){&r->registryBank,1});}
static BOOL acquire(MFRealm *r,int kind,int64_t words,MFHandle *h){coax_t_RrGrant g=aethyr_render_registry_acquire(ledger(r),aethyr_render_resource_kind(kind),words);if(g.status!=COAX_RrStatus_ok)return NO;*h=(MFHandle){g.handle.slot,g.handle.generation};return YES;}
static void releaseHandle(MFRealm *r,MFHandle *h){if(h->generation)aethyr_render_registry_release(ledger(r),h->slot,h->generation);*h=(MFHandle){0};}
static BOOL live(MFRealm *r,MFHandle h){return aethyr_render_registry_live(ledger(r),h.slot,h.generation);}
static NSString *text(coax_string s){return [[NSString alloc]initWithBytes:s.data length:(NSUInteger)s.count encoding:NSUTF8StringEncoding];}
static NSData *file(NSString *dir,coax_string name){return [NSData dataWithContentsOfFile:[dir stringByAppendingPathComponent:text(name)]];}
static BOOL same(coax_string a,coax_string b){return a.count==b.count&&!memcmp(a.data,b.data,(size_t)a.count);}
typedef coax_t_RenderPresentation(*MFPresentationPolicy)(int64_t,int64_t,int64_t,int64_t,int64_t);
static MFPresentationPolicy presentationPolicy(BOOL quantized){return quantized?(MFPresentationPolicy)dlsym(RTLD_DEFAULT,"render_presentation_quantized_v1"):render_presentation;}
static MTLPixelFormat pixel(coax_t_RenderTextureFormat f){switch(f){case COAX_RenderTextureFormat_rgba8:return MTLPixelFormatRGBA8Unorm;case COAX_RenderTextureFormat_bgra8:return MTLPixelFormatBGRA8Unorm;case COAX_RenderTextureFormat_rgba16f:return MTLPixelFormatRGBA16Float;case COAX_RenderTextureFormat_depth32:return MTLPixelFormatDepth32Float;default:return MTLPixelFormatInvalid;}}
static MTLPixelFormat texel(coax_t_RenderTexelFormat f){switch(f){case COAX_RenderTexelFormat_rgba8:return MTLPixelFormatRGBA8Unorm;case COAX_RenderTexelFormat_rgba8_srgb:return MTLPixelFormatRGBA8Unorm_sRGB;case COAX_RenderTexelFormat_bgra8:return MTLPixelFormatBGRA8Unorm;case COAX_RenderTexelFormat_bgra8_srgb:return MTLPixelFormatBGRA8Unorm_sRGB;case COAX_RenderTexelFormat_rgba16f:return MTLPixelFormatRGBA16Float;case COAX_RenderTexelFormat_rgba32f:return MTLPixelFormatRGBA32Float;case COAX_RenderTexelFormat_depth32f:return MTLPixelFormatDepth32Float;default:return MTLPixelFormatInvalid;}}
static coax_t_RenderHostWordView wordView(const void *p,size_t capacity,size_t bytes){uint64_t a=(uintptr_t)p;return aethyr_render_words(a,a,capacity,bytes);}
static coax_v_u32 words(const void *p,size_t capacity,size_t bytes){coax_t_RenderHostWordView v=wordView(p,capacity,bytes);return v.status==COAX_RenderFrameStatus_ok?(coax_v_u32){(uint32_t*)(uintptr_t)v.address,v.words}:(coax_v_u32){0};}
static coax_t_RenderUpload upload(const void *p,size_t capacity,size_t bytes,size_t gpu){coax_t_RenderUpload u;coax_default_t_RenderUpload(&u);u.source_address=(uintptr_t)p;u.source_base=u.source_address;u.source_bytes=capacity;u.bytes=bytes;u.gpu_bytes=gpu;return u;}
static BOOL parseNumber(const char *s,int64_t *out){if(!s||!*s)return NO;errno=0;char *end;long long n=strtoll(s,&end,10);if(errno||*end)return NO;*out=n;return YES;}
static BOOL reflection(MFRealm *r,int entry,NSData *raw){
 NSMutableData *owned=[raw mutableCopy];[owned appendBytes:"\0" length:1];[r->reflectionBytes addObject:owned];
 char *cursor=owned.mutableBytes,*line;int header=0,n=0,na=0;int64_t stride=0;coax_string vertex={0},fragment={0};
 coax_t_MetalHostUse uses[32]={0};coax_t_RenderVertexInput attrs[32]={0};
 while((line=strsep(&cursor,"\n"))){if(!*line)continue;
  char *parts[9]={0},*p=line;int count=0;while(p){char *q=strsep(&p," \t\r");if(!*q)continue;if(count==9)return NO;parts[count++]=q;}
  if(!count)continue;
  if(header<3){const char *rows[]={"metal-reflection","lengths-buffer","vertex-buffer"};const char *values[]={"1","29","30"};if(count!=2||strcmp(parts[0],rows[header])||strcmp(parts[1],values[header]))return NO;header++;continue;}
  if(!strcmp(parts[0],"entry")){if(count!=3)return NO;coax_string name={parts[2],(int64_t)strlen(parts[2])};if(!strcmp(parts[1],"vertex")&&!vertex.count)vertex=name;else if(!strcmp(parts[1],"fragment")&&!fragment.count)fragment=name;else return NO;}
  else if(!strcmp(parts[0],"resource")){if(count!=7||n==32)return NO;int kind=!strcmp(parts[1],"buffer")?1:!strcmp(parts[1],"texture")?2:!strcmp(parts[1],"sampler")?3:0;int64_t a[4];for(int i=0;i<4;i++)if(!parseNumber(parts[2+i],&a[i]))return NO;
   uses[n++]=metal_host_use(2,entry,(coax_string){parts[6],(int64_t)strlen(parts[6])},kind,a[0],a[1],a[2],a[3]);}
  else if(!strcmp(parts[0],"attribute")){if(count!=7||na==32||!same(vertex,(coax_string){parts[1],(int64_t)strlen(parts[1])}))return NO;int64_t a[5];for(int i=0;i<5;i++)if(!parseNumber(parts[2+i],&a[i]))return NO;
   if(na&&stride!=a[4])return NO;attrs[na++]=(coax_t_RenderVertexInput){a[0],a[1],a[2],a[3]};stride=a[4];}
  else return NO;
 }
 r->interfaces[entry]=metal_host_interface(2,entry,vertex,fragment,(coax_v_t_MetalHostUse){uses,n},(coax_v_t_RenderVertexInput){attrs,na},stride);
 return header==3&&r->interfaces[entry].valid;
}
static void blend(MTLRenderPipelineColorAttachmentDescriptor *a,coax_t_RenderBlend c,coax_t_RenderBlend alpha){
 MTLBlendFactor src[]={MTLBlendFactorOne,MTLBlendFactorOne,MTLBlendFactorZero,MTLBlendFactorOne,MTLBlendFactorZero,MTLBlendFactorOne,MTLBlendFactorZero};
 MTLBlendFactor dst[]={MTLBlendFactorZero,MTLBlendFactorOne,MTLBlendFactorSourceColor,MTLBlendFactorOne,MTLBlendFactorOne,MTLBlendFactorOneMinusSourceAlpha,MTLBlendFactorOneMinusSourceAlpha};
 a.blendingEnabled=c!=COAX_RenderBlend_replace||alpha!=COAX_RenderBlend_replace;a.rgbBlendOperation=c==COAX_RenderBlend_maximum?MTLBlendOperationMax:MTLBlendOperationAdd;a.alphaBlendOperation=alpha==COAX_RenderBlend_maximum?MTLBlendOperationMax:MTLBlendOperationAdd;
 a.sourceRGBBlendFactor=src[c];a.destinationRGBBlendFactor=dst[c];a.sourceAlphaBlendFactor=src[alpha];a.destinationAlphaBlendFactor=dst[alpha];
}

@implementation MFRealm
- (BOOL)fail:(NSString*)message {error=message?:@"Metal effect failed";return NO;}
- (BOOL)open:(NSString*)resources producer:(NSString*)producer golden:(BOOL)isGolden fixture:(BOOL)isFixture {
 directory=resources;golden=isGolden;fixture=isFixture;effects=[NSMutableArray array];inflight=[NSMutableArray array];buffers=[NSMutableArray array];targets=[NSMutableDictionary dictionary];targetHandles=[NSMutableDictionary dictionary];pipelines=[NSMutableDictionary dictionary];depthStates=[NSMutableDictionary dictionary];samplers=[NSMutableDictionary dictionary];libraries=[NSMutableDictionary dictionary];reflectionBytes=[NSMutableArray array];textureImages=[NSMutableArray array];mediaCache=[NSMutableDictionary dictionary];screenFormat=COAX_RenderTextureFormat_rgba16f;
 image=dlopen(producer.fileSystemRepresentation,RTLD_NOW|RTLD_LOCAL);if(!image)return [self fail:[NSString stringWithUTF8String:dlerror()]];face.image=image;
 coax_default_t_HostTransport(&transport);
 coax_default_t_HostLifecycle(&life);frameTime=-1;
#define MF_LOAD(result,name,args) face.name=(result(*)args)dlsym(image,"coaxial_"#name);
 MF_FACE(MF_LOAD)
#undef MF_LOAD
 if(!face.state_size||!face.state_init||!face.frame||!face.max_vertices||!face.vertex_floats||!face.uniform_floats||!face.inbox||!face.inbox_cap||!face.message)return [self fail:@"Producer is missing the realm face ABI"];
 stateBytes=[NSMutableData dataWithLength:face.state_size()];vertices=[NSMutableData dataWithLength:(size_t)face.max_vertices()*face.vertex_floats()*4];uniforms=[NSMutableData dataWithLength:(size_t)face.uniform_floats()*4];state=(uintptr_t)stateBytes.mutableBytes;face.state_init(state);if(golden&&face.golden)face.golden(1);
 if(fixture)return YES;
 device=MTLCreateSystemDefaultDevice();queue=[device newCommandQueue];if(!device||!queue)return [self fail:@"No native Metal device"];
 aethyr_render_registry_open(ledger(self));
 NSString *render=[resources stringByAppendingPathComponent:@"metal/render"];
 packageBytes=[NSData dataWithContentsOfFile:[render stringByAppendingPathComponent:@"render.json"]];metalBytes=[NSData dataWithContentsOfFile:[render stringByAppendingPathComponent:@"metal.json"]];
 if(!packageBytes||!metalBytes)return [self fail:@"Missing packaged Metal products"];
 coax_default_t_RenderHostPackageState(&packageState);coax_t_RenderDeviceCapabilities caps=aethyr_render_device(COAX_RenderBackend_metal,true,true);
 if(aethyr_render_package_load((coax_v_t_RenderHostPackageState){&packageState,1},(coax_string){packageBytes.bytes,(int64_t)packageBytes.length},aethyr_render_entry_storage((coax_v_t_RenderHostEntryBank){&entryBank,1}),2,caps,0)!=COAX_RenderPackageStatus_ok)return [self fail:@"Coax refused the portable shader package"];
 metal=metal_package_parse((coax_string){metalBytes.bytes,(int64_t)metalBytes.length},packageState.package,cv(packageBytes));if(metal.status!=COAX_RenderPackageStatus_ok)return [self fail:@"Coax refused the Metal product manifest"];
 // Authenticate all files and interfaces before any GPU shader is created.
 shaderSources=[NSMutableDictionary dictionary];
 for(int i=0;i<14;i++)if(metal.entries.e[i].present){coax_t_MetalPackageEntry e=metal.entries.e[i];NSData *source=file(render,e.file),*meta=file(render,e.reflection_file);
  if(!source||!meta||metal_package_product(metal,i,0,cv(source))!=COAX_RenderPackageStatus_ok||metal_package_product(metal,i,1,cv(meta))!=COAX_RenderPackageStatus_ok||!reflection(self,i,meta))return [self fail:@"Coax refused Metal bytes or their actual compiler interface"];shaderSources[@(i)]=source;}
 if(![self compileLibraries])return NO;
 packBytes=[NSData dataWithContentsOfFile:[resources stringByAppendingPathComponent:@"web/realms/everglade/assets/everglade.pack"]];
 if(!packBytes||!metal_pack_read((coax_string){packBytes.bytes,(int64_t)packBytes.length},(coax_v_t_PackRead){&pack,1}))return [self fail:@"Coax refused the asset pack"];
 int layers=0;for(int i=0;i<pack.count;i++)if(pack.entries.e[i].kind==4)layers++;
 [self message:605 data:[[NSString stringWithFormat:@"%lu %lu",(unsigned long)packBytes.length,(unsigned long)packBytes.length]dataUsingEncoding:NSUTF8StringEncoding]];
 for(int part=1;part<=3;part++)for(int i=0;i<pack.count;i++)if(pack.entries.e[i].kind==part){coax_t_PackEntry e=pack.entries.e[i];if(part==1){if(!mf_asset((__bridge void*)self,0,(const uint8_t*)packBytes.bytes+e.at,(size_t)e.n))return [self fail:@"Coax refused the catalog allocation"];}else face.asset_ready(state,part-1,(uint32_t)e.n);}
 face.asset_ready(state,3,layers);face.asset_ready(state,4,1);
 coax_t_RenderHostBufferSizes sizes=aethyr_render_buffer_sizes(2,face.max_vertices(),face.vertex_floats(),face.uniform_floats(),device.maxBufferLength);if(sizes.status!=COAX_RenderFrameStatus_ok)return [self fail:@"Coax refused the producer buffer extents"];
 NSMutableArray *huds=[NSMutableArray array];for(int i=0;i<2;i++){if(!acquire(self,3,aethyr_render_buffer_words(sizes.vertex_bytes),&hudHandles[i]))return [self fail:@"HUD ledger capacity"];id<MTLBuffer> b=[device newBufferWithLength:sizes.vertex_bytes options:MTLResourceStorageModeShared];if(!b)return [self fail:@"HUD allocation"];[huds addObject:b];}hud=huds;
 for(int i=0;i<pack.count;i++)if(pack.entries.e[i].kind==3){coax_t_PackEntry e=pack.entries.e[i];if(!acquire(self,4,aethyr_render_buffer_words(e.n),&indexHandle))return [self fail:@"Index ledger capacity"];indices=[device newBufferWithBytes:(const uint8_t*)packBytes.bytes+e.at length:(NSUInteger)e.n options:MTLResourceStorageModeShared];}
 for(int set=0;set<2;set++)for(int resource=0;resource<aethyr_render_binding_count(2,set);resource++){
  coax_t_RenderHostBinding b=aethyr_render_binding(2,set,resource);uint8_t scratch[32];coax_string name=aethyr_render_buffer_name(b.source,(coax_v_u8){scratch,32});if(!name.count)continue;BOOL exists=NO;for(MFBuffer *old in buffers)if(!memcmp(old->name,name.data,name.count)&&old->name[name.count]==0)exists=YES;if(exists)continue;
  MFBuffer *row=[MFBuffer new];row->source=b.source;memcpy(row->name,name.data,name.count);
  if(b.source.kind==COAX_RenderSourceKind_main_uniform)row->capacity=uniforms.length;
  else if(b.source.kind==COAX_RenderSourceKind_dynamic_store)row->capacity=face.store_cap((int32_t)b.source.index);
  else if(b.source.kind==COAX_RenderSourceKind_particle_emitters)row->capacity=face.particle_cap();
  else if(b.source.kind==COAX_RenderSourceKind_immutable_storage_asset){for(int i=0;i<pack.count;i++)if(pack.entries.e[i].kind==2)row->capacity=(size_t)pack.entries.e[i].n;}
  else return [self fail:@"Unobserved buffer origin"];
  coax_t_RenderBufferView allocation=aethyr_render_buffer_bytes(row->capacity,device.maxBufferLength);if(allocation.status!=COAX_RenderResourceStatus_ok)return [self fail:@"Coax refused a physical buffer extent"];
  row->retained=[NSMutableData dataWithLength:(NSUInteger)allocation.bytes];NSMutableArray *pair=[NSMutableArray array];
  if(b.source.kind==COAX_RenderSourceKind_immutable_storage_asset)for(int i=0;i<pack.count;i++)if(pack.entries.e[i].kind==2)memcpy(row->retained.mutableBytes,(const uint8_t*)packBytes.bytes+pack.entries.e[i].at,row->capacity);
  for(int i=0;i<2;i++){if(!acquire(self,b.source.kind==COAX_RenderSourceKind_main_uniform?0:1,aethyr_render_buffer_words(allocation.bytes),&row->handles[i]))return [self fail:@"Buffer ledger capacity"];id<MTLBuffer> buffer=[device newBufferWithLength:(NSUInteger)allocation.bytes options:MTLResourceStorageModeShared];if(!buffer)return [self fail:@"Buffer allocation"];memset(buffer.contents,0,buffer.length);[pair addObject:buffer];}row->gpu=pair;[buffers addObject:row];
 }
 NSString *textures=[resources stringByAppendingPathComponent:@"metal/assets/textures.config"];char why[256];if(!rfi_load_textures(textures.fileSystemRepresentation,&textureInput,why,sizeof why))return [self fail:[NSString stringWithUTF8String:why]];
 return [self createTextures];
}
- (BOOL)createTextures {
 if(textureInput.texture_count>48)return [self fail:@"Texture observation capacity"];
 for(size_t i=0;i<textureInput.texture_count;i++){RfiTexture *t=&textureInput.textures[i];coax_t_RenderTextureLevel levels[32];if(t->level_count>32)return [self fail:@"Texture level capacity"];
  for(size_t k=0;k<t->level_count;k++){RfiLevel l=t->levels[k];levels[k]=(coax_t_RenderTextureLevel){l.width,l.height,l.offset,l.row_bytes,l.layer_bytes};}
  coax_string format={t->format,(int64_t)strlen(t->format)};coax_t_RFTextureNeed need=aethyr_native_texture_require(format,t->layers,(coax_v_t_RenderTextureLevel){levels,(int64_t)t->level_count});if(need.status!=COAX_RenderTextureStatus_ok)return [self fail:@"Coax refused texture layout"];
  coax_t_RQColor *colours=calloc((size_t)need.pixels,sizeof*colours);coax_t_RQMip *mips=calloc(t->level_count,sizeof*mips);coax_t_RFTextureDecoded decoded=aethyr_native_texture_decode(format,t->layers,(coax_v_t_RenderTextureLevel){levels,(int64_t)t->level_count},(coax_v_u8){t->bytes.data,(int64_t)t->bytes.size},(coax_v_t_RQColor){colours,need.pixels},(coax_v_t_RQMip){mips,(int64_t)t->level_count});free(colours);free(mips);if(decoded.status!=COAX_RenderTextureStatus_ok)return [self fail:@"Coax refused texture bytes"];
  textureNames[i]=(coax_t_RenderHostNamedTexture){.name={t->name,(int64_t)strlen(t->name)},.layers=t->layers,.depth=decoded.texture.depth};
  coax_t_RenderHostTextureReceipt binding={0};BOOL found=NO;for(int set=0;set<3&&!found;set++)for(int resource=0;resource<aethyr_render_binding_count(2,set)&&!found;resource++)for(int context=-1;context<(set==2?112:0)&&!found;context++){coax_t_RenderHostTextureReceipt q=aethyr_render_resolve_texture(2,set,resource,(coax_v_t_RenderHostNamedTexture){&textureNames[i],1},context);if(q.status==COAX_RenderResourceStatus_ok&&q.named_index==0){binding=q;found=YES;}}
  if(!found)return [self fail:@"Coax could not bind an observed texture"];
  MTLTextureDescriptor *d=[MTLTextureDescriptor new];d.textureType=binding.layout.dimension==COAX_RenderTextureDimension_d2_array?MTLTextureType2DArray:MTLTextureType2D;d.pixelFormat=texel(need.format);d.width=(NSUInteger)levels[0].width;d.height=(NSUInteger)levels[0].height;d.arrayLength=(NSUInteger)t->layers;d.mipmapLevelCount=t->level_count;d.usage=MTLTextureUsageShaderRead;d.storageMode=MTLStorageModeShared;
  if(!acquire(self,5,aethyr_render_texture_words(d.width,d.height,t->layers,t->level_count,aethyr_native_texel_bytes(need.format)),&textureHandles[i]))return [self fail:@"Texture ledger capacity"];
  id<MTLTexture> texture=[device newTextureWithDescriptor:d];if(!texture)return [self fail:@"Texture allocation"];
  for(size_t k=0;k<t->level_count;k++)for(int64_t s=0;s<t->layers;s++){RfiLevel l=t->levels[k];[texture replaceRegion:MTLRegionMake2D(0,0,(NSUInteger)l.width,(NSUInteger)l.height) mipmapLevel:k slice:(NSUInteger)s withBytes:t->bytes.data+l.offset+(uint64_t)s*l.layer_bytes bytesPerRow:l.row_bytes bytesPerImage:l.layer_bytes];}[textureImages addObject:texture];
 }
 return YES;
}
- (BOOL)compileLibraries {
 MTLCompileOptions *options=[MTLCompileOptions new];options.languageVersion=MTLLanguageVersion3_2;options.mathMode=MTLMathModeSafe;options.mathFloatingPointFunctions=MTLMathFloatingPointFunctionsPrecise;
 for(NSNumber *i in shaderSources){NSError *failure=nil;NSString *source=[[NSString alloc]initWithData:shaderSources[i] encoding:NSUTF8StringEncoding];id<MTLLibrary> library=[device newLibraryWithSource:source options:options error:&failure];if(!library)return [self fail:failure.localizedDescription];libraries[i]=library;}return YES;
}
- (BOOL)loadReplay:(const char*)path {
 if(fixture||frames||pending)return [self fail:@"Frozen input must be selected before the first GPU frame"];
 RfiFrame candidate={0};char why[256];if(!rfi_load(path,&candidate,why,sizeof why))return [self fail:[NSString stringWithUTF8String:why]];
 // The SDK observes bytes; the already admitted package owns their identity.
 if(candidate.package.size!=packageBytes.length||memcmp(candidate.package.data,packageBytes.bytes,packageBytes.length)){rfi_close(&candidate);return [self fail:@"Frozen frame and authenticated product package differ"];}
 for(size_t i=0;i<textureInput.texture_count;i++)releaseHandle(self,&textureHandles[i]);[textureImages removeAllObjects];rfi_close(&textureInput);textureInput=candidate;replay=YES;
 for(MFBuffer *b in buffers){RfiBuffer *raw=NULL;for(size_t i=0;i<textureInput.buffer_count;i++)if(!strcmp(textureInput.buffers[i].name,b->name))raw=&textureInput.buffers[i];if(!raw)return [self fail:@"Frozen frame is missing an actual buffer observation"];
  coax_t_RenderBufferView extent=aethyr_render_buffer_bytes(raw->bytes.size,device.maxBufferLength);if(extent.status!=COAX_RenderResourceStatus_ok)return [self fail:@"Coax refused frozen buffer allocation"];
  b->capacity=raw->source_capacity;b->retained=[NSMutableData dataWithLength:(NSUInteger)extent.bytes];if(raw->bytes.size)memcpy(b->retained.mutableBytes,raw->bytes.data,raw->bytes.size);NSMutableArray *pair=[NSMutableArray array];
  for(int i=0;i<2;i++){releaseHandle(self,&b->handles[i]);if(!acquire(self,b->source.kind==COAX_RenderSourceKind_main_uniform?0:1,aethyr_render_buffer_words(extent.bytes),&b->handles[i]))return [self fail:@"Frozen buffer ledger capacity"];id<MTLBuffer> gpu=[device newBufferWithLength:(NSUInteger)extent.bytes options:MTLResourceStorageModeShared];if(!gpu)return [self fail:@"Frozen buffer allocation"];[pair addObject:gpu];}b->gpu=pair;
 }
 coax_t_RenderUpload observations[3]={upload(textureInput.uniforms.data,textureInput.uniforms.size,textureInput.uniforms.size,uniforms.length),upload(textureInput.hud.data,textureInput.hud.size,textureInput.hud.size,vertices.length),upload(textureInput.indices.data,textureInput.indices.size,textureInput.indices.size,indices.length)};coax_t_RenderFrameLimits limits;coax_default_t_RenderFrameLimits(&limits);
 if(aethyr_native_uploads(limits,(coax_v_t_RenderUpload){observations,3}).status!=COAX_RenderFrameStatus_ok)return [self fail:@"Coax refused frozen producer allocation spans"];
 memcpy(uniforms.mutableBytes,textureInput.uniforms.data,textureInput.uniforms.size);memcpy(vertices.mutableBytes,textureInput.hud.data,textureInput.hud.size);memcpy(indices.contents,textureInput.indices.data,textureInput.indices.size);
 return [self createTextures];
}
- (void)message:(int32_t)kind data:(NSData*)data {size_t n=MIN(data.length,face.inbox_cap());memcpy((void*)face.inbox(state),data.bytes,n);face.message(state,kind,(uint32_t)n);}
- (void)deliverMediaNotice {if(mediaNotice){NSData *notice=mediaNotice;mediaNotice=nil;[self message:(int32_t)htn_kind() data:notice];lastMediaNotice=notice;}}
- (BOOL)ensureTarget:(coax_t_RenderTarget)t width:(int64_t)width height:(int64_t)height {
 if(t==COAX_RenderTarget_none)return YES;coax_t_RenderTargetDescriptor d=aethyr_render_target_raw(t,width,height,false,screenFormat,16384);if(d.status!=COAX_RenderResourceStatus_ok)return [self fail:@"Coax refused the target extent"];
 id<MTLTexture> old=targets[@(t)];if(old&&(old.width!=(NSUInteger)d.width||old.height!=(NSUInteger)d.height||old.pixelFormat!=pixel(d.format))){MFHandle h;[targetHandles[@(t)]getValue:&h size:sizeof h];releaseHandle(self,&h);[targets removeObjectForKey:@(t)];[targetHandles removeObjectForKey:@(t)];old=nil;}
 if(!old){MFHandle h;if(!acquire(self,d.format==COAX_RenderTextureFormat_depth32?5:2,aethyr_render_format_words(d.format,d.width,d.height,1,1),&h))return [self fail:@"Target ledger capacity"];
  MTLTextureDescriptor *descriptor=[MTLTextureDescriptor texture2DDescriptorWithPixelFormat:pixel(d.format) width:(NSUInteger)d.width height:(NSUInteger)d.height mipmapped:NO];descriptor.usage=MTLTextureUsageRenderTarget|MTLTextureUsageShaderRead;descriptor.storageMode=d.format==COAX_RenderTextureFormat_depth32?MTLStorageModePrivate:MTLStorageModeShared;
  old=[device newTextureWithDescriptor:descriptor];if(!old){releaseHandle(self,&h);return [self fail:@"Target allocation"];}targets[@(t)]=old;targetHandles[@(t)]=[NSValue valueWithBytes:&h objCType:@encode(MFHandle)];}
 MFHandle h;[targetHandles[@(t)]getValue:&h size:sizeof h];return live(self,h)||[self fail:@"Target is not live in the Coax ledger"];
}
- (id<MTLRenderPipelineState>)pipeline:(int)entry {
 NSString *key=[NSString stringWithFormat:@"%d:%d",entry,screenFormat];id<MTLRenderPipelineState> pipeline=pipelines[key];if(pipeline)return pipeline;
 coax_t_RenderHostPipeline p=aethyr_render_pipeline(2,entry);if(!p.valid){[self fail:@"Absent canonical pipeline"];return nil;}MTLRenderPipelineDescriptor *d=[MTLRenderPipelineDescriptor new];id<MTLLibrary> library=libraries[@(entry)];
 d.vertexFunction=[library newFunctionWithName:text(interfaces[entry].vertex)];d.fragmentFunction=[library newFunctionWithName:text(interfaces[entry].fragment)];
 coax_t_RenderTarget colours[2]={p.colour0,p.colour1};for(int i=0;i<2;i++)if(colours[i]!=COAX_RenderTarget_none){coax_t_RenderTargetDescriptor desc=aethyr_render_target_raw(colours[i],1,1,false,screenFormat,16384);d.colorAttachments[i].pixelFormat=pixel(desc.format);blend(d.colorAttachments[i],p.state.colour,p.state.alpha);if(i==1&&!p.state.aux_write)d.colorAttachments[i].writeMask=MTLColorWriteMaskNone;}
 if(p.depth!=COAX_RenderTarget_none)d.depthAttachmentPixelFormat=MTLPixelFormatDepth32Float;
 if(entry==COAX_RenderPassId_hud){MTLVertexDescriptor *v=[MTLVertexDescriptor vertexDescriptor];for(int i=0;i<4;i++){v.attributes[i].format=MTLVertexFormatFloat4;v.attributes[i].offset=i*16;v.attributes[i].bufferIndex=30;}v.layouts[30].stride=64;v.layouts[30].stepFunction=MTLVertexStepFunctionPerVertex;d.vertexDescriptor=v;}
 NSError *failure=nil;pipeline=[device newRenderPipelineStateWithDescriptor:d error:&failure];if(!pipeline){[self fail:failure.localizedDescription];return nil;}pipelines[key]=pipeline;return pipeline;
}
- (BOOL)present:(id<MTLCommandBuffer>)command drawable:(id<CAMetalDrawable>)drawable width:(int64_t)width height:(int64_t)height {
 MFPresentationPolicy policy=presentationPolicy(presentationQuantized);if(!policy)return [self fail:@"Private Coax presentation descriptor is absent"];
 coax_t_RenderPresentation p=policy(2,width,height,COAX_RenderTextureFormat_bgra8,16384);
 return [self present:command drawable:drawable descriptor:p];
}
- (BOOL)present:(id<MTLCommandBuffer>)command drawable:(id<CAMetalDrawable>)drawable descriptor:(coax_t_RenderPresentation)p {
 if(p.status!=COAX_RenderResourceStatus_ok)return [self fail:@"Coax refused experimental presentation"];
 id<MTLTexture> destination=drawable.texture;
 if(!destination){
  if(presentedTexture&&(presentedTexture.width!=(NSUInteger)p.display.width||presentedTexture.height!=(NSUInteger)p.display.height)){releaseHandle(self,&presentedHandle);presentedTexture=nil;}
  if(!presentedTexture){
   if(!acquire(self,2,aethyr_render_format_words(p.display.format,p.display.width,p.display.height,1,1),&presentedHandle))return [self fail:@"Presentation ledger capacity"];
   MTLTextureDescriptor *d=[MTLTextureDescriptor texture2DDescriptorWithPixelFormat:pixel(p.display.format) width:(NSUInteger)p.display.width height:(NSUInteger)p.display.height mipmapped:NO];d.usage=MTLTextureUsageRenderTarget|MTLTextureUsageShaderRead;d.storageMode=MTLStorageModeShared;presentedTexture=[device newTextureWithDescriptor:d];
   if(!presentedTexture)return [self fail:@"Presentation allocation"];
  }
  if(!live(self,presentedHandle))return [self fail:@"Presentation target is not live"];destination=presentedTexture;
 }
 NSString *key=[NSString stringWithFormat:@"presentation:%lld:%lld",(long long)p.entry,(long long)p.display.format];id<MTLRenderPipelineState> pipeline=pipelines[key];
 if(!pipeline){MTLRenderPipelineDescriptor *d=[MTLRenderPipelineDescriptor new];id<MTLLibrary> library=libraries[@(p.entry)];d.vertexFunction=[library newFunctionWithName:text(interfaces[p.entry].vertex)];d.fragmentFunction=[library newFunctionWithName:text(interfaces[p.entry].fragment)];d.colorAttachments[0].pixelFormat=pixel(p.display.format);blend(d.colorAttachments[0],p.state.colour,p.state.alpha);NSError *failure=nil;pipeline=[device newRenderPipelineStateWithDescriptor:d error:&failure];if(!pipeline)return [self fail:failure.localizedDescription];pipelines[key]=pipeline;}
 NSString *samplerKey=@"presentation";id<MTLSamplerState> sampler=samplers[samplerKey];if(!sampler){MTLSamplerDescriptor *d=[MTLSamplerDescriptor new];d.minFilter=p.sampler.min_linear?MTLSamplerMinMagFilterLinear:MTLSamplerMinMagFilterNearest;d.magFilter=p.sampler.mag_linear?MTLSamplerMinMagFilterLinear:MTLSamplerMinMagFilterNearest;d.mipFilter=p.sampler.mip_linear?MTLSamplerMipFilterLinear:MTLSamplerMipFilterNearest;d.sAddressMode=p.sampler.address_u==COAX_RenderSamplerAddress_repeat?MTLSamplerAddressModeRepeat:MTLSamplerAddressModeClampToEdge;d.tAddressMode=p.sampler.address_v==COAX_RenderSamplerAddress_repeat?MTLSamplerAddressModeRepeat:MTLSamplerAddressModeClampToEdge;d.maxAnisotropy=p.sampler.physical_anisotropy;d.compareFunction=p.sampler.compare==COAX_RenderSamplerCompare_less?MTLCompareFunctionLess:MTLCompareFunctionNever;sampler=[device newSamplerStateWithDescriptor:d];if(!sampler)return [self fail:@"Presentation sampler allocation"];samplers[samplerKey]=sampler;}
 MTLRenderPassDescriptor *pass=[MTLRenderPassDescriptor renderPassDescriptor];pass.colorAttachments[0].texture=destination;pass.colorAttachments[0].loadAction=MTLLoadActionClear;pass.colorAttachments[0].storeAction=MTLStoreActionStore;id<MTLRenderCommandEncoder> encoder=[command renderCommandEncoderWithDescriptor:pass];if(!encoder)return [self fail:@"Presentation encoder"];
 [encoder setRenderPipelineState:pipeline];[encoder setCullMode:MTLCullModeNone];uint32_t lengths[29]={0};
 coax_t_RenderHostUse uses[3]={p.uniform,p.image,p.sample};coax_t_MetalHostInterface m=interfaces[p.entry];
 for(int i=0;i<m.count;i++){coax_t_MetalHostUse u=m.uses.e[i];coax_t_RenderHostUse use=uses[u.canonical.use.native_kind-1];NSUInteger at=(NSUInteger)u.slot;BOOL vertex=use.use.stages&1,fragment=use.use.stages&2;
  if(use.use.native_kind==1){lengths[at]=(uint32_t)(sizeof p.uniforms.e/use.use.stride);if(vertex)[encoder setVertexBytes:p.uniforms.e length:sizeof p.uniforms.e atIndex:at];if(fragment)[encoder setFragmentBytes:p.uniforms.e length:sizeof p.uniforms.e atIndex:at];}
  else if(use.use.native_kind==2){id<MTLTexture> source=targets[@(p.source)];MFHandle handle;[targetHandles[@(p.source)]getValue:&handle size:sizeof handle];if(!source||!live(self,handle)){[encoder endEncoding];return [self fail:@"Presentation source is not live"];}if(vertex)[encoder setVertexTexture:source atIndex:at];if(fragment)[encoder setFragmentTexture:source atIndex:at];}
  else if(use.use.native_kind==3){if(vertex)[encoder setVertexSamplerState:sampler atIndex:at];if(fragment)[encoder setFragmentSamplerState:sampler atIndex:at];}
 }
 [encoder setVertexBytes:lengths length:sizeof lengths atIndex:29];[encoder setFragmentBytes:lengths length:sizeof lengths atIndex:29];[encoder drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:(NSUInteger)p.vertices];[encoder endEncoding];if(drawable)[command presentDrawable:drawable];return YES;
}
- (BOOL)bind:(id<MTLRenderCommandEncoder>)e entry:(int)entry op:(coax_t_RenderOp)op media:(const coax_t_RenderMediaRect*)media images:(NSDictionary<NSNumber*,MFMedia*>*)images {
 coax_t_MetalHostInterface m=interfaces[entry];uint32_t lengths[29]={0};
 for(int i=0;i<m.count;i++){coax_t_MetalHostUse u=m.uses.e[i];coax_t_RenderHostUse c=u.canonical;
  if(c.use.native_kind==1){coax_t_RenderHostNamedBuffer generated={0};const void *bytes=NULL;size_t count=0;if(c.source.kind==COAX_RenderSourceKind_bloom_step){bytes=op.bloom_step.e;count=sizeof op.bloom_step.e;}if(c.source.kind==COAX_RenderSourceKind_media_uniform&&media){bytes=media->uniforms.e;count=sizeof media->uniforms.e;}
   if(bytes)generated=(coax_t_RenderHostNamedBuffer){.words={(uint32_t*)bytes,(int64_t)(count/4)},.observed={.source_capacity=count,.logical_payload=count,.gpu_bytes=count,.alignment=4,.zero_initialized=true}};
   coax_t_RenderHostBufferReceipt v=aethyr_render_resolve_buffer(2,c.binding_set,c.use.resource,(coax_v_t_RenderHostNamedBuffer){named,(int64_t)buffers.count},generated);if(v.status!=COAX_RenderResourceStatus_ok)return [self fail:@"Coax refused a buffer binding"];
   lengths[u.slot]=(uint32_t)v.view.elements;
   if(bytes){[e setVertexBytes:bytes length:count atIndex:(NSUInteger)u.slot];[e setFragmentBytes:bytes length:count atIndex:(NSUInteger)u.slot];}
   else{MFBuffer *b=buffers[(NSUInteger)v.named_index];if(!live(self,b->handles[slot]))return [self fail:@"Bound buffer is not live"];[e setVertexBuffer:b->gpu[slot] offset:(NSUInteger)v.view.offset atIndex:(NSUInteger)u.slot];[e setFragmentBuffer:b->gpu[slot] offset:(NSUInteger)v.view.offset atIndex:(NSUInteger)u.slot];}
  }else if(c.use.native_kind==2){id<MTLTexture> texture=nil;if(c.source.kind==COAX_RenderSourceKind_media_image&&media){MFMedia *mediaImage=images[@(media->line)];if(!mediaImage||!live(self,mediaImage->handle))return [self fail:@"Bound media texture is not live"];texture=mediaImage->texture;}
   else{coax_t_RenderHostTextureReceipt v=aethyr_render_resolve_texture(2,c.binding_set,c.use.resource,(coax_v_t_RenderHostNamedTexture){textureNames,(int64_t)textureInput.texture_count},-1);if(v.status!=COAX_RenderResourceStatus_ok)return [self fail:@"Coax refused a texture binding"];
    if(v.source.kind==COAX_RenderSourceKind_render_target)texture=targets[@(v.source.target)];else if(v.source.kind==COAX_RenderSourceKind_pass_source)texture=targets[@(op.source)];else if(v.named_index>=0){if(!live(self,textureHandles[v.named_index]))return [self fail:@"Bound texture is not live"];texture=textureImages[(NSUInteger)v.named_index];}}
   if(!texture)return [self fail:@"Missing admitted texture"];[e setVertexTexture:texture atIndex:(NSUInteger)u.slot];[e setFragmentTexture:texture atIndex:(NSUInteger)u.slot];
  }else if(c.use.native_kind==3){NSString *key=text(c.source.sampler_name);id<MTLSamplerState> sampler=samplers[key];if(!sampler){coax_t_RenderSamplerDescriptor p=aethyr_render_sampler(c.source.sampler_name);if(p.status!=COAX_RenderResourceStatus_ok)return [self fail:@"Coax refused the sampler"];
    MTLSamplerDescriptor *d=[MTLSamplerDescriptor new];d.minFilter=p.min_linear?MTLSamplerMinMagFilterLinear:MTLSamplerMinMagFilterNearest;d.magFilter=p.mag_linear?MTLSamplerMinMagFilterLinear:MTLSamplerMinMagFilterNearest;d.mipFilter=p.mip_linear?MTLSamplerMipFilterLinear:MTLSamplerMipFilterNearest;d.sAddressMode=p.address_u==COAX_RenderSamplerAddress_repeat?MTLSamplerAddressModeRepeat:MTLSamplerAddressModeClampToEdge;d.tAddressMode=p.address_v==COAX_RenderSamplerAddress_repeat?MTLSamplerAddressModeRepeat:MTLSamplerAddressModeClampToEdge;d.maxAnisotropy=(NSUInteger)p.physical_anisotropy;d.compareFunction=p.compare==COAX_RenderSamplerCompare_less?MTLCompareFunctionLess:MTLCompareFunctionNever;sampler=[device newSamplerStateWithDescriptor:d];if(!sampler)return [self fail:@"Sampler allocation"];samplers[key]=sampler;}
   [e setVertexSamplerState:sampler atIndex:(NSUInteger)u.slot];[e setFragmentSamplerState:sampler atIndex:(NSUInteger)u.slot];
  }else return [self fail:@"Unknown admitted resource effect"];
 }
 [e setVertexBytes:lengths length:sizeof lengths atIndex:29];[e setFragmentBytes:lengths length:sizeof lengths atIndex:29];if(entry==COAX_RenderPassId_hud)[e setVertexBuffer:hud[slot] offset:0 atIndex:30];return YES;
}
- (BOOL)render:(float)width height:(float)height scale:(float)scale dt:(float)dt {
 uint64_t began=measureRows?nanoseconds():0;
 if(wantsRecovery&&![self recoverGPU])return NO;
 if(error)return NO;if(face.frame_admit&&!face.frame_admit(pending))return YES;if(slotBusy[slot])return YES;
 if(!fixture&&pending){id<MTLTexture> screen=targets[@(COAX_RenderTarget_screen)];if(screen&&(screen.width!=(NSUInteger)width||screen.height!=(NSUInteger)height||screen.pixelFormat!=pixel(screenFormat)))return YES;}
 // Concrete presentation availability precedes both the admitted clock and
 // the producer. A temporarily unavailable drawable consumes neither.
 id<CAMetalDrawable> drawable=nil;if(layer){layer.drawableSize=CGSizeMake(width,height);drawable=[layer nextDrawable];if(!drawable)return YES;}
 if(clockedFrame){coax_t_HostFrameOffer offer=htl_offer((coax_v_t_HostLifecycle){&life,1},frameTime,pending);if(!offer.admit)return YES;dt=(float)offer.dt;}
 uint32_t count=replay?(uint32_t)textureInput.counts[3]:face.frame(state,(uintptr_t)vertices.mutableBytes,(uintptr_t)uniforms.mutableBytes,width,height,scale,dt,still?1:0);if(fixture){frames++;return YES;}
 lastHud=count;
 coax_t_FrameCounts counts={0};counts.width=(int64_t)width;counts.height=(int64_t)height;counts.hud_vertices=count;counts.vertex_stride=face.vertex_floats();counts.draw_count=face.draw_count(state);counts.water_draws=face.water_draws(state);counts.grass_instances=face.grass_instances(state);counts.particle_instances=face.particle_instances(state);
 if(replay){int64_t *c=textureInput.counts;counts=(coax_t_FrameCounts){.width=c[1],.height=c[2],.hud_vertices=c[3],.vertex_stride=c[4],.draw_count=c[5],.water_draws=c[6],.grass_instances=c[7],.particle_instances=c[8],.surface_vertices=c[9],.caster_vertices=c[10],.caster_instances=c[11]};}
 coax_t_RenderFrameLimits limits;coax_default_t_RenderFrameLimits(&limits);coax_t_RenderUpload uploads[50];int n=(int)buffers.count;
 for(int i=0;i<n;i++){MFBuffer *b=buffers[i];const void *source=NULL;size_t bytes=0;
  if(!replay&&b->source.kind==COAX_RenderSourceKind_main_uniform){source=uniforms.bytes;bytes=b->capacity;}
  if(!replay&&b->source.kind==COAX_RenderSourceKind_dynamic_store){uint32_t v=face.store_version(state,(int32_t)b->source.index);if(!b->observed||v!=b->version){source=(void*)face.store(state,(int32_t)b->source.index);bytes=face.store_bytes(state,(int32_t)b->source.index);b->version=v;b->observed=YES;}}
  if(!replay&&b->source.kind==COAX_RenderSourceKind_particle_emitters){source=(void*)face.particles(state);bytes=face.particle_bytes(state);}
  coax_t_RenderHostUploadReceipt staged=aethyr_render_upload_stage(b->upload,bytes,b->capacity);if(staged.status!=COAX_RenderFrameStatus_ok||wordView(source,b->capacity,bytes).status!=COAX_RenderFrameStatus_ok)return [self fail:@"Coax refused a producer update span"];
  if(bytes)memcpy(b->retained.mutableBytes,source,bytes);b->upload=staged.next;
  named[i]=(coax_t_RenderHostNamedBuffer){.name={b->name,(int64_t)strlen(b->name)},.words={(uint32_t*)b->retained.mutableBytes,(int64_t)b->retained.length/4},.observed={.source_capacity=b->capacity,.logical_payload=b->capacity,.initialized_highwater=b->retained.length,.gpu_bytes=b->gpu[slot].length,.alignment=4,.zero_initialized=true}};
  if(replay)for(size_t k=0;k<textureInput.buffer_count;k++){RfiBuffer *raw=&textureInput.buffers[k];if(!strcmp(raw->name,b->name)){coax_t_RenderHostBool flag=aethyr_render_bool(raw->zero_initialized);if(!flag.ok)return [self fail:@"Coax refused a frozen initialization observation"];named[i].observed=(coax_t_RenderBufferObserved){.source_capacity=raw->source_capacity,.logical_payload=raw->logical_payload,.initialized_highwater=b->retained.length,.gpu_bytes=b->gpu[slot].length,.alignment=4,.zero_initialized=flag.value};}}
  uploads[i]=upload(b->retained.bytes,b->retained.length,b->retained.length,b->gpu[slot].length);
 }
 uploads[n]=upload(vertices.bytes,vertices.length,(size_t)count*face.vertex_floats()*4,hud[slot].length);uploads[n+1]=upload(indices.contents,indices.length,indices.length,indices.length);
 coax_t_RenderRuntimePreflight preflight=aethyr_render_preflight(2,counts,limits,(coax_v_t_RenderUpload){uploads,n+2});if(preflight.status!=COAX_RenderRuntimeStatus_ok)return [self fail:@"Coax refused frame counts or uploads"];
 uintptr_t drawAddress=replay?(uintptr_t)textureInput.draws.data:face.draws(state);coax_t_RenderHostWordView drawView=aethyr_render_words(drawAddress,replay?drawAddress:state,replay?textureInput.draws.size:stateBytes.length,(uint64_t)counts.draw_count*16);if(drawView.status!=COAX_RenderFrameStatus_ok)return [self fail:@"Coax refused the measured draw allocation"];
 coax_t_RenderGeometryBuffer geometry[48];coax_t_RenderHostGeometryBuffers mapped=aethyr_render_geometry_buffers(2,(coax_v_t_RenderHostNamedBuffer){named,n},(coax_v_t_RenderGeometryBuffer){geometry,48});if(mapped.status!=COAX_RenderResourceStatus_ok)return [self fail:@"Coax refused geometry origins"];
 coax_t_RenderDrawDomain *domains=calloc(counts.draw_count?((size_t)counts.draw_count):1,sizeof*domains);coax_v_u32 records={(uint32_t*)drawAddress,drawView.words};
 coax_t_RenderRuntimeFrame receipt=aethyr_render_frame(2,counts,limits,COAX_RenderReadiness_ready,true,(coax_v_t_RenderUpload){uploads,n+2},records,words(indices.contents,indices.length,indices.length),(coax_v_f32){uniforms.mutableBytes,(int64_t)uniforms.length/4},(coax_v_t_RenderGeometryBuffer){geometry,mapped.count},(coax_v_f32){vertices.mutableBytes,(int64_t)vertices.length/4},(coax_v_t_RenderDrawDomain){domains,counts.draw_count},aethyr_render_geometry_work());free(domains);
 if(receipt.status!=COAX_RenderRuntimeStatus_ok){
  coax_t_RenderDrawDomain d[4096];coax_t_RenderGeometryResult why=metal_geometry_diagnose(counts,records,words(indices.contents,indices.length,indices.length),(coax_v_f32){uniforms.mutableBytes,(int64_t)uniforms.length/4},(coax_v_t_RenderGeometryBuffer){geometry,mapped.count},(coax_v_f32){vertices.mutableBytes,(int64_t)vertices.length/4},(coax_v_t_RenderDrawDomain){d,4096});fprintf(stderr," diagnostic geometry%u resource%lld draw%lld work%lld\n",why.status,(long long)why.bad_resource,(long long)why.bad_draw,(long long)why.work);
  return [self fail:[NSString stringWithFormat:@"Coax refused frame: runtime%u geometry%u resource%lld draw%lld",receipt.status,receipt.geometry_status,(long long)why.bad_resource,(long long)why.bad_draw]];}
 coax_t_RenderMediaRequest requests[112];uint32_t mediaLength=replay?(uint32_t)textureInput.media_text.size:face.media_len?face.media_len(state):0;coax_t_RenderMediaParse parsed=aethyr_render_media_parse((coax_string){replay?(const char*)textureInput.media_text.data:face.media?(const char*)face.media(state):NULL,mediaLength},(coax_v_t_RenderMediaRequest){requests,112});if(parsed.status!=COAX_RenderMediaStatus_ok)return [self fail:@"Coax refused the media packet"];
 coax_t_RenderHostMediaImage observed[112];coax_t_RenderHostMediaTexture physical[112];coax_t_RenderMediaRect mediaRects[112];char mediaNames[112][32];NSMutableDictionary<NSNumber*,MFMedia*> *mediaByLine=[NSMutableDictionary dictionary];int imageCount=0,physicalCount=0;
 for(int i=0;i<parsed.count;i++){coax_t_RenderMediaRequest request=requests[i];NSString *url=text(request.url);MFMedia *m=mediaCache[url];
  if(replay){char scratch[32];coax_string name=aethyr_render_texture_name((coax_t_RenderBindingSource){.kind=COAX_RenderSourceKind_media_image,.index=request.line,.valid=true},(coax_v_u8){(uint8_t*)scratch,32});for(size_t k=0;k<textureInput.texture_count;k++)if(same(name,textureNames[k].name)){m=[MFMedia new];m->texture=textureImages[k];m->handle=textureHandles[k];m->width=m->texture.width;m->height=m->texture.height;}}
  if(!replay&&!m&&request.kind==COAX_RenderMediaKind_image){
   NSString *relative=[url hasPrefix:@"/worlds/"]?[@"/realms/" stringByAppendingString:[url substringFromIndex:8]]:url;NSString *base=[[directory stringByAppendingPathComponent:@"web"]stringByStandardizingPath];NSString *path=[[base stringByAppendingPathComponent:[relative substringFromIndex:1]]stringByStandardizingPath];
   if([path hasPrefix:[base stringByAppendingString:@"/"]]){if(face.host_contract_version&&face.host_contract_version()==1){NSData *source=[NSData dataWithContentsOfFile:path];m=[MFMedia new];m->failed=YES;mediaCache[url]=m;
     if(source){coax_t_HostMediaPlan p=htm_plan(cv(source));if(p.status==0){NSMutableData *work=[NSMutableData dataWithLength:(NSUInteger)p.work],*rgba=[NSMutableData dataWithLength:(NSUInteger)p.bytes];if(work&&rgba&&htm_decode(cv(source),cv(work),cv(rgba))==0){
       m->width=p.width;m->height=p.height;MTLTextureDescriptor *d=[MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatRGBA8Unorm width:m->width height:m->height mipmapped:NO];d.usage=MTLTextureUsageShaderRead;d.storageMode=MTLStorageModeShared;
       if(acquire(self,5,aethyr_render_texture_words(m->width,m->height,1,1,4),&m->handle)){m->texture=[device newTextureWithDescriptor:d];if(m->texture){[m->texture replaceRegion:MTLRegionMake2D(0,0,m->width,m->height) mipmapLevel:0 withBytes:rgba.bytes bytesPerRow:m->width*4];m->rgba=rgba;m->failed=NO;}else releaseHandle(self,&m->handle);}
       if(m->failed){m->width=0;m->height=0;}
     }}}
    }else{CGImageSourceRef source=CGImageSourceCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:path],NULL);CGImageRef cg=source?CGImageSourceCreateImageAtIndex(source,0,NULL):NULL;if(source)CFRelease(source);
    if(cg){coax_t_RenderMediaExtent extent=aethyr_render_media_extent(request.kind,CGImageGetWidth(cg),CGImageGetHeight(cg));if(extent.ok){m=[MFMedia new];m->width=extent.width;m->height=extent.height;NSMutableData *rgba=[NSMutableData dataWithLength:(NSUInteger)m->width*m->height*4];CGColorSpaceRef colour=CGColorSpaceCreateWithName(kCGColorSpaceSRGB);CGContextRef context=CGBitmapContextCreate(rgba.mutableBytes,m->width,m->height,8,m->width*4,colour,kCGImageAlphaPremultipliedLast|kCGBitmapByteOrder32Big);CGColorSpaceRelease(colour);if(context){CGContextSetInterpolationQuality(context,kCGInterpolationHigh);CGContextDrawImage(context,CGRectMake(0,0,m->width,m->height),cg);CGContextRelease(context);
       if(!render_image_straight(cv(rgba))){CGImageRelease(cg);return [self fail:@"Decoded media is not premultiplied RGBA8"];}
       MTLTextureDescriptor *d=[MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatRGBA8Unorm width:m->width height:m->height mipmapped:NO];d.usage=MTLTextureUsageShaderRead;d.storageMode=MTLStorageModeShared;
       if(!acquire(self,5,aethyr_render_texture_words(m->width,m->height,1,1,4),&m->handle)){CGImageRelease(cg);return [self fail:@"Media ledger capacity"];}m->rgba=rgba;m->texture=[device newTextureWithDescriptor:d];[m->texture replaceRegion:MTLRegionMake2D(0,0,m->width,m->height) mipmapLevel:0 withBytes:rgba.bytes bytesPerRow:m->width*4];mediaCache[url]=m;}}CGImageRelease(cg);}}}}
  BOOL duplicate=NO;for(int j=0;j<imageCount;j++)if(same(observed[j].url,request.url)&&observed[j].kind==request.kind)duplicate=YES;
  if(!duplicate){coax_t_RenderHostMediaImage observation={.kind=request.kind,.url=request.url,.ready=m&&m->texture,.width=m?m->width:0,.height=m?m->height:0};if(replay)for(size_t k=0;k<textureInput.media_image_count;k++){RfiMediaImage *raw=&textureInput.media_images[k];if(same(request.url,(coax_string){raw->url,(int64_t)strlen(raw->url)})){coax_t_RenderHostMediaObserved q=aethyr_render_media_observe(raw->kind,(coax_string){raw->url,(int64_t)strlen(raw->url)},raw->ready,raw->width,raw->height);if(q.status!=COAX_RenderHostMediaStatus_ok)return [self fail:@"Coax refused frozen media observation"];observation=q.image;}}observed[imageCount++]=observation;}
  if(m&&m->texture){[mediaByLine setObject:m forKey:@(request.line)];coax_string name=aethyr_render_texture_name((coax_t_RenderBindingSource){.kind=COAX_RenderSourceKind_media_image,.index=request.line,.valid=true},(coax_v_u8){(uint8_t*)mediaNames[physicalCount],32});physical[physicalCount++]=(coax_t_RenderHostMediaTexture){.name=name,.width=m->width,.height=m->height,.layers=1};}
 }
 if(!replay&&face.host_contract_version&&face.host_contract_version()==1){coax_t_HostMediaNotice rows[112];for(int i=0;i<imageCount;i++){coax_t_RenderHostMediaImage o=observed[i];MFMedia *m=mediaCache[text(o.url)];rows[i]=(coax_t_HostMediaNotice){.url=o.url,.ready=o.ready,.failed=m&&m->failed,.width=o.width,.height=o.height};}NSMutableData *raw=[NSMutableData dataWithLength:face.inbox_cap()];int64_t n=htn_format((coax_v_t_HostMediaNotice){rows,imageCount},cv(raw));if(n<0)return [self fail:@"Coax refused media notice capacity or observations"];raw.length=(NSUInteger)n;if(n&&![raw isEqualToData:lastMediaNotice])mediaNotice=raw;}
 coax_t_RenderHostMediaReceipt media=aethyr_render_media_draws(2,(coax_v_t_RenderMediaRequest){requests,parsed.count},(coax_v_t_RenderHostMediaImage){observed,imageCount},counts.width,counts.height,(coax_v_t_RenderHostMediaTexture){physical,physicalCount},(coax_v_t_RenderMediaRect){mediaRects,112});if(media.status!=COAX_RenderHostMediaStatus_ok)return [self fail:@"Coax refused media composition"];
 for(int64_t i=0;i<receipt.plan.count;i++){coax_t_RenderOp op=receipt.plan.ops.e[i];if(op.kind==COAX_RenderOpKind_begin){if(![self ensureTarget:op.colour0 width:counts.width height:counts.height]||![self ensureTarget:op.colour1 width:counts.width height:counts.height]||![self ensureTarget:op.depth width:counts.width height:counts.height])return NO;}if(op.kind==COAX_RenderOpKind_copy){if(![self ensureTarget:op.source width:counts.width height:counts.height]||![self ensureTarget:op.target width:counts.width height:counts.height])return NO;}}
 if(!aethyr_render_registry_advance(ledger(self)))return [self fail:@"Coax ledger epochs exhausted"];
 if(!live(self,hudHandles[slot])||!live(self,indexHandle))return [self fail:@"Frame buffer is not live"];
 for(MFBuffer *b in buffers){if(!live(self,b->handles[slot]))return [self fail:@"Frame storage is not live"];}
 // Only now may a candidate write GPU-visible bytes.
 for(MFBuffer *b in buffers)memcpy(b->gpu[slot].contents,b->retained.bytes,b->retained.length);if(count)memcpy(hud[slot].contents,vertices.bytes,(size_t)count*face.vertex_floats()*4);
 id<MTLCommandBuffer> cb=[queue commandBuffer];id<MTLRenderCommandEncoder> encoder=nil;int64_t eachStart=0,current=0,eachEnd=0;uint64_t calls=0;
 for(int64_t at=0;at<receipt.plan.count;at++){coax_t_RenderOp op=receipt.plan.ops.e[at];
  if(op.kind==COAX_RenderOpKind_begin){MTLRenderPassDescriptor *d=[MTLRenderPassDescriptor renderPassDescriptor];coax_t_RenderTarget colours[2]={op.colour0,op.colour1};coax_t_RenderHostClear clear=aethyr_render_clear();for(int i=0;i<2;i++)if(colours[i]!=COAX_RenderTarget_none){MTLRenderPassColorAttachmentDescriptor *a=d.colorAttachments[i];a.texture=targets[@(colours[i])];a.loadAction=op.keep?MTLLoadActionLoad:MTLLoadActionClear;a.storeAction=MTLStoreActionStore;a.clearColor=MTLClearColorMake(clear.colour.e[0],clear.colour.e[1],clear.colour.e[2],clear.colour.e[3]);}if(op.depth!=COAX_RenderTarget_none){d.depthAttachment.texture=targets[@(op.depth)];d.depthAttachment.loadAction=op.keep?MTLLoadActionLoad:MTLLoadActionClear;d.depthAttachment.storeAction=op.store_depth?MTLStoreActionStore:MTLStoreActionDontCare;d.depthAttachment.clearDepth=clear.depth;}encoder=[cb renderCommandEncoderWithDescriptor:d];if(!encoder)return [self fail:@"Render pass encoder"];[encoder setCullMode:MTLCullModeNone];[encoder setFrontFacingWinding:MTLWindingCounterClockwise];}
  else if(op.kind==COAX_RenderOpKind_end){[encoder endEncoding];encoder=nil;}
  else if(op.kind==COAX_RenderOpKind_copy){id<MTLTexture> src=targets[@(op.source)],dst=targets[@(op.target)];id<MTLBlitCommandEncoder> blit=[cb blitCommandEncoder];[blit copyFromTexture:src sourceSlice:0 sourceLevel:0 sourceOrigin:MTLOriginMake(0,0,0) sourceSize:MTLSizeMake(src.width,src.height,1) toTexture:dst destinationSlice:0 destinationLevel:0 destinationOrigin:MTLOriginMake(0,0,0)];[blit endEncoding];}
  else if(op.kind==COAX_RenderOpKind_each_begin){eachStart=at;current=op.first;eachEnd=op.first+op.count;}
  else if(op.kind==COAX_RenderOpKind_each_end){if(++current<eachEnd)at=eachStart;}
  else if(op.kind==COAX_RenderOpKind_draw||op.kind==COAX_RenderOpKind_media_before||op.kind==COAX_RenderOpKind_media_after){int entry=op.kind==COAX_RenderOpKind_draw?op.pass:13;id<MTLRenderPipelineState> pipeline=[self pipeline:entry];if(!pipeline){[encoder endEncoding];return NO;}[encoder setRenderPipelineState:pipeline];coax_t_RenderPassState p=aethyr_render_pipeline(2,entry).state;
   NSString *key=[NSString stringWithFormat:@"%d:%d",p.depth,p.depth_write];id<MTLDepthStencilState> depth=depthStates[key];if(!depth){MTLDepthStencilDescriptor *d=[MTLDepthStencilDescriptor new];d.depthCompareFunction=p.depth==COAX_RenderDepth_less?MTLCompareFunctionLess:MTLCompareFunctionAlways;d.depthWriteEnabled=p.depth_write;depth=[device newDepthStencilStateWithDescriptor:d];depthStates[key]=depth;}[encoder setDepthStencilState:depth];[encoder setDepthBias:(float)p.bias slopeScale:(float)p.slope clamp:0];
   if(entry==13){for(int64_t i=0;i<media.count;i++){coax_t_RenderMediaRect rect=mediaRects[i];if(!rect.draw||rect.over!=(op.kind==COAX_RenderOpKind_media_after))continue;if(![self bind:encoder entry:entry op:op media:&rect images:mediaByLine]){[encoder endEncoding];return NO;}[encoder drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:(NSUInteger)aethyr_render_media_vertices()];calls++;}}
   else{if(![self bind:encoder entry:entry op:op media:NULL images:mediaByLine]){[encoder endEncoding];return NO;}
    if(op.draw_source==COAX_RenderDrawSource_vertices){if(op.count&&op.instances){[encoder drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:(NSUInteger)op.first vertexCount:(NSUInteger)op.count instanceCount:(NSUInteger)op.instances];calls++;}}
    else{int64_t first=op.draw_source==COAX_RenderDrawSource_current?current:op.first,end=op.draw_source==COAX_RenderDrawSource_current?current+1:op.first+op.count;for(int64_t i=first;i<end;i++){coax_t_RenderDecodedDraw d=aethyr_render_decode_draw(records,i);if(d.kind==COAX_RenderDrawKind_skip)continue;if(d.kind==COAX_RenderDrawKind_nonindexed)[encoder drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:d.first vertexCount:d.count instanceCount:d.instances baseInstance:d.base_instance];else if(d.kind==COAX_RenderDrawKind_indexed)[encoder drawIndexedPrimitives:MTLPrimitiveTypeTriangle indexCount:d.count indexType:MTLIndexTypeUInt32 indexBuffer:indices indexBufferOffset:d.first*4 instanceCount:d.instances baseVertex:0 baseInstance:d.base_instance];else{[encoder endEncoding];return [self fail:@"Admitted draw records changed"];}calls++;}}
   }
  }
 }
 if(encoder)[encoder endEncoding];if(presentationExperiment){if(![self present:cb drawable:drawable width:(int64_t)width height:(int64_t)height])return NO;}else if(drawable){id<MTLTexture> screen=targets[@(COAX_RenderTarget_screen)];id<MTLBlitCommandEncoder> blit=[cb blitCommandEncoder];[blit copyFromTexture:screen sourceSlice:0 sourceLevel:0 sourceOrigin:MTLOriginMake(0,0,0) sourceSize:MTLSizeMake(screen.width,screen.height,1) toTexture:drawable.texture destinationSlice:0 destinationLevel:0 destinationOrigin:MTLOriginMake(0,0,0)];[blit endEncoding];[cb presentDrawable:drawable];}
 MFFrameObservation observation={.frame=frames+1,.start_ns=began,.submit_ns=measureRows?nanoseconds():0,.input_sequence=inputSequence,.input_ns=inputNS,.x_mm=face.field_probe?face.field_probe(state,0):0,.z_mm=face.field_probe?face.field_probe(state,2):0,.width=counts.width,.height=counts.height,.scale=scale};uint64_t observedEpoch=measureEpoch;
 int submittedSlot=slot;uint64_t submittedEpoch=gpuEpoch;slotBusy[slot]=YES;pending++;slot=(slot+1)%2;[inflight addObject:cb];__weak MFRealm *weak=self;[cb addCompletedHandler:^(id<MTLCommandBuffer> completed){uint64_t finished=observation.start_ns?nanoseconds():0;dispatch_async(dispatch_get_main_queue(),^{MFRealm *r=weak;if(!r||r->closed||r->gpuEpoch!=submittedEpoch)return;[r->inflight removeObject:completed];r->slotBusy[submittedSlot]=NO;r->pending--;if(completed.error){if(completed.error.code==MTLCommandBufferErrorDeviceRemoved)r->wantsRecovery=YES;else[r fail:completed.error.localizedDescription];}else{r->completedFrames++;r->lastGpuNS=(uint64_t)((completed.GPUEndTime-completed.GPUStartTime)*1e9);if(observation.start_ns&&r->measureRows&&r->measureEpoch==observedEpoch){MFFrameObservation row=observation;row.complete_ns=finished;row.gpu_ns=r->lastGpuNS;if(r->measureRows.length/sizeof(row)<4096)[r->measureRows appendBytes:&row length:sizeof(row)];else r->measureDropped++;}}});}];[cb commit];frames++;draws=calls;
 for(MFBuffer *b in buffers)b->upload=aethyr_render_upload_ack(b->upload,true);return YES;
}
- (void)releaseGPU {
 if(queue){id<MTLCommandBuffer> end=[queue commandBuffer];[end commit];[end waitUntilCompleted];}
 for(id<MTLCommandBuffer> cb in inflight)if(cb.status==MTLCommandBufferStatusCompleted&&!cb.error)completedFrames++;[inflight removeAllObjects];
 gpuEpoch++;pending=0;slot=0;slotBusy[0]=slotBusy[1]=NO;
 for(MFBuffer *b in buffers)for(int i=0;i<2;i++)releaseHandle(self,&b->handles[i]);for(int i=0;i<2;i++)releaseHandle(self,&hudHandles[i]);releaseHandle(self,&indexHandle);
 releaseHandle(self,&presentedHandle);presentedTexture=nil;
 for(NSValue *v in targetHandles.allValues){MFHandle h;[v getValue:&h size:sizeof h];releaseHandle(self,&h);}for(size_t i=0;i<textureInput.texture_count;i++)releaseHandle(self,&textureHandles[i]);for(MFMedia *m in mediaCache.allValues)releaseHandle(self,&m->handle);
 for(MFBuffer *b in buffers)b->gpu=nil;for(MFMedia *m in mediaCache.allValues)m->texture=nil;if(layer)layer.device=nil;
 [targets removeAllObjects];[targetHandles removeAllObjects];[textureImages removeAllObjects];[pipelines removeAllObjects];[depthStates removeAllObjects];[samplers removeAllObjects];[libraries removeAllObjects];hud=nil;indices=nil;queue=nil;device=nil;
}
- (BOOL)recoverGPU {
 if(fixture||closed)return NO;coax_t_LifeActions lost=htl_event((coax_v_t_HostLifecycle){&life,1},8,htl_tick((int64_t)(nanoseconds()/1000)),0);if(!(lost.does&32))return [self fail:@"Lifecycle did not stop drawing"];
 NSData *indexBytes=[NSData dataWithBytes:indices.contents length:indices.length];NSUInteger hudBytes=hud[0].length;
 [self releaseGPU];recoveryReleasedLive=ledger(self).data[0].live;
 device=MTLCreateSystemDefaultDevice();queue=[device newCommandQueue];if(!device||!queue)return [self fail:@"Metal recovery did not obtain a device"];
 coax_t_LifeActions next=htl_event((coax_v_t_HostLifecycle){&life,1},9,htl_tick((int64_t)(nanoseconds()/1000)),0);if(!(next.does&64))return [self fail:@"Lifecycle did not request GPU reload"];
 if(![self compileLibraries])return NO;
 NSMutableArray *pair=[NSMutableArray array];for(int i=0;i<2;i++){if(!acquire(self,3,aethyr_render_buffer_words(hudBytes),&hudHandles[i]))return [self fail:@"Recovery HUD ledger capacity"];id<MTLBuffer> b=[device newBufferWithLength:hudBytes options:MTLResourceStorageModeShared];if(!b)return [self fail:@"Recovery HUD allocation"];[pair addObject:b];}hud=pair;
 if(!acquire(self,4,aethyr_render_buffer_words(indexBytes.length),&indexHandle))return [self fail:@"Recovery index ledger capacity"];indices=[device newBufferWithBytes:indexBytes.bytes length:indexBytes.length options:MTLResourceStorageModeShared];if(!indices)return [self fail:@"Recovery index allocation"];
 for(MFBuffer *b in buffers){NSMutableArray *pair=[NSMutableArray array];for(int i=0;i<2;i++){if(!acquire(self,b->source.kind==COAX_RenderSourceKind_main_uniform?0:1,aethyr_render_buffer_words(b->retained.length),&b->handles[i]))return [self fail:@"Recovery buffer ledger capacity"];id<MTLBuffer> gpu=[device newBufferWithLength:b->retained.length options:MTLResourceStorageModeShared];if(!gpu)return [self fail:@"Recovery buffer allocation"];[pair addObject:gpu];}b->gpu=pair;}
 if(![self createTextures])return NO;
 for(MFMedia *m in mediaCache.allValues){if(m->failed)continue;if(!acquire(self,5,aethyr_render_texture_words(m->width,m->height,1,1,4),&m->handle))return [self fail:@"Recovery media ledger capacity"];MTLTextureDescriptor *d=[MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatRGBA8Unorm width:m->width height:m->height mipmapped:NO];d.usage=MTLTextureUsageShaderRead;d.storageMode=MTLStorageModeShared;m->texture=[device newTextureWithDescriptor:d];if(!m->texture)return [self fail:@"Recovery media allocation"];[m->texture replaceRegion:MTLRegionMake2D(0,0,m->width,m->height) mipmapLevel:0 withBytes:m->rgba.bytes bytesPerRow:m->width*4];}
 if(layer)layer.device=device;wantsRecovery=NO;recoveries++;return YES;
}
- (void)shutdown {
 if(closed)return;closed=YES;ht_reset(net(self));if(socketContext){networkAPI.close(socketContext);socketContext=NULL;}[self releaseGPU];lastClosedLive=ledger(self).data[0].live;lastClosedWords=ledger(self).data[0].words;
 [buffers removeAllObjects];[mediaCache removeAllObjects];
}
- (void)dealloc {
 [self shutdown];
 rfi_close(&textureInput);if(image)dlclose(image);
}
@end

void *mf_open(const char *resources,const char *producer,bool golden,bool fixture){if(activeRealm){lastFailure=@"Only one active realm image is admitted";return NULL;}MFRealm *r=[MFRealm new];activeRealm=r;if(![r open:[NSString stringWithUTF8String:resources] producer:[NSString stringWithUTF8String:producer] golden:golden fixture:fixture]){lastFailure=r->error;activeRealm=nil;return NULL;}return (__bridge_retained void*)r;}
const char *mf_error(void *host){return (host?((__bridge MFRealm*)host)->error:lastFailure).UTF8String;}
void mf_message(void *host,int32_t kind,const uint8_t *bytes,size_t n){[(__bridge MFRealm*)host message:kind data:[NSData dataWithBytes:bytes length:n]];}
bool mf_key(void *host,int32_t key,bool down,int32_t mods){MFRealm *r=(__bridge MFRealm*)host;if(r->measureRows){r->inputNS=nanoseconds();r->inputSequence++;}return down?r->face.key&&r->face.key(r->state,key,mods):r->face.key_up&&r->face.key_up(r->state,key,mods);}
bool mf_pointer(void *host,float x,float y,int32_t phase,int32_t kind,int32_t id){MFRealm *r=(__bridge MFRealm*)host;return r->face.pointer&&r->face.pointer(r->state,x,y,phase,kind,id);}
void mf_wheel(void *host,float dy){MFRealm *r=(__bridge MFRealm*)host;if(r->face.wheel)r->face.wheel(r->state,dy);}
void mf_focus(void *host,bool focused){MFRealm *r=(__bridge MFRealm*)host;htl_event((coax_v_t_HostLifecycle){&r->life,1},focused?2:3,htl_tick((int64_t)(nanoseconds()/1000)),0);if(focused){if(r->face.focus)r->face.focus(r->state);}else if(r->face.blur)r->face.blur(r->state);}
int32_t mf_action(void *host){MFRealm *r=(__bridge MFRealm*)host;return r->face.action?r->face.action(r->state):0;}
const uint8_t *mf_outbox(void *host,size_t *n){MFRealm *r=(__bridge MFRealm*)host;*n=r->face.outbox_len?r->face.outbox_len(r->state):0;return r->face.outbox?(void*)r->face.outbox(r->state):NULL;}
void mf_action_done(void *host){MFRealm *r=(__bridge MFRealm*)host;if(r->face.action_done)r->face.action_done(r->state);}
int32_t mf_probe(void *host,int32_t field){MFRealm *r=(__bridge MFRealm*)host;return r->face.field_probe?r->face.field_probe(r->state,field):-1;}
int64_t mf_observe(void *host,int64_t event,int64_t field){MFRealm *r=(__bridge MFRealm*)host;int64_t(*probe)(uintptr_t,int32_t,int32_t)=dlsym(r->image,"coaxial_host_probe");return probe?probe(r->state,(int32_t)event,(int32_t)field):-1;}
void mf_observe_reset(void *host){MFRealm *r=(__bridge MFRealm*)host;void(*reset)(uintptr_t)=dlsym(r->image,"coaxial_host_reset");if(reset)reset(r->state);}
void mf_layer(void *host,void *raw){MFRealm *r=(__bridge MFRealm*)host;r->layer=(__bridge CAMetalLayer*)raw;if(r->layer){r->layer.device=r->device;r->layer.pixelFormat=MTLPixelFormatBGRA8Unorm;r->layer.framebufferOnly=NO;r->layer.displaySyncEnabled=YES;r->screenFormat=r->presentationExperiment?COAX_RenderTextureFormat_rgba16f:COAX_RenderTextureFormat_bgra8;}else r->screenFormat=COAX_RenderTextureFormat_rgba16f;}
bool mf_offscreen_bgra8(void *host){MFRealm *r=(__bridge MFRealm*)host;if(r->frames||r->pending||r->layer)return false;r->screenFormat=COAX_RenderTextureFormat_bgra8;return true;}
bool mf_presentation_experiment(void *host){MFRealm *r=(__bridge MFRealm*)host;if(r->frames||r->pending||r->fixture)return false;r->presentationExperiment=YES;r->screenFormat=COAX_RenderTextureFormat_rgba16f;return true;}
bool mf_presentation_experiment_v1(void *host){if(!presentationPolicy(YES)||!mf_presentation_experiment(host))return false;((__bridge MFRealm*)host)->presentationQuantized=YES;return true;}
bool mf_present_probe(void *host,const uint8_t *bytes,size_t count,int64_t width,int64_t height,int64_t format,bool quantized){
 MFRealm *r=(__bridge MFRealm*)host;if(r->frames||r->pending||r->layer||r->fixture)return false;
 // The optional private product supplies its own checked Coax descriptor.
 // Its absence cannot silently fall back to the legacy quantization path.
 MFPresentationPolicy policy=presentationPolicy(quantized);
 if(!policy)return [r fail:@"Private Coax presentation descriptor is absent"];
 coax_t_RenderPresentation p=policy(2,width,height,format,16384);if(p.status!=COAX_RenderResourceStatus_ok)return [r fail:@"Coax refused presentation probe extent/format"];
 int64_t size=aethyr_render_format_words(p.draw.format,p.draw.width,p.draw.height,1,1)*4;
 if(size<=0||count!=(uint64_t)size||!htb_copy_span((uintptr_t)bytes,count,count,UINT64_MAX))return [r fail:@"Presentation probe allocation observation differs from its descriptor"];
 r->screenFormat=p.draw.format;r->presentationExperiment=YES;if(![r ensureTarget:p.source width:p.draw.width height:p.draw.height]||!aethyr_render_registry_advance(ledger(r)))return false;
 id<MTLTexture> source=r->targets[@(p.source)];[source replaceRegion:MTLRegionMake2D(0,0,p.draw.width,p.draw.height) mipmapLevel:0 withBytes:bytes bytesPerRow:(NSUInteger)(size/p.draw.height)];
 id<MTLCommandBuffer> command=[r->queue commandBuffer];if(!command||![r present:command drawable:nil descriptor:p])return false;
 [command commit];[command waitUntilCompleted];if(command.error)return [r fail:command.error.localizedDescription];r->completedFrames++;r->draws=1;return true;
}
bool mf_frame(void *host,float width,float height,float scale,float dt){MFRealm *r=(__bridge MFRealm*)host;BOOL ok=[r render:width height:height scale:scale dt:dt];if(ok)[r deliverMediaNotice];return ok;}
bool mf_frame_at(void *host,float width,float height,float scale,int64_t monotonic_us){MFRealm *r=(__bridge MFRealm*)host;r->frameTime=monotonic_us;r->clockedFrame=YES;BOOL ok=[r render:width height:height scale:scale dt:0];r->clockedFrame=NO;if(ok)[r deliverMediaNotice];return ok;}
void mf_shown(void *host,bool shown){MFRealm *r=(__bridge MFRealm*)host;coax_t_LifeActions a=htl_event((coax_v_t_HostLifecycle){&r->life,1},shown?5:4,htl_tick((int64_t)(nanoseconds()/1000)),0);if((a.does&1)&&r->face.blur)r->face.blur(r->state);}
int64_t mf_lifecycle_info(void *host,int32_t field){MFRealm *r=(__bridge MFRealm*)host;switch(field){case 0:return r->life.profile.e[0].active;case 1:return r->life.profile.e[0].drawing;case 2:return r->life.shown;case 3:return r->life.focused;case 4:return r->life.last_us;default:return -1;}}
bool mf_replay(void *host,const char *path){return [(__bridge MFRealm*)host loadReplay:path];}
bool mf_recover(void *host){return [(__bridge MFRealm*)host recoverGPU];}
const uint8_t *mf_cpu_bytes(void *host,int32_t kind,size_t *n){MFRealm *r=(__bridge MFRealm*)host;*n=0;if(kind==0){*n=r->uniforms.length;return r->uniforms.bytes;}if(kind==1){*n=(size_t)r->lastHud*r->face.vertex_floats()*4;return r->vertices.bytes;}if(kind==2){*n=(size_t)r->face.draw_count(r->state)*16;return(void*)r->face.draws(r->state);}if(kind>=3&&kind<=6){*n=r->face.store_bytes(r->state,kind-3);return(void*)r->face.store(r->state,kind-3);}if(kind==7){*n=r->face.particle_bytes(r->state);return(void*)r->face.particles(r->state);}if(kind==8){*n=r->stateBytes.length;return r->stateBytes.bytes;}if(kind==9&&r->face.media&&r->face.media_len){*n=r->face.media_len(r->state);return(void*)r->face.media(r->state);}return NULL;}
size_t mf_cpu_capacity(void *host,int32_t kind){MFRealm *r=(__bridge MFRealm*)host;if(kind==0)return r->uniforms.length;if(kind>=3&&kind<=6)return r->face.store_cap(kind-3);if(kind==7)return r->face.particle_cap();return 0;}
void mf_still(void *host,bool value){((__bridge MFRealm*)host)->still=value;}
bool mf_asset(void *host,int32_t index,const uint8_t *bytes,size_t count){MFRealm *r=(__bridge MFRealm*)host;if(!r->face.asset||!r->face.asset_capacity||!r->face.asset_ready)return false;uintptr_t dst=r->face.asset(r->state,index,(uint32_t)count);uint32_t cap=r->face.asset_capacity(r->state,index);if(aethyr_render_asset_span(dst,cap,count,UINT64_MAX).status!=COAX_RenderFrameStatus_ok){r->face.asset_ready(r->state,index,0);return false;}if(count)memcpy((void*)dst,bytes,count);r->face.asset_ready(r->state,index,(uint32_t)count);return true;}
bool mf_surface(void *host,int64_t cw,int64_t ch,int64_t dw,int64_t dh,const char *quality,MFSurface *out){MFRealm *r=(__bridge MFRealm*)host;coax_string name={quality,quality?(int64_t)strlen(quality):0};coax_t_RenderHostSurface s=render_host_surface(dw,dh,cw,ch,r->face.pixel_budget?r->face.pixel_budget():0,render_host_surface_mode(name));*out=(MFSurface){s.width,s.height,s.scale};return s.valid;}
int64_t mf_info(void *host,int32_t field){MFRealm *r=(__bridge MFRealm*)host;switch(field){case 0:return(int64_t)r->frames;case 1:return(int64_t)r->draws;case 2:return r->pending;case 3:return ledger(r).data[0].live;case 4:return ledger(r).data[0].words;case 5:return ledger(r).data[0].epoch;case 6:return(int64_t)r->state;case 7:return r->face.net_probe?r->face.net_probe(r->state,0):-1;case 8:return r->face.net_probe?r->face.net_probe(r->state,1):-1;case 9:return r->face.wants_text?r->face.wants_text(r->state):0;case 10:return(int64_t)r->completedFrames;case 11:return(int64_t)r->lastGpuNS;case 12:return(int64_t)r->recoveries;case 13:return r->recoveryReleasedLive;default:return -1;}}
void mf_measure_reset(void *host){MFRealm *r=(__bridge MFRealm*)host;r->measureRows=[NSMutableData data];r->measureEpoch++;r->measureDropped=0;r->inputSequence=0;r->inputNS=0;}
const MFFrameObservation *mf_measure_rows(void *host,size_t *count,size_t *dropped){MFRealm *r=(__bridge MFRealm*)host;*count=r->measureRows.length/sizeof(MFFrameObservation);*dropped=r->measureDropped;return r->measureRows.bytes;}
const char *mf_take_effect(void *host){MFRealm *r=(__bridge MFRealm*)host;if(!r->effects.count)return NULL;NSDictionary *d=r->effects.firstObject;[r->effects removeObjectAtIndex:0];r->lastEffect=[NSJSONSerialization dataWithJSONObject:d options:0 error:NULL];NSMutableData *zero=[r->lastEffect mutableCopy];[zero appendBytes:"\0" length:1];r->lastEffect=zero;return r->lastEffect.bytes;}
bool mf_http_chunk(void *host,int64_t id,int64_t generation,int64_t engine,int64_t count){return ht_http_chunk(net((__bridge MFRealm*)host),(coax_t_HostOwner){id,generation,engine},count);}
void mf_http_finish(void *host,int64_t id,int64_t generation,int64_t engine,const uint8_t *bytes,size_t count,bool ok,bool utf8){MFRealm *r=(__bridge MFRealm*)host;coax_t_HostDelivery d=ht_http_finish(net(r),(coax_t_HostOwner){id,generation,engine},milliseconds(),ok,utf8);if(d.deliver)[r message:(int32_t)d.kind data:d.failure?[NSData dataWithBytes:"0\n" length:2]:[NSData dataWithBytes:bytes length:MIN(count,(size_t)d.bytes)]];}
void mf_http_timeout(void *host,int64_t id,int64_t generation,int64_t engine){MFRealm *r=(__bridge MFRealm*)host;coax_t_HostDelivery d=ht_http_timeout(net(r),(coax_t_HostOwner){id,generation,engine},milliseconds());if(d.deliver)[r message:(int32_t)d.kind data:[NSData dataWithBytes:"0\n" length:2]];}
void mf_socket_event(void *host,int64_t id,int64_t generation,int64_t engine,int32_t event,const uint8_t *bytes,size_t count){MFRealm *r=(__bridge MFRealm*)host;coax_t_HostDelivery d=ht_socket_deliver(net(r),(coax_t_HostOwner){id,generation,engine},event,count,r->face.inbox_cap());if(d.deliver)[r message:(int32_t)d.kind data:[NSData dataWithBytes:bytes length:(NSUInteger)d.bytes]];}
static bool snapshotTexture(MFRealm *r,id<MTLTexture> t,const char *path){
 id<MTLCommandBuffer> end=[r->queue commandBuffer];[end commit];[end waitUntilCompleted];if(!t||(t.pixelFormat!=MTLPixelFormatRGBA16Float&&t.pixelFormat!=MTLPixelFormatRGBA8Unorm&&t.pixelFormat!=MTLPixelFormatBGRA8Unorm))return false;
 size_t n=t.width*t.height*4;uint16_t *half=calloc(n,2);float *full=calloc(n,4);if(!half||!full){free(half);free(full);return false;}
 if(t.pixelFormat==MTLPixelFormatRGBA16Float){[t getBytes:half bytesPerRow:t.width*8 fromRegion:MTLRegionMake2D(0,0,t.width,t.height) mipmapLevel:0];for(size_t i=0;i<n;i++){_Float16 v;memcpy(&v,&half[i],2);full[i]=(float)v;}}
 else{uint8_t *bytes=(uint8_t*)half;[t getBytes:bytes bytesPerRow:t.width*4 fromRegion:MTLRegionMake2D(0,0,t.width,t.height) mipmapLevel:0];for(size_t i=0;i<n;i++){size_t lane=i%4,at=t.pixelFormat==MTLPixelFormatBGRA8Unorm&&(lane==0||lane==2)?i+2-2*lane:i;full[i]=(float)bytes[at]/255.0f;}}
 NSString *output=[NSString stringWithUTF8String:path];BOOL ok=[[NSData dataWithBytes:full length:n*4]writeToFile:output atomically:YES];
 CGDataProviderRef provider=CGDataProviderCreateWithData(NULL,full,n*4,NULL);CGColorSpaceRef colour=CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
 CGImageRef image=CGImageCreate(t.width,t.height,32,128,t.width*16,colour,kCGBitmapFloatComponents|kCGBitmapByteOrder32Little|kCGImageAlphaLast,provider,NULL,NO,kCGRenderingIntentDefault);
 if(image){NSURL *url=[NSURL fileURLWithPath:[[output stringByDeletingPathExtension]stringByAppendingPathExtension:@"png"]];CGImageDestinationRef destination=CGImageDestinationCreateWithURL((__bridge CFURLRef)url,CFSTR("public.png"),1,NULL);if(destination){CGImageDestinationAddImage(destination,image,NULL);CGImageDestinationFinalize(destination);CFRelease(destination);}CGImageRelease(image);}
 CGColorSpaceRelease(colour);CGDataProviderRelease(provider);free(half);free(full);return ok;
}
bool mf_snapshot(void *host,const char *path){MFRealm *r=(__bridge MFRealm*)host;return snapshotTexture(r,r->targets[@(COAX_RenderTarget_screen)],path);}
bool mf_snapshot_presented(void *host,const char *path){MFRealm *r=(__bridge MFRealm*)host;return r->presentationExperiment&&snapshotTexture(r,r->presentedTexture,path);}
bool mf_snapshot_presented_raw(void *host,const char *path){
 MFRealm *r=(__bridge MFRealm*)host;id<MTLTexture> t=r->presentedTexture;if(!r->presentationExperiment||!t||(t.pixelFormat!=MTLPixelFormatBGRA8Unorm&&t.pixelFormat!=MTLPixelFormatRGBA8Unorm))return false;
 id<MTLCommandBuffer> end=[r->queue commandBuffer];[end commit];[end waitUntilCompleted];if(end.error)return false;
 NSMutableData *raw=[NSMutableData dataWithLength:t.width*t.height*4];[t getBytes:raw.mutableBytes bytesPerRow:t.width*4 fromRegion:MTLRegionMake2D(0,0,t.width,t.height) mipmapLevel:0];return [raw writeToFile:[NSString stringWithUTF8String:path] atomically:YES];
}
bool mf_media_snapshot(void *host,const char *url,const char *path){
 MFRealm *r=(__bridge MFRealm*)host;MFMedia *m=r->mediaCache[[NSString stringWithUTF8String:url]];if(!m||!m->texture||!live(r,m->handle)||m->texture.pixelFormat!=MTLPixelFormatRGBA8Unorm)return false;
 id<MTLCommandBuffer> end=[r->queue commandBuffer];[end commit];[end waitUntilCompleted];if(end.error)return false;
 NSMutableData *raw=[NSMutableData dataWithLength:m->texture.width*m->texture.height*4];[m->texture getBytes:raw.mutableBytes bytesPerRow:m->texture.width*4 fromRegion:MTLRegionMake2D(0,0,m->texture.width,m->texture.height) mipmapLevel:0];return [raw writeToFile:[NSString stringWithUTF8String:path] atomically:YES];
}
const uint8_t *mf_media_notice(void *host,size_t *count){MFRealm *r=(__bridge MFRealm*)host;*count=r->lastMediaNotice.length;return r->lastMediaNotice.bytes;}
int64_t mf_media_info(void *host,const char *url,int32_t field){MFRealm *r=(__bridge MFRealm*)host;MFMedia *m=r->mediaCache[[NSString stringWithUTF8String:url]];if(!m)return 0;switch(field){case 0:return 1;case 1:return m->texture&&!m->failed;case 2:return m->failed;case 3:return m->width;case 4:return m->height;default:return -1;}}
const uint8_t *mf_frame_jpeg(void *host,size_t *count){
 *count=0;MFRealm *r=(__bridge MFRealm*)host;id<MTLTexture> t=r->targets[@(COAX_RenderTarget_screen)];if(!t||t.pixelFormat!=MTLPixelFormatBGRA8Unorm)return NULL;
 id<MTLCommandBuffer> end=[r->queue commandBuffer];[end commit];[end waitUntilCompleted];if(end.error)return NULL;
 coax_t_HostImageExtent size=hpi_extent(t.width,t.height);if(size.width<1||size.height<1)return NULL;
 NSMutableData *pixels=[NSMutableData dataWithLength:t.width*t.height*4];[t getBytes:pixels.mutableBytes bytesPerRow:t.width*4 fromRegion:MTLRegionMake2D(0,0,t.width,t.height) mipmapLevel:0];
 CGColorSpaceRef colour=CGColorSpaceCreateWithName(kCGColorSpaceSRGB);CGDataProviderRef provider=CGDataProviderCreateWithCFData((__bridge CFDataRef)pixels);CGImageRef image=CGImageCreate(t.width,t.height,8,32,t.width*4,colour,kCGBitmapByteOrder32Little|kCGImageAlphaNoneSkipFirst,provider,NULL,NO,kCGRenderingIntentDefault);
 CGContextRef scaled=CGBitmapContextCreate(NULL,size.width,size.height,8,size.width*4,colour,kCGBitmapByteOrder32Big|kCGImageAlphaNoneSkipLast);NSMutableData *jpeg=[NSMutableData data];
 if(image&&scaled){CGContextSetInterpolationQuality(scaled,kCGInterpolationHigh);CGContextDrawImage(scaled,CGRectMake(0,0,size.width,size.height),image);CGImageRef output=CGBitmapContextCreateImage(scaled);CGImageDestinationRef destination=CGImageDestinationCreateWithData((__bridge CFMutableDataRef)jpeg,CFSTR("public.jpeg"),1,NULL);if(output&&destination){CGImageDestinationAddImage(destination,output,(__bridge CFDictionaryRef)@{(__bridge NSString*)kCGImageDestinationLossyCompressionQuality:@(size.quality)});if(!CGImageDestinationFinalize(destination))[jpeg setLength:0];}if(destination)CFRelease(destination);if(output)CGImageRelease(output);}
 if(scaled)CGContextRelease(scaled);if(image)CGImageRelease(image);CGDataProviderRelease(provider);CGColorSpaceRelease(colour);if(!jpeg.length)return NULL;
 r->frameImage=[[jpeg base64EncodedStringWithOptions:0]dataUsingEncoding:NSUTF8StringEncoding];*count=r->frameImage.length;return r->frameImage.bytes;
}
int64_t mf_http_image(const uint8_t *body,size_t body_count,const uint8_t *image,size_t image_count,uint8_t *out,size_t out_count){return hpi_attach((coax_string){(const char*)body,(int64_t)body_count},(coax_string){(const char*)image,(int64_t)image_count},(coax_v_u8){out,(int64_t)out_count});}
void mf_close(void *host){MFRealm *r=CFBridgingRelease(host);[r shutdown];if(activeRealm==r)activeRealm=nil;}
int64_t mf_closed_info(int32_t field){return field==0?lastClosedLive:field==1?lastClosedWords:-1;}
void mf_network_config(void *host,const char *page,const char *local){MFRealm *r=(__bridge MFRealm*)host;r->pageOrigin=page?[NSString stringWithUTF8String:page]:nil;r->localOrigin=local?[NSString stringWithUTF8String:local]:nil;}
void mf_network_reset(void *host){MFRealm *r=(__bridge MFRealm*)host;ht_reset(net(r));if(r->socketContext){networkAPI.close(r->socketContext);r->socketContext=NULL;}}
int64_t mf_network_poll(void *host){MFRealm *r=(__bridge MFRealm*)host;if(!r->socketContext)return 0;int64_t handled=0,start=milliseconds();MNSocketEvent e;
 while(networkAPI.poll_more(handled,start,milliseconds())&&networkAPI.next(r->socketContext,&e)){mf_socket_event(host,e.owner.id,e.owner.generation,e.owner.engine,(int32_t)e.event,e.bytes,e.count);networkAPI.done(e.token);handled++;}return handled;}
int64_t mf_network_info(void *host,int32_t field){MFRealm *r=(__bridge MFRealm*)host;return r->socketContext?networkAPI.info(r->socketContext,field):0;}
const uint8_t *mf_sound_bank(void *host,size_t *n){MFRealm *r=(__bridge MFRealm*)host;*n=0;if(!r->face.sound_bank||!r->face.sound_bank_len||!r->face.sound_bank_capacity)return NULL;size_t count=r->face.sound_bank_len(r->state),capacity=r->face.sound_bank_capacity(r->state);uintptr_t p=r->face.sound_bank(r->state);if(!htb_copy_span(p,capacity,count,UINT64_MAX))return NULL;*n=count;return(void*)p;}
const uint8_t *mf_sound_bank_storage(void *host,size_t *n){MFRealm *r=(__bridge MFRealm*)host;*n=0;if(!r->face.sound_bank_storage||!r->face.sound_bank_storage_bytes)return NULL;uintptr_t p=r->face.sound_bank_storage(r->state);size_t capacity=r->face.sound_bank_storage_bytes(r->state);if(!htb_copy_span(p,capacity,0,UINT64_MAX))return NULL;*n=capacity;return(void*)p;}
typedef struct {coax_t_HostAudioBank state;uintptr_t base;size_t capacity;} MFAudioBank;
void *mf_audio_bank_open(const uint8_t *base,size_t capacity){MFAudioBank *p=calloc(1,sizeof*p);if(p){hab_reset((coax_v_t_HostAudioBank){&p->state,1});p->base=(uintptr_t)base;p->capacity=capacity;}return p;}
bool mf_audio_bank_register(void *bank,const uint8_t *line,size_t count,MFAudioBankAsset *out){MFAudioBank *p=bank;*out=(MFAudioBankAsset){0};if(!p||count>INT64_MAX)return false;coax_v_t_HostAudioBank s={&p->state,1};int64_t id=hab_insert(s,(coax_string){(const char*)line,(int64_t)count},p->base,p->capacity);if(id<0)return false;coax_t_HostAudioBankSpan q=hab_source(s,id);if(!q.valid)return false;*out=(MFAudioBankAsset){(const uint8_t*)q.name.data,(size_t)q.name.count,(const uint8_t*)(uintptr_t)q.address,(size_t)q.bytes};return true;}
int64_t mf_audio_bank_count(void *bank){MFAudioBank *p=bank;return p?p->state.count:0;}
void mf_audio_bank_close(void *bank){free(bank);}
int64_t mf_audio_extent(int64_t frames,int64_t channels){return ham_extent(frames,channels);}
void *mf_audio_memory_open(void){return calloc(1,sizeof(coax_t_HostAudioMemory));}
int64_t mf_audio_memory_reserve(void *memory,int64_t words){return memory?ham_reserve((coax_v_t_HostAudioMemory){memory,1},words):-1;}
int64_t mf_audio_memory_words(void *memory,int64_t ticket){return memory?ham_words((coax_v_t_HostAudioMemory){memory,1},ticket):-1;}
bool mf_audio_memory_release(void *memory,int64_t ticket){return memory&&ham_release((coax_v_t_HostAudioMemory){memory,1},ticket);}
int64_t mf_audio_memory_total(void *memory){return memory?((coax_t_HostAudioMemory*)memory)->total:0;}
void mf_audio_memory_close(void *memory){free(memory);}
const uint8_t *mf_sound_commands(void *host,size_t *n){MFRealm *r=(__bridge MFRealm*)host;*n=0;if(!r->face.sounds||!r->face.sounds_len||!r->face.sounds_capacity)return NULL;size_t count=r->face.sounds_len(r->state),capacity=r->face.sounds_capacity(r->state);uintptr_t p=r->face.sounds(r->state);if(!htb_copy_span(p,capacity,count,UINT64_MAX))return NULL;*n=count;return(void*)p;}
int32_t mf_sound_probe(void *host,int32_t slot,float right,float ahead,float up){MFRealm *r=(__bridge MFRealm*)host;return r->face.sound_probe?r->face.sound_probe(r->state,slot,right,ahead,up):0;}
bool mf_audio_decode(const uint8_t *bytes,size_t n,MFAudioEffect *out){coax_t_AudioHostEffect e=audio_host_parse((coax_string){(const char*)bytes,(int64_t)n});*out=(MFAudioEffect){e.kind,(const uint8_t*)e.name.data,(size_t)e.name.count,(const uint8_t*)e.id.data,(size_t)e.id.count,e.gain,e.pan,e.delay,e.lowpass,e.rate,e.wet};return e.kind!=0;}
bool mf_audio_asset_name(const uint8_t *bytes,size_t n){return apv_asset_name((coax_string){(const char*)bytes,(int64_t)n});}
typedef struct {coax_t_AudioPcmHost state;double *workspace;int64_t words;float block[2048];} MFAudioPCM;
void *mf_pcm_open(void){MFAudioPCM *p=calloc(1,sizeof*p);if(!p)return NULL;coax_default_t_AudioPcmHost(&p->state);p->words=ac_words(1024,128);p->workspace=calloc((size_t)p->words,sizeof(double));if(!p->workspace){free(p);return NULL;}return p;}
int64_t mf_pcm_command(void *pcm,const uint8_t *line,size_t n,const float *samples,size_t count,int64_t channels,bool started){MFAudioPCM *p=pcm;if(!p)return -1;return ap_context_command((coax_v_t_AudioPcmHost){&p->state,1},(coax_v_f64){p->workspace,p->words},(coax_string){(const char*)line,(int64_t)n},(coax_v_f32){(float*)samples,(int64_t)count},channels,started);}
const float *mf_pcm_block(void *pcm,size_t *samples){MFAudioPCM *p=pcm;*samples=0;if(!p||!ap_block((coax_v_t_AudioPcmHost){&p->state,1},(coax_v_f64){p->workspace,p->words},(coax_v_f32){p->block,2048}))return NULL;*samples=2048;return p->block;}
int64_t mf_pcm_info(void *pcm,int32_t field){MFAudioPCM *p=pcm;if(!p)return 0;switch(field){case 0:return p->state.frames;case 1:return p->state.accepted;case 2:return p->state.refused;case 3:return p->state.room.e[0].parts;case 4:{int64_t n=0;for(int i=0;i<128;i++)if(p->state.voices.e[i].active)n++;return n;}default:return 0;}}
void mf_pcm_close(void *pcm){MFAudioPCM *p=pcm;if(p){free(p->workspace);free(p);}}
typedef struct {coax_t_AudioWavFilter state;double workspace[20480];} MFAudioWav;
void *mf_wav_open(void){MFAudioWav *p=calloc(1,sizeof*p);if(p&&!apw_coefficients((coax_v_t_AudioWavFilter){&p->state,1},(coax_v_f64){p->workspace,20480})){free(p);return NULL;}return p;}
int64_t mf_wav_info(const uint8_t *bytes,size_t count,int32_t field){coax_t_AudioWavPlan p=apw_plan((coax_v_u8){(uint8_t*)bytes,(int64_t)count});if(!p.valid)return -1;return field==0?p.samples:field==1?p.channels:field==2?p.output_frames:p.rate;}
bool mf_wav_decode(void *decoder,const uint8_t *bytes,size_t count,float *samples,size_t sample_count){MFAudioWav *p=decoder;return p&&apw_decode((coax_v_t_AudioWavFilter){&p->state,1},(coax_v_u8){(uint8_t*)bytes,(int64_t)count},(coax_v_f64){p->workspace,20480},(coax_v_f32){samples,(int64_t)sample_count});}
void mf_wav_close(void *decoder){free(decoder);}

// Imports copy borrowed producer bytes while that call owns them. Concrete
// network callbacks are queued by the platform and fenced again on delivery.
int64_t guard_now_ms(void){const char *fixed=getenv("VERSE_NATIVE_CLOCK_MS");int64_t observed;if(fixed&&parseNumber(fixed,&observed))return observed;return milliseconds();}
double coaxial_measure_ms(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return(double)t.tv_sec*1000+(double)t.tv_nsec/1000000;}
int32_t coaxial_socket_open(uintptr_t address,uint32_t length){MFRealm *r=activeRealm;if(!r)return -1;NSString *url=[[NSString alloc]initWithBytes:(void*)address length:length encoding:NSUTF8StringEncoding];if(!url)return -1;
 if([url hasPrefix:@"local:"]){uint8_t target[8192];int64_t n=htr_local_target(cs(url),cs(r->localOrigin?:@""),(coax_v_u8){target,sizeof target});if(n<0)return -1;url=[[NSString alloc]initWithBytes:target length:(NSUInteger)n encoding:NSUTF8StringEncoding];}
 NSURL *base=r->pageOrigin?[NSURL URLWithString:r->pageOrigin]:nil,*resolved=[NSURL URLWithString:url relativeToURL:base];if(!resolved)return -1;NSURLComponents *parts=[NSURLComponents componentsWithURL:resolved resolvingAgainstBaseURL:YES];if([parts.scheme isEqual:@"http"])parts.scheme=@"ws";if([parts.scheme isEqual:@"https"])parts.scheme=@"wss";if(!parts.host.length||parts.fragment||(![parts.scheme isEqual:@"ws"]&&![parts.scheme isEqual:@"wss"])||!parts.URL)return -1;
 if(!socketSDK(r))return -1;NSData *target=[parts.URL.absoluteString dataUsingEncoding:NSUTF8StringEncoding];if(!networkAPI.uri(target.bytes,target.length))return -1;coax_t_HostOwner o=ht_socket_reserve(net(r));if(o.id<0)return -1;r->socketOwners[o.id]=o;networkAPI.start(r->socketContext,(MNSocketOwner){o.id,o.generation,o.engine},target.bytes,target.length);return(int32_t)o.id;}
void coaxial_socket_send(int32_t id,uintptr_t address,uint32_t length){MFRealm *r=activeRealm;if(!r||!r->socketContext||id<0||id>=4)return;coax_t_HostOwner o=r->socketOwners[id];if(ht_socket_send(net(r),o))networkAPI.send(r->socketContext,(MNSocketOwner){o.id,o.generation,o.engine},(void*)address,length);}
void coaxial_socket_close(int32_t id){MFRealm *r=activeRealm;if(!r||!r->socketContext||id<0||id>=4)return;coax_t_HostOwner o=r->socketOwners[id];if(ht_socket_close(net(r),o))networkAPI.socket_close(r->socketContext,(MNSocketOwner){o.id,o.generation,o.engine});}
static int32_t http_import(uintptr_t address,uint32_t length,uintptr_t body,uint32_t bodyLength,int32_t flags,int64_t timeout,BOOL bounded){MFRealm *r=activeRealm;if(!r)return -1;NSString *path=[[NSString alloc]initWithBytes:(void*)address length:length encoding:NSUTF8StringEncoding],*payload=[[NSString alloc]initWithBytes:(void*)body length:bodyLength encoding:NSUTF8StringEncoding];if(!path||!payload||!r->pageOrigin)return -1;
 uint8_t target[12288];int64_t n=htr_http_target(cs(path),(coax_v_u8){target,sizeof target});if(n<0)return -1;NSString *normalized=[[NSString alloc]initWithBytes:target length:(NSUInteger)n encoding:NSUTF8StringEncoding];
 NSURL *base=[NSURL URLWithString:r->pageOrigin],*url=[NSURL URLWithString:[r->pageOrigin stringByAppendingString:normalized]];if(!url||![url.scheme isEqual:base.scheme]||![url.host.lowercaseString isEqual:base.host.lowercaseString]||!(url.port==base.port||[url.port isEqual:base.port]))return -1;
 if((flags&16)&&[payload rangeOfString:@"\n"].location==NSNotFound)return -1;
 coax_t_HostOwner o=ht_http_reserve(net(r),flags,bounded,timeout,milliseconds(),r->face.inbox_cap());if(((flags&4)||bounded)&&o.id<0)return -1;effect(r,@"http",o,@{@"url":url.absoluteString,@"body":payload,@"flags":@(flags),@"bounded":@(bounded),@"timeout":@(timeout),@"capacity":@(r->face.inbox_cap())});return o.id<0?0:(int32_t)o.id;}
int32_t coaxial_http(uintptr_t a,uint32_t b,uintptr_t c,uint32_t d,int32_t flags){return http_import(a,b,c,d,flags,0,NO);}
int32_t guard_http(uintptr_t a,uint32_t b,uintptr_t c,uint32_t d,uint32_t flags,int64_t timeout){return http_import(a,b,c,d,(int32_t)flags,timeout,YES);}
int32_t guard_cancel(int32_t id){MFRealm *r=activeRealm;if(!r||!ht_http_cancel(net(r),id))return 0;effect(r,@"http-cancel",(coax_t_HostOwner){.id=id},@{});return 1;}
int32_t coaxial_http_bounded(uintptr_t a,uint32_t b,uintptr_t c,uint32_t d,uint32_t flags,int64_t timeout){return guard_http(a,b,c,d,flags,timeout);}
int32_t coaxial_http_cancel(int32_t id){return guard_cancel(id);}
int64_t coaxial_now_ms(void){return guard_now_ms();}
void host_line(int64_t h,double ax,double ay,double bx,double by,double a,double b,double w){(void)h;(void)ax;(void)ay;(void)bx;(void)by;(void)a;(void)b;(void)w;}
void host_dot(int64_t h,double x,double y,double s,double l,double soft,double crisp){(void)h;(void)x;(void)y;(void)s;(void)l;(void)soft;(void)crisp;}
void host_slot(int64_t h,double x,double y,double w,double ht,double p,double f,double s,double i){(void)h;(void)x;(void)y;(void)w;(void)ht;(void)p;(void)f;(void)s;(void)i;}
