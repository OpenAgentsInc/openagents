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
const SourceUnit *source_unit_at(const Source *src, Loc loc) {
  loc = diagnostic_loc(src, loc);
  for (const auto &unit : src->units)
    if (loc.offset >= unit->begin && loc.offset < unit->end) return unit.get();
  return nullptr;
}
// The checked tuple encoding (SU_decl_id); native code only transports it.
std::string declaration_id(const Source *src, uint8_t kind, Str name, Loc loc) {
  const SourceUnit *unit = source_unit_at(src, loc);
  if (!unit || unit->module.empty()) return "";
  uint8_t out[512];
  int64_t n = SU_decl_id({unit->module.data(), (int64_t)unit->module.size()}, kind, {name.data, name.count}, {out, (int64_t)sizeof out});
  return n < 0 ? "" : std::string((const char *)out, (size_t)n);
}

// Validate complete physical units with the existing Coax grammar kernel.
// Source/proto directives are already parsed by their checked Coax scanners;
// this bridge removes only those token spans, never parses declarations itself.
static bool source_unit_syntax(const Source *source,const std::vector<Token> &tokens,
                               const std::vector<coax_t_SU_Load> &loads,int64_t load_count,const coax_t_SU_Scan &unit,
                               const std::vector<coax_t_SU_Export> &exports,int64_t export_count) {
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
    suppressed=suppressed || (unit.claims && token.loc.offset>=unit.owner_begin && token.loc.offset<unit.owner_end);
    for(int64_t n=0;n<export_count;n++)suppressed=suppressed || (token.loc.offset>=exports[(size_t)n].begin && token.loc.offset<exports[(size_t)n].end);
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
    std::vector<std::pair<int64_t,int64_t>> loaders; // (loading unit, directive offset there)
  };
  std::vector<std::shared_ptr<Unit>> units;
  int64_t total = 0;
  bool composition = false;
  std::vector<int64_t> finished; // units in the order their loads completed
  src->path = path;
  std::function<bool(const std::string &, int64_t, const Source *, Loc, int64_t)> visit;
  visit = [&](const std::string &display, int64_t depth, const Source *parent, Loc directive, int64_t from) {
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
    if (found.action == 1) {
      units[(size_t)found.index]->loaders.push_back({from,directive.offset});
      if (from>=0) units[(size_t)from]->source->loads.push_back((int32_t)found.index);
      return true;
    }
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
    const int64_t self=(int64_t)units.size()-1;
    if (from>=0) u->loaders.push_back({from,directive.offset});
    if (from>=0) units[(size_t)from]->source->loads.push_back((int32_t)self);
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
      if (scan.error==4) error_at(&local,at,"E0600","#load \"FILE.coax\" as NAME requires an identifier alias followed by ';'","#load \"hud.coax\" as hud;");
      else if (scan.error==5) error_at(&local,at,"E0608","module parameters are not supported","load the module as written: #load \"hud.coax\" as hud;");
      else if (scan.error==6) error_at(&local,at,"E0609","an aliased #load names a .coax file path; package search is not supported","write the relative path: #load \"lib/hud.coax\" as hud;");
      else if (scan.error==3) error_at(&local,at,"E0605","#module requires one top-level lowercase dotted ID string followed by ';'","write #module \"engine.hud\"; once, at file scope");
      else error_at(&local,at,scan.error==2?"E0603":"E0600",scan.error==2?"too many source loads in one unit":"#load requires a top-level nonempty NUL-free string path followed by ';'");
      return false;
    }
    if (scan.claims) {
      std::vector<coax_string> claims;
      for (const auto &other:units) claims.push_back({other->source->module.data(),other->source->claims?(int64_t)other->source->module.size():0});
      claims.back()={nullptr,0};
      const std::string id(scan.owner.data,(size_t)scan.owner.count);
      auto prior=SU_claim_index({claims.data(),(int64_t)claims.size()},{id.data(),(int64_t)id.size()});
      if (prior>=0) {
        Loc at{1,1,(int32_t)scan.owner_begin};for(const auto &t:tokens)if(t.loc.offset==at.offset){at=t.loc;break;}
        error_at(&local,at,"E0606","logical module '"+id+"' is already claimed by '"+units[(size_t)prior]->source->path+"'");
        return false;
      }
      u->source->claims=true;u->source->module=id;
    }
    // Export lists name declarations the unit's logical module exposes; the
    // directive bytes are blanked like #module, so the parser never sees them.
    std::vector<coax_t_SU_Export> exported(512);
    auto list=SU_exports({records.data(),(int64_t)records.size()},{exported.data(),(int64_t)exported.size()});
    if (list.error) {
      Loc at{1,1,(int32_t)list.offset};
      for (const auto &t:tokens) if (t.loc.offset==at.offset) {at=t.loc;break;}
      if (list.error==3) error_at(&local,at,"E0614","an export list names this module's own declarations; reexports are not supported","export the declaration from the module that declares it");
      else error_at(&local,at,"E0613",list.error==2?"too many exported names in one unit":"#export requires file-scope identifiers separated by ',' and followed by ';'","#export hud_slot, HudRect;");
      return false;
    }
    u->source->authority=scan.authority;u->balanced=scan.balanced;u->masked=local.text;
    composition=composition || scan.count!=0;
    if (composition && !u->balanced) {
      error_at(&local,{1,1,0},"E0604","source delimiters must balance within a unit and fit depth 128");return false;
    }
    if (composition && !source_unit_syntax(&local,tokens,loads,scan.count,scan,exported,list.count)) return false;
    for (int64_t i=0;i<list.count;i++) {
      const auto &e=exported[(size_t)i];Loc at{1,1,(int32_t)e.at};
      for (const auto &t:tokens) if (t.loc.offset==at.offset) {at=t.loc;break;}
      u->source->exports.push_back({std::string(e.name.data,(size_t)e.name.count),at});
      for (int64_t b=e.begin;b<e.end;b++) if (u->masked[(size_t)b]!='\n' && u->masked[(size_t)b]!='\r')u->masked[(size_t)b]=' ';
    }
    if (scan.claims) for (int64_t at=scan.owner_begin;at<scan.owner_end;at++) if (u->masked[(size_t)at]!='\n' && u->masked[(size_t)at]!='\r')u->masked[(size_t)at]=' ';
    // Keep the original bytes for diagnostics; only the compiler's flat input is blanked.
    for (int64_t i=0;i<scan.count;i++) {
      const auto &r=loads[(size_t)i];
      for (int64_t at=r.begin;at<r.end;at++) if (u->masked[(size_t)at]!='\n' && u->masked[(size_t)at]!='\r')u->masked[(size_t)at]=' ';
      std::filesystem::path next(std::string(r.path.data,(size_t)r.path.count));
      if (next.is_relative()) next=std::filesystem::path(display).parent_path()/next;
      Loc at{1,1,(int32_t)r.begin};for(const auto &t:tokens)if(t.loc.offset==at.offset){at=t.loc;break;}
      if (!visit(next.lexically_normal().string(),depth+1,&local,at,self)) return false;
      if (r.alias.count) u->source->aliases.push_back({std::string(r.alias.data,(size_t)r.alias.count),u->source->loads.back(),(int32_t)r.alias_begin});
    }
    u->state=2;finished.push_back(self);return true;
  };
  if (!visit(path,0,src,{1,1,0},-1)) return false;
  int32_t line=1;
  for (const auto &u:units) {
    u->source->begin=(int32_t)src->text.size();u->source->first_line=line;
    src->text+=u->masked;
    // A physical boundary cannot join tokens or extend a line comment into the next unit.
    if (composition && (src->text.empty() || src->text.back()!='\n'))src->text+='\n';
    line+=(int32_t)std::count(src->text.begin()+u->source->begin,src->text.end(),'\n');
    u->source->end=(int32_t)src->text.size();src->units.push_back(u->source);
  }
  // Owners in topological order (a loader finishes after what it loads): a
  // claim owns itself; an unclaimed member joins its loaders' owners, which
  // SU_owner_join keeps independent of load and traversal order.
  std::vector<int64_t> owner(units.size(),-1);
  for (auto it=finished.rbegin();it!=finished.rend();++it) {
    const auto &u=units[(size_t)*it];
    if (u->source->claims) {owner[(size_t)*it]=*it;continue;}
    int64_t joined=-1;
    for (const auto &edge:u->loaders) {
      int64_t before=joined;joined=SU_owner_join(joined,owner[(size_t)edge.first]);
      if (joined==-2) {
        const auto &from=units[(size_t)edge.first]->source;
        int32_t offset=from->begin+(int32_t)edge.second;
        Loc at{1,1,offset};
        for (int32_t i=0;i<offset;i++) if (src->text[(size_t)i]=='\n') {at.line++;at.col=1;} else at.col++;
        error_at(src,at,"E0607","source unit '"+u->source->path+"' has no #module of its own and is loaded under two logical modules, '"+
                 units[(size_t)before]->source->module+"' and '"+units[(size_t)owner[(size_t)edge.first]]->source->module+"'",
                 "give the shared unit its own #module, or load it under one module only");
        return false;
      }
    }
    owner[(size_t)*it]=joined;
  }
  for (size_t i=0;i<units.size();i++) if (owner[i]>=0) units[i]->source->module=units[(size_t)owner[i]]->source->module;
  // An alias names the loaded unit's own claim, so it binds to that logical
  // module's declaration IDs and never to a path. Within one module an alias
  // means one module.
  auto alias_loc=[&](const SourceUnit *from,int32_t offset){
    Loc at{1,1,from->begin+offset};
    for (int32_t i=0;i<at.offset;i++) if (src->text[(size_t)i]=='\n') {at.line++;at.col=1;} else at.col++;
    return at;
  };
  for (const auto &u:units) if (!u->source->exports.empty() && u->source->module.empty()) {
    const auto &e=u->source->exports.front();
    error_at(src,{u->source->first_line+e.loc.line-1,e.loc.col,u->source->begin+e.loc.offset},"E0615",
             "#export in '"+u->source->path+"', which belongs to no logical module",
             "claim one with #module \"some.id\"; or load this file from a unit that does");
    return false;
  }
  std::vector<std::pair<const SourceUnit*,const SourceUnit::Alias*>> seen;
  for (const auto &u:units) for (const auto &a:u->source->aliases) {
    const auto &target=units[(size_t)a.target]->source;
    if (!target->claims) {
      error_at(src,alias_loc(u->source.get(),a.offset),"E0610","alias '"+a.name+"' names '"+target->path+"', which has no #module of its own",
               "add #module \"some.id\"; to that file, or load it without an alias");
      return false;
    }
    for (const auto &[from,b]:seen) {
      if (b->name==a.name && from->module==u->source->module && units[(size_t)b->target]->source->module!=target->module) {
        error_at(src,alias_loc(u->source.get(),a.offset),"E0611","alias '"+a.name+"' already names module '"+units[(size_t)b->target]->source->module+"' here",
                 "pick another alias");
        return false;
      }
    }
    seen.push_back({u->source.get(),&a});
  }
  return true;
}
} // namespace coax
