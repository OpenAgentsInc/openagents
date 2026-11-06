// Checked-tree and manifest transport for compatibility.coax. Included by the
// driver after front_end and the query string codec; all admission is Coax.

struct SurfaceRow {
  int64_t kind=0,ordinal=0,size=0,alignment=0;
  std::string owner,name,shape;
  bool defaulted=false,persisted=false,abi=false;
  uint32_t effects=0;
};
struct Surface {
  std::vector<SurfaceRow> rows;
  std::vector<coax_t_KC_Row> facts;
  void freeze() {
    facts.clear();facts.reserve(rows.size());
    for(const auto &r:rows)facts.push_back({r.kind,{r.owner.data(),(int64_t)r.owner.size()},{r.name.data(),(int64_t)r.name.size()},r.ordinal,
      {r.shape.data(),(int64_t)r.shape.size()},r.size,r.alignment,r.defaulted,r.persisted,r.abi,r.effects});
  }
  coax_rv_t_KC_Row view() { return {facts.data(),(int64_t)facts.size()}; }
};

struct SurfaceObserver {
  const File *file;const SourceUnit *entry;Surface *surface;
  std::set<Type> observed,abi_types,persisted_types;
  std::map<Type,size_t> type_rows;
  bool complete=true;
  bool published(Str name,Loc loc) const {
    auto *unit=source_unit_at(file->src,loc);if(!unit)return false;
    if(entry->module.empty()?unit!=entry:unit->module!=entry->module)return false;
    if(unit->exports.empty())return true;
    for(const auto &e:unit->exports)if(e.name==to_std(name))return true;
    return false;
  }
  std::string qualified(Str name,Loc loc,Str key={}) const {
    auto *unit=source_unit_at(file->src,loc);
    std::string prefix;
    if(unit&&!unit->module.empty())prefix=unit->module+"::";
    else if(unit&&unit!=entry)prefix=std::filesystem::path(unit->canonical).filename().string()+"::";
    return prefix+to_std(name)+(key.count?"<"+to_std(key)+">":"");
  }
  std::string signature(Type type) const {
    if(type->physical)return "Physical("+qualified(type->struct_decl->name,type->struct_decl->loc,type->struct_decl->key)+",["+to_std(type->struct_decl->compatibility_metadata)+"]="+signature(type->physical)+")";
    switch(type->kind) {
      case TypeKind::struct_:return qualified(type->struct_decl->name,type->struct_decl->loc,type->struct_decl->key);
      case TypeKind::enum_:return qualified(type->enum_decl->name,type->enum_decl->loc);
      case TypeKind::union_:return qualified(type->union_decl->name,type->union_decl->loc);
      case TypeKind::array:return "["+std::to_string(type->length)+"]"+signature(type->elem);
      case TypeKind::view:return std::string(type->length?"[]const ":"[]")+signature(type->elem);
      case TypeKind::optional:return "Optional("+signature(type->elem)+")";
      case TypeKind::result:return "Result("+signature(type->elem)+","+signature(type->sum_error)+")";
      default:return type_name(type);
    }
  }
  void default_value(const std::string &owner,const std::string &path,const Value &value,bool durable) {
    if(!complete)return;
    if(surface->rows.size()>=100000){complete=false;return;}
    Type type=value.type;
    if(type->physical){default_value(owner,path,(*value.agg)[0],durable);return;}
    SurfaceRow row;row.kind=3;row.owner=owner;row.name=path;row.persisted=durable;
    switch(type->kind) {
      case TypeKind::integer:case TypeKind::enum_: {
        Type scalar=type->kind==TypeKind::enum_?type->elem:type;uint64_t bits=value.bits;
        if(scalar->bits<64)bits&=(UINT64_C(1)<<scalar->bits)-1;
        row.shape="integer:"+std::to_string(scalar->bits)+":"+debug_source_hash(std::to_string(bits));break;
      }
      case TypeKind::floating: {
        uint64_t bits=0;if(type->bits==32){float f=(float)value.f;uint32_t word;memcpy(&word,&f,4);bits=word;}else memcpy(&bits,&value.f,8);
        row.shape="float:"+std::to_string(type->bits)+":"+debug_source_hash(std::to_string(bits));break;
      }
      case TypeKind::bool_:row.shape=std::string("bool:")+(value.bits?"1":"0");break;
      case TypeKind::string_:row.shape="string:"+std::to_string(value.s.count)+":"+debug_source_hash(std::string(value.s.count?value.s.data:"",(size_t)value.s.count));break;
      case TypeKind::struct_:row.shape="record";break;
      case TypeKind::array:row.shape="array:"+std::to_string(type->length);break;
      case TypeKind::union_:case TypeKind::result:case TypeKind::optional:row.shape="sum:"+std::to_string(value.sum_tag);break;
      case TypeKind::view:row.shape="borrowed";break; // Never a persisted field.
      case TypeKind::void_:row.shape="void";break;
      default:complete=false;return;
    }
    surface->rows.push_back(std::move(row));
    if(type->kind==TypeKind::struct_)for(int32_t i=0;i<type->struct_decl->fields.count;i++)default_value(owner,path+"."+to_std(type->struct_decl->fields[i].name),(*value.agg)[(size_t)i],durable);
    if(type->kind==TypeKind::array)for(size_t i=0;i<value.agg->size();i++)default_value(owner,path+"["+std::to_string(i)+"]",(*value.agg)[i],durable);
    if(is_sum(type)&&value.agg&&!value.agg->empty())default_value(owner,path+".@payload",(*value.agg)[0],durable);
  }
  void default_expression(const std::string &owner,const Field &field,bool durable) {
    if(!field.init)return;const Expr *e=field.init;Value value;value.type=e->type;
    if(e->kind==ExprKind::ct_value)value=*e->ct;
    else if(e->kind==ExprKind::int_lit||e->kind==ExprKind::bool_lit||e->kind==ExprKind::enum_value)value.bits=e->int_value;
    else if(e->kind==ExprKind::float_lit)value.f=e->float_value;
    else if(e->kind==ExprKind::string_lit)value.s=e->text;
    else {complete=false;return;}
    default_value(owner,to_std(field.name),value,durable);
  }
  void observe(Type type,bool abi=false,bool persisted=false) {
    if(!type)return;
    persisted=persisted||(type->kind==TypeKind::struct_&&type->struct_decl->persisted);
    const bool first=observed.insert(type).second;
    const bool newly_abi=abi&&abi_types.insert(type).second;
    const bool newly_persisted=persisted&&persisted_types.insert(type).second;
    if(!first&&!newly_abi&&!newly_persisted)return;
    if(type->physical){observe(type->physical,abi,persisted);return;}
    if(type->kind==TypeKind::array||type->kind==TypeKind::view||is_sum(type)) {
      if(type->kind!=TypeKind::union_){observe(type->elem,abi,persisted);if(type->kind==TypeKind::result)observe(type->sum_error,abi,persisted);return;}
    }
    if(type->kind!=TypeKind::struct_&&type->kind!=TypeKind::enum_&&type->kind!=TypeKind::union_)return;
    const auto name=signature(type);
    if(first) {
      SurfaceRow r;r.name=name;r.size=type->size;r.alignment=type->align;
      r.shape=type->kind==TypeKind::struct_?"struct":type->kind==TypeKind::enum_?"enum:"+signature(type->elem):"union:"+std::to_string(type->union_decl->variants.count)+":default="+std::to_string(type->sum_default);
      r.shape+=";c="+c_type(type);
      if(type->kind==TypeKind::struct_){r.shape+=":id="+std::to_string(type->struct_decl->identity)+to_std(type->struct_decl->compatibility_metadata);}
      if(type->kind==TypeKind::enum_)r.shape+=";flags="+std::to_string(type->enum_decl->flags);
      if(type->kind==TypeKind::enum_)for(const auto &m:type->enum_decl->members)r.shape+=";"+to_std(m.name)+"="+std::to_string(m.bits);
      type_rows[type]=surface->rows.size();surface->rows.push_back(std::move(r));
      int64_t ordinal=0;
      if(type->kind==TypeKind::struct_)for(const auto &field:type->struct_decl->fields) {
        SurfaceRow f;f.kind=1;f.owner=name;f.name=to_std(field.name);f.ordinal=ordinal++;f.shape=signature(field.type)+";counter="+std::to_string(field.counter);
        f.size=field.offset;f.alignment=field.type->size;f.defaulted=field.init!=nullptr;surface->rows.push_back(std::move(f));
        default_expression(name,field,persisted);
      }
      if(type->kind==TypeKind::union_)for(const auto &variant:type->union_decl->variants) {
        SurfaceRow f;f.kind=1;f.owner=name;f.name=to_std(variant.name);f.ordinal=ordinal++;f.shape=signature(variant.type);
        f.size=type->sum_payload_offset;f.alignment=variant.type->size;surface->rows.push_back(std::move(f));
      }
    }
    if(abi) {
      surface->rows[type_rows.at(type)].abi=true;
      for(auto &r:surface->rows)if(r.kind==1&&r.owner==name)r.abi=true;
    }
    if(persisted){surface->rows[type_rows.at(type)].persisted=true;for(auto &r:surface->rows)if(r.owner==name)r.persisted=true;}
    if(type->kind==TypeKind::struct_)for(const auto &f:type->struct_decl->fields)observe(f.type,abi,persisted);
    if(type->kind==TypeKind::union_)for(const auto &v:type->union_decl->variants)observe(v.type,abi,persisted);
  }
  bool collect(const EmitOptions &opts) {
    EffectFacts effects{file,true};effects.collect();size_t count=effects.procedures.size();
    std::vector<coax_t_EC_Procedure> p(count);
    for(size_t i=0;i<count;i++) {
      const auto *proc=effects.procedures[i];p[i].declared=proc->effect_present;p[i].foreign=proc->foreign;
      for(const auto &n:proc->effect_names)p[i].allowed|=(uint32_t)EC_name({n.name.data,n.name.count});
    }
    std::vector<coax_t_EC_Edge> edges;for(auto e:effects.edges)edges.push_back({e.from,e.to});
    std::vector<coax_t_EC_Site> sites;for(auto s:effects.sites)sites.push_back({s.procedure,EC_observe(s.kind,opts.wrapping)});
    std::vector<int64_t> parent(count),via(count),queue(count);
    auto proc=[&](const ProcDecl *decl) {
      if(decl->origin||!published(decl->name,decl->loc))return true;
      if(decl->generic){fprintf(stderr,"coax compat: '%s' needs a concrete published facade; generic surfaces are not qualified\n",to_std(decl->name).c_str());return false;}
      const bool abi=decl->export_c||decl->foreign;
      const auto owner=qualified(decl->name,decl->loc);
      SurfaceRow r;r.kind=2;r.name=owner;r.shape=decl->foreign?"foreignC":decl->export_c?"C":"Coax";r.size=decl->params.count;
      int64_t ordinal=0;
      for(const auto &param:decl->params){
        SurfaceRow argument;argument.kind=4;argument.owner=owner;argument.name="argument";argument.ordinal=ordinal++;argument.shape=signature(param.type);surface->rows.push_back(std::move(argument));observe(param.type,abi);
      }
      SurfaceRow result;result.kind=5;result.owner=owner;result.name="return";result.shape=signature(decl->ret);surface->rows.push_back(std::move(result));observe(decl->ret,abi);
      auto it=effects.ids.find(decl);
      if(it!=effects.ids.end()) {
        auto inferred=EC_trace({p.data(),(int64_t)count},{edges.data(),(int64_t)edges.size()},{sites.data(),(int64_t)sites.size()},it->second,{parent.data(),(int64_t)count},{via.data(),(int64_t)count},{queue.data(),(int64_t)count});
        if(inferred.status==1||inferred.status==2)return false;
        r.effects=inferred.inferred;
      }else r.effects=EC_expand(4); // Uninstantiated generic bodies have no resolved closure yet.
      surface->rows.push_back(std::move(r));return true;
    };
    for(const auto *d:file->generic_structs)if(published(d->name,d->loc)){fprintf(stderr,"coax compat: '%s' needs a concrete published facade; generic surfaces are not qualified\n",to_std(d->name).c_str());return false;}
    for(const auto *d:file->structs)if(published(d->name,d->loc)||d->persisted)observe(d->type);
    for(const auto *d:file->enums)if(published(d->name,d->loc))observe(d->type);
    for(const auto *d:file->unions)if(published(d->name,d->loc))observe(d->type);
    for(const auto *d:file->procs)if(!proc(d))return false;
    for(const auto *d:file->generics)if(!proc(d))return false;
    return complete;
  }
};

