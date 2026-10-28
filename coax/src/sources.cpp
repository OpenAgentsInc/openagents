// Filesystem/storage transport only. lib/compiler/sources.coax owns syntax,
// canonical-identity admission, cycles and graph budgets. The lexer supplies
// decoded tokens, so comments and string contents cannot become directives.
#include "coax.h"
#include <algorithm>
#include <filesystem>
#include <functional>
#ifdef COAX_BOOTSTRAP_IMPORTER
#include "sources.h"
#else
#include "proto_importer.h"
#endif
namespace coax {
static_assert((uint8_t)Tok::directive == 6 && (uint8_t)Tok::string_lit == 5 &&
              (uint8_t)Tok::semicolon == 15 && (uint8_t)Tok::lparen == 8 &&
              (uint8_t)Tok::rparen == 9 && (uint8_t)Tok::lbrace == 10 &&
              (uint8_t)Tok::rbrace == 11 && (uint8_t)Tok::lbracket == 12 &&
              (uint8_t)Tok::rbracket == 13, "source token ABI");

SourcePoint source_point(const Source *src, Loc loc) {
  loc = diagnostic_loc(src, loc);
  for (const auto &unit : src->units) {
    if (loc.offset >= unit->begin && (loc.offset < unit->end || (unit==src->units.back() && loc.offset==unit->end))) {
      loc.offset -= unit->begin;
      loc.line -= unit->first_line - 1;
      return {unit->path, loc};
    }
  }
  return {src->path, loc};
}
bool source_authority(const Source *src, Loc loc) {
  loc = diagnostic_loc(src, loc);
  for (const auto &unit : src->units)
    if (loc.offset >= unit->begin && loc.offset < unit->end) return unit->authority;
  return false;
}

// Validate complete physical units with the existing Coax grammar kernel.
// Source/proto directives are already parsed by their checked Coax scanners;
// this bridge removes only those token spans, never parses declarations itself.
static bool source_unit_syntax(const Source *source,const std::vector<Token> &tokens,
                               const std::vector<coax_t_SU_Load> &loads,int64_t load_count) {
  auto input=parser_tokens(tokens);auto caps=PS_caps((int64_t)input.size(),(int64_t)source->text.size());
  std::vector<coax_t_PS_Import> imports((size_t)caps.imports);
  auto scan=PS_imports({input.data(),(int64_t)input.size()},{imports.data(),(int64_t)imports.size()},false);
  if (scan.code.count) {
    error_at(source,parser_loc(scan.loc),std::string(scan.code.data,(size_t)scan.code.count).c_str(),
             std::string(scan.message.data,(size_t)scan.message.count));return false;
  }
  std::vector<Token> kept;
  for (const auto &token:tokens) {
    bool suppressed=false;
    for(int64_t n=0;n<load_count;n++)suppressed=suppressed || (token.loc.offset>=loads[(size_t)n].begin && token.loc.offset<loads[(size_t)n].end);
    for(int64_t n=0;n<scan.count;n++)suppressed=suppressed || (token.loc.offset>=imports[(size_t)n].loc.offset && token.loc.offset<imports[(size_t)n].end);
    if(!suppressed || token.kind==Tok::eof)kept.push_back(token);
  }
  input=parser_tokens(kept);caps=PS_caps((int64_t)input.size(),(int64_t)source->text.size());
  std::vector<coax_t_PS_Node> nodes((size_t)caps.nodes);std::vector<uint8_t> message((size_t)caps.message);coax_t_PS_State state{};
  auto parsed=PS_parse({source->text.data(),(int64_t)source->text.size()},{input.data(),(int64_t)input.size()},
                       {nodes.data(),(int64_t)nodes.size()},{&state,1},{message.data(),(int64_t)message.size()});
  if(!parsed.ok) {
    error_at(source,parser_loc(state.loc),std::string(state.code.data,(size_t)state.code.count).c_str(),
             std::string((char*)message.data(),(size_t)state.length),std::string(state.fix.data?state.fix.data:"",(size_t)state.fix.count));
  }
  return parsed.ok;
}

static bool load_sources(Arena *arena, Source *src, const char *path) {
  struct Unit {
    std::shared_ptr<SourceUnit> source;
    std::string masked;
    uint8_t state = 1;
    bool balanced = true;
  };
  std::vector<std::shared_ptr<Unit>> units;
  int64_t total = 0;
  bool composition = false;
  src->path = path;
  std::function<bool(const std::string &, int64_t, const Source *, Loc)> visit;
  visit = [&](const std::string &display, int64_t depth, const Source *parent, Loc directive) {
    std::error_code ec;
    auto canonical = std::filesystem::canonical(display, ec);
    if (ec || !std::filesystem::is_regular_file(canonical, ec)) {
      error_at(parent, directive, "E0601", "cannot read source unit '" + display + "'");
      return false;
    }
    const std::string key = canonical.string();
    std::vector<coax_string> names; std::vector<uint8_t> states;
    for (const auto &u : units) {names.push_back({u->source->canonical.data(),(int64_t)u->source->canonical.size()});states.push_back(u->state);}
    auto found = SU_visit({names.data(),(int64_t)names.size()}, {states.data(),(int64_t)states.size()}, {key.data(),(int64_t)key.size()});
    if (found.action == 1) return true;
    if (found.action == 2) {
      error_at(parent,directive,"E0602","source cycle loads active unit '" + display + "'");
      return false;
    }
    auto bytes = std::filesystem::file_size(canonical,ec);
    if (ec || bytes > INT64_MAX || !SU_admit((int64_t)units.size(),depth,total,(int64_t)bytes)) {
      error_at(parent,directive,"E0603","source graph exceeds its unit, depth or byte budget");
      return false;
    }
    auto u = std::make_shared<Unit>();u->source=std::make_shared<SourceUnit>();
    u->source->path=display;u->source->canonical=key;
    FILE *f = fopen(key.c_str(),"rb");
    if (!f) {error_at(parent,directive,"E0601","cannot open source unit '"+display+"'");return false;}
    char buffer[65536];size_t n;
    while ((n=fread(buffer,1,sizeof buffer,f))>0) {
      if (!SU_admit((int64_t)units.size(),depth,total,(int64_t)u->source->text.size()+(int64_t)n)) {
        fclose(f);error_at(parent,directive,"E0603","source bytes grew beyond the graph budget");return false;
      }
      u->source->text.append(buffer,n);
    }
    bool read_ok=!ferror(f);fclose(f);
    if (!read_ok) {error_at(parent,directive,"E0601","failed reading source unit '"+display+"'");return false;}
    total+=(int64_t)u->source->text.size();units.push_back(u);
    Source local;local.path=display;local.text=u->source->text;
    std::vector<Token> tokens;
    if (!lex(arena,&local,&tokens)) return false;
    std::vector<coax_t_SU_Token> records;
    for (const auto &t : tokens) {
      const auto text=t.kind==Tok::string_lit?t.string_value:t.text;
      records.push_back({(uint32_t)t.kind,{text.data,text.count},t.loc.offset,t.loc.offset+t.text.count});
    }
    std::vector<coax_t_SU_Load> loads(512);
    auto scan=SU_scan({records.data(),(int64_t)records.size()},{loads.data(),(int64_t)loads.size()});
    if (scan.error) {
      Loc at{1,1,(int32_t)scan.offset};
      for (const auto &t:tokens) if (t.loc.offset==at.offset) {at=t.loc;break;}
      error_at(&local,at,scan.error==2?"E0603":"E0600",scan.error==2?"too many source loads in one unit":"#load requires a top-level nonempty NUL-free string path followed by ';'");
      return false;
    }
    u->source->authority=scan.authority;u->balanced=scan.balanced;u->masked=local.text;
    composition=composition || scan.count!=0;
    if (composition && !u->balanced) {
      error_at(&local,{1,1,0},"E0604","source delimiters must balance within a unit and fit depth 128");return false;
    }
    if (composition && !source_unit_syntax(&local,tokens,loads,scan.count)) return false;
    // Keep the original bytes for diagnostics; only the compiler's flat input is blanked.
    for (int64_t i=0;i<scan.count;i++) {
      const auto &r=loads[(size_t)i];
      for (int64_t at=r.begin;at<r.end;at++) if (u->masked[(size_t)at]!='\n' && u->masked[(size_t)at]!='\r')u->masked[(size_t)at]=' ';
      std::filesystem::path next(std::string(r.path.data,(size_t)r.path.count));
      if (next.is_relative()) next=std::filesystem::path(display).parent_path()/next;
      Loc at{1,1,(int32_t)r.begin};for(const auto &t:tokens)if(t.loc.offset==at.offset){at=t.loc;break;}
      if (!visit(next.lexically_normal().string(),depth+1,&local,at)) return false;
    }
    u->state=2;return true;
  };
  if (!visit(path,0,src,{1,1,0})) return false;
  int32_t line=1;
  for (const auto &u:units) {
    u->source->begin=(int32_t)src->text.size();u->source->first_line=line;
    src->text+=u->masked;
    // A physical boundary cannot join tokens or extend a line comment into the next unit.
    if (composition && (src->text.empty() || src->text.back()!='\n'))src->text+='\n';
    line+=(int32_t)std::count(src->text.begin()+u->source->begin,src->text.end(),'\n');
    u->source->end=(int32_t)src->text.size();src->units.push_back(u->source);
  }
  return true;
}
} // namespace coax
