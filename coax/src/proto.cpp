// Host boundary for the Coax descriptor importer: files, owned buffers and
// allocation only. Parsing, generation and fingerprinting are checked Coax.
#include <cstdint>
#include <cstdio>
#include <string>
#include <vector>
#include <memory>
#include <string_view>
#ifndef COAX_BOOTSTRAP_IMPORTER
#include "proto_importer.h"
#endif
namespace coax::proto {
using View = std::string_view;
#include "proto_runtime.inc"
struct Result { std::string source, provenance, error; };
static Result import_descriptor(View raw,const std::string &prefix,const std::string &compiler) {
#ifdef COAX_BOOTSTRAP_IMPORTER
  (void)raw;(void)prefix;(void)compiler;
  return {{},{},"proto imports are disabled in the build-only stage0 compiler"};
#else
  Result result;
  try {
    const auto caps=D_capacities();
    const auto limits=F_capacities();
    std::vector<coax_t_D_Type> types((size_t)caps.types), enums((size_t)caps.types);
    std::vector<coax_t_D_Field> fields((size_t)caps.fields);
    std::vector<coax_t_D_Value> values((size_t)caps.values);
    std::vector<coax_string> groups((size_t)caps.groups), files((size_t)caps.files), deps((size_t)caps.deps), symbols((size_t)caps.symbols);
    // Allocate without clearing: Coax writes every byte before exposing it.
    std::unique_ptr<uint8_t[]> arena(new uint8_t[(size_t)caps.arena]), output(new uint8_t[(size_t)caps.output]);
    coax_t_D_State state{};
    state.types={types.data(),(int64_t)types.size()};state.enums={enums.data(),(int64_t)enums.size()};
    state.fields={fields.data(),(int64_t)fields.size()};state.values={values.data(),(int64_t)values.size()};
    state.groups={groups.data(),(int64_t)groups.size()};state.files={files.data(),(int64_t)files.size()};
    state.deps={deps.data(),(int64_t)deps.size()};state.symbols={symbols.data(),(int64_t)symbols.size()};
    state.arena={arena.get(),caps.arena};state.output={output.get(),caps.output};
    std::unique_ptr<uint8_t[]> runtime(new uint8_t[(size_t)limits.runtime]), final_source(new uint8_t[(size_t)limits.source]);
    const auto runtime_result=F_runtime({PROTO_CORE_SOURCE,sizeof(PROTO_CORE_SOURCE)-1},
        {PROTO_MEMORY_SOURCE,sizeof(PROTO_MEMORY_SOURCE)-1},{runtime.get(),limits.runtime});
    if(runtime_result.error.count) {result.error.assign(runtime_result.error.data,(size_t)runtime_result.error.count);return result;}
    const coax_string runtime_input={(const char*)runtime.get(),runtime_result.written};
    const coax_string raw_input={raw.data(),(int64_t)raw.size()}, prefix_input={prefix.data(),(int64_t)prefix.size()};
    int64_t count=G_generate({&state,1},raw_input,prefix_input,runtime_input);
    if(state.error.count) {result.error.assign(state.error.data,(size_t)state.error.count);return result;}
    const auto packaged=F_package(raw_input,prefix_input,{compiler.data(),(int64_t)compiler.size()},runtime_input,
        {PROTO_IMPORTER_SOURCE,sizeof(PROTO_IMPORTER_SOURCE)-1},
        {PROTO_FINGERPRINT_SOURCE,sizeof(PROTO_FINGERPRINT_SOURCE)-1},
        {(const char*)output.get(),count},{final_source.get(),limits.source});
    if(packaged.error.count) {result.error.assign(packaged.error.data,(size_t)packaged.error.count);return result;}
    result.source.assign((const char*)final_source.get(),(size_t)packaged.written);
    result.provenance.assign((const char*)final_source.get(),(size_t)packaged.provenance);
  } catch(const std::bad_alloc &) {result.source.clear();result.provenance.clear();result.error="descriptor allocation failed";}
  return result;
#endif
}
static Result import_file(const std::string &path,const std::string &prefix,const std::string &compiler) {
#ifdef COAX_BOOTSTRAP_IMPORTER
  (void)path;
  return import_descriptor({},prefix,compiler);
#else
  const size_t capacity=(size_t)F_capacities().descriptor;
  FILE *file=fopen(path.c_str(),"rb");
  if (!file) return {{},{},"cannot read proto descriptor: "+path};
  std::string raw;
  char buffer[65536];
  size_t count;
  while ((count=fread(buffer,1,sizeof buffer,file))!=0) {
    if (count>capacity-raw.size()) {
      fclose(file);
      return {{},{},"descriptor byte budget exceeded"};
    }
    raw.append(buffer,count);
  }
  bool ok=!ferror(file);
  fclose(file);
  if (!ok) return {{},{},"cannot read proto descriptor: "+path};
  return import_descriptor(raw,prefix,compiler);
#endif
}
} // namespace coax::proto