static std::string surface_text(const Surface &surface) {
  std::string out="coax.compat.v1\n";
  for(const auto &r:surface.rows)out+=std::to_string(r.kind)+"\t"+query_escape(r.owner)+"\t"+query_escape(r.name)+"\t"+std::to_string(r.ordinal)+"\t"+
    query_escape(r.shape)+"\t"+std::to_string(r.size)+"\t"+std::to_string(r.alignment)+"\t"+(r.defaulted?"1":"0")+"\t"+(r.persisted?"1":"0")+"\t"+(r.abi?"1":"0")+"\t"+std::to_string(r.effects)+"\n";
  return out;
}
static bool surface_number(const std::string &s,int64_t *out) {
  auto r=std::from_chars(s.data(),s.data()+s.size(),*out);return r.ec==std::errc()&&r.ptr==s.data()+s.size();
}
static bool surface_load(const char *path,const EmitOptions &opts,Surface *surface,bool require_declarations=true) {
  std::ifstream in(path);if(!in){fprintf(stderr,"coax compat: cannot read '%s'\n",path);return false;}
  std::string first;std::getline(in,first);
  if(first=="coax.compat.v1") {
    std::string line;
    while(std::getline(in,line)) {
      std::vector<std::string> f;size_t start=0;
      for(size_t i=0;i<=line.size();i++)if(i==line.size()||line[i]=='\t'){f.push_back(line.substr(start,i-start));start=i+1;}
      if(f.size()!=11||surface->rows.size()>=100000||line.size()>1048576)return false;
      SurfaceRow r;int64_t d,p,a,e;
      if(!surface_number(f[0],&r.kind)||!surface_number(f[3],&r.ordinal)||!surface_number(f[5],&r.size)||!surface_number(f[6],&r.alignment)||
         !surface_number(f[7],&d)||!surface_number(f[8],&p)||!surface_number(f[9],&a)||!surface_number(f[10],&e)||d<0||d>1||p<0||p>1||a<0||a>1||e<0||e>UINT32_MAX)return false;
      r.owner=query_unescape(f[1]);r.name=query_unescape(f[2]);r.shape=query_unescape(f[4]);r.defaulted=d;r.persisted=p;r.abi=a;r.effects=(uint32_t)e;
      surface->rows.push_back(std::move(r));
    }
  } else {
    struct Restore {bool value;~Restore(){g_require_effect_declarations=value;}} restore{g_require_effect_declarations};
    g_require_effect_declarations=require_declarations;
    Source src;Arena arena;File file;if(!front_end(path,&src,&arena,&file,opts,1,false))return false;
    auto canonical=std::filesystem::weakly_canonical(path).string();const SourceUnit *entry=nullptr;
    for(const auto &u:src.units)if(u->canonical==canonical)entry=u.get();
    if(!entry||!SurfaceObserver{&file,entry,surface}.collect(opts))return false;
  }
  surface->freeze();return KC_valid(surface->view());
}
static int compat_command(int argc,char **argv) {
  const bool emit=(argc==4||(argc==5&&std::string(argv[4])=="--legacy"))&&std::string(argv[2])=="--emit";
  if(!emit&&argc!=4&&!(argc==6&&std::string(argv[4])=="--baseline")){fprintf(stderr,"usage: coax compat OLD NEW [--baseline BASE]\n       coax compat --emit MODULE.coax\n");return 1;}
  EmitOptions opts;Surface old,fresh,base;
  if(emit){if(!surface_load(argv[3],opts,&fresh,argc!=5)){fprintf(stderr,"coax compat: invalid surface\n");return 1;}auto text=surface_text(fresh);fwrite(text.data(),1,text.size(),stdout);return 0;}
  if(!surface_load(argv[2],opts,&old,false)||!surface_load(argv[3],opts,&fresh)||(argc==6&&!surface_load(argv[5],opts,&base,false))){fprintf(stderr,"coax compat: invalid or incomplete surface\n");return 1;}
  int failures=0;
  auto report=[&](int64_t code,const SurfaceRow &r){fprintf(stderr,"coax compat: incompatible %s%s%s (reason %lld)\n",r.owner.c_str(),r.owner.empty()?"":".",r.name.c_str(),(long long)code);failures++;};
  for(size_t i=0;i<old.rows.size();i++) {
    auto problem=KC_problem(old.view(),fresh.view(),(int64_t)i);
    if(problem&&(argc!=6||KC_regression(old.view(),base.view(),fresh.view(),(int64_t)i)))report(problem,old.rows[i]);
  }
  for(size_t i=0;i<fresh.rows.size();i++) {
    auto problem=KC_added(old.view(),fresh.view(),(int64_t)i);
    if(problem&&(argc!=6||KC_added_regression(old.view(),base.view(),fresh.view(),(int64_t)i)))report(problem,fresh.rows[i]);
  }
  if(!failures)puts("compat: compatible");return failures?1:0;
}
