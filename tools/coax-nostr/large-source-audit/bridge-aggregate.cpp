// Representation/owned-storage bridge only. Scanning, load syntax, parsing,
// source/dataflow admission and source fingerprints remain existing Coax code.
#include "frontend.h"
#include "policy.h"
#include <algorithm>
#include <cstring>
#include <deque>
#include <new>
#include <string>
#include <vector>
static_assert(COAX_PS_Tag_type_named==1&&COAX_PS_Tag_expr_int_lit==16&&COAX_PS_Tag_expr_bool_lit==18,"literal/type tag ABI");
static_assert(COAX_PS_Tag_expr_ident==20&&COAX_PS_Tag_expr_call==21&&COAX_PS_Tag_expr_unary==22&&COAX_PS_Tag_expr_binary==23,"expression ABI");
static_assert(COAX_PS_Tag_expr_run==24&&COAX_PS_Tag_expr_field==25&&COAX_PS_Tag_expr_index==26&&COAX_PS_Tag_expr_slice==27,"memory/run ABI");
static_assert(COAX_PS_Tag_expr_cast==30&&COAX_PS_Tag_expr_ifx==31,"cast/condition ABI");
static_assert(COAX_PS_Tag_stmt_block==40&&COAX_PS_Tag_stmt_var==42&&COAX_PS_Tag_stmt_assign==43&&COAX_PS_Tag_stmt_ret==44,"statement ABI");
static_assert(COAX_PS_Tag_stmt_if_==45&&COAX_PS_Tag_stmt_while_==46&&COAX_PS_Tag_decl_proc==64&&COAX_PS_Tag_decl_constant==65,"control/declaration ABI");
static_assert(COAX_PS_Tag_decl_param==68&&COAX_PS_Tag_decl_file==74,"file/parameter ABI");
static_assert(COAX_LX_Kind_k_equal==23&&COAX_LX_Kind_k_plus_equal==24&&COAX_LX_Kind_k_slash_equal==27&&COAX_LX_Kind_k_percent_equal==28&&COAX_LX_Kind_k_shl_equal==32&&COAX_LX_Kind_k_shr_equal==33,"assignment operator ABI");
static_assert(COAX_LX_Kind_k_plus==34&&COAX_LX_Kind_k_minus==35&&COAX_LX_Kind_k_star==36&&COAX_LX_Kind_k_slash==37&&COAX_LX_Kind_k_percent==38,"arithmetic operator ABI");
static_assert(COAX_LX_Kind_k_amp==39&&COAX_LX_Kind_k_pipe==40&&COAX_LX_Kind_k_caret==41&&COAX_LX_Kind_k_shl==42&&COAX_LX_Kind_k_shr==43,"bit operator ABI");
static_assert(COAX_LX_Kind_k_eqeq==44&&COAX_LX_Kind_k_ge==49&&COAX_LX_Kind_k_andand==50&&COAX_LX_Kind_k_oror==51&&COAX_LX_Kind_k_bang==52,"comparison operator ABI");

static_assert(COAX_PS_Tag_type_array==2&&COAX_PS_Tag_type_view==3&&COAX_PS_Tag_expr_struct_lit==28&&COAX_PS_Tag_expr_array_lit==29,"aggregate type/value ABI");
static_assert(COAX_PS_Tag_stmt_for_range==47&&COAX_PS_Tag_stmt_assert_==52&&COAX_PS_Tag_decl_struct_==66&&COAX_PS_Tag_decl_field==69&&COAX_PS_Tag_decl_field_init==71,"aggregate statement/field ABI");
static coax_t_ScResult failure(uint32_t error,int64_t unit,int64_t line=0,int64_t column=0){coax_t_ScResult r{};r.error=error;r.unit=unit;r.line=line;r.column=column;return r;}
static coax_t_ScResult ag_bridge_impl(const coax_string *sources,int64_t count,const coax_t_ScType *types,int64_t type_count,coax_string entry,coax_t_AgGuard *guards,int64_t capacity,int64_t *guard_count,bool compiled,bool classified,uint64_t secret_mask,const coax_string *names,const coax_string *parameter_types,int64_t parameter_count,bool views,const int64_t *extents,const uint32_t *access){
 *guard_count=0;
 try{
  if(!LA_units(count))return failure(2,-1);
  std::vector<int64_t> lengths;for(int64_t i=0;i<count;i++)lengths.push_back(sources[i].count);
  if(!LA_inputs({lengths.data(),(int64_t)lengths.size()},0))return failure(2,-1);
  std::vector<coax_t_ScNode> flat(1);std::vector<uint64_t> values(1);std::vector<int64_t> roots;std::deque<std::string> owned;
  for(int64_t unit=0;unit<count;unit++){
   const auto source=sources[unit];const auto caps=LX_capacities(source.count);auto lex=LX_begin(source.count);
   std::vector<uint8_t> decoded((size_t)caps.decoded),message((size_t)caps.diagnostic);
   std::vector<coax_t_PS_Token> tokens;std::vector<coax_t_SU_Token> syntax;
   for(;;){
    auto item=LX_next(source,{&lex,1},{decoded.data(),(int64_t)decoded.size()},{message.data(),(int64_t)message.size()});
    if(item.diagnostic.code.count)return failure(1,unit,item.diagnostic.loc.line,item.diagnostic.loc.column);
    if(!item.has_token)continue;
    const auto t=item.token;coax_string text{source.data+t.text_begin,t.text_end-t.text_begin};
    if(t.decoded){owned.emplace_back((char*)decoded.data(),(size_t)t.decoded_count);const auto &s=owned.back();text={s.data(),(int64_t)s.size()};}
    if(!audit_source_token_allowed(t.kind==COAX_LX_Kind_k_directive,text))return failure(5,unit,t.loc.line,t.loc.column);
    double floating;std::memcpy(&floating,&t.float_value,sizeof(floating));
    tokens.push_back({(uint32_t)t.kind,text,{t.loc.line,t.loc.column,t.loc.offset},t.int_value,floating,t.float_bits,t.text_end-t.text_begin});
    syntax.push_back({(uint32_t)t.kind,text,t.loc.offset,t.loc.offset+t.text_end-t.text_begin});
    if(t.kind==COAX_LX_Kind_k_eof)break;
   }
   std::vector<coax_t_SU_Load> loads(tokens.size());
   const auto scan=SU_scan({syntax.data(),(int64_t)syntax.size()},{loads.data(),(int64_t)loads.size()});
   if(scan.error||!scan.balanced)return failure(1,unit);
   std::vector<coax_t_PS_Token> filtered;int64_t load=0;
   for(const auto &t:tokens){
    while(load<scan.count&&t.loc.offset>=loads[(size_t)load].end)load++;
    if(load<scan.count&&t.loc.offset>=loads[(size_t)load].begin&&t.loc.offset<loads[(size_t)load].end)continue;
    filtered.push_back(t);
   }
   auto pcaps=PS_caps((int64_t)filtered.size(),source.count);int64_t available=LA_node_limit()-(int64_t)flat.size()+1;
   if(available<2)return failure(2,unit);
   std::vector<coax_t_PS_Node> nodes((size_t)std::min(pcaps.nodes,available));message.resize((size_t)pcaps.message);coax_t_PS_State state{};
   const auto result=PS_parse(source,{filtered.data(),(int64_t)filtered.size()},{nodes.data(),(int64_t)nodes.size()},{&state,1},{message.data(),(int64_t)message.size()});
   if(!result.ok)return failure(1,unit,state.loc.line,state.loc.column);
   const int64_t base=(int64_t)flat.size()-1;auto ref=[&](int64_t id){return id?base+id:0;};
   roots.push_back(ref(result.root));
   for(int64_t i=1;i<=state.used;i++){
    const auto &n=nodes[(size_t)i];
    flat.push_back({(uint32_t)n.kind,unit,n.loc.line,n.loc.column,n.name,ref(n.a),ref(n.b),ref(n.c),ref(n.d),ref(n.head),ref(n.next),n.op,n.flag,n.flag2});values.push_back(n.value);
   }
  }
  std::vector<coax_t_AgMemory> memory(1);
  coax_t_ScResult result{};
  if(classified) {
   if(parameter_count<0||parameter_count>8)return failure(2,-1);
   std::vector<coax_t_AgLabel> labels;for(int64_t i=0;i<parameter_count;i++)labels.push_back({names[i],parameter_types[i],views?extents[i]:0,views?access[i]:0});
   result=views?AG_check_public_views({flat.data(),(int64_t)flat.size()},{values.data(),(int64_t)values.size()},{roots.data(),(int64_t)roots.size()},{const_cast<coax_t_ScType*>(types),type_count},entry,{memory.data(),1},secret_mask,{labels.data(),parameter_count},compiled):AG_check_classified({flat.data(),(int64_t)flat.size()},{values.data(),(int64_t)values.size()},{roots.data(),(int64_t)roots.size()},{const_cast<coax_t_ScType*>(types),type_count},entry,{memory.data(),1},secret_mask,{labels.data(),parameter_count},compiled);
  }
  else result=compiled?AG_check_compiled({flat.data(),(int64_t)flat.size()},{values.data(),(int64_t)values.size()},{roots.data(),(int64_t)roots.size()},{const_cast<coax_t_ScType*>(types),type_count},entry,{memory.data(),1}):AG_check({flat.data(),(int64_t)flat.size()},{values.data(),(int64_t)values.size()},{roots.data(),(int64_t)roots.size()},{const_cast<coax_t_ScType*>(types),type_count},entry,{memory.data(),1});
  if(capacity<memory[0].guard_count)return failure(2,-1);
  for(int64_t i=0;i<memory[0].guard_count;i++)guards[i]=memory[0].guards.e[i];
  *guard_count=memory[0].guard_count;return result;
 }catch(const std::bad_alloc&){return failure(2,-1);}
}
extern "C" coax_t_ScResult ag_bridge(const coax_string *sources,int64_t count,const coax_t_ScType *types,int64_t type_count,coax_string entry,coax_t_AgGuard *guards,int64_t capacity,int64_t *guard_count){return ag_bridge_impl(sources,count,types,type_count,entry,guards,capacity,guard_count,false,false,0,nullptr,nullptr,0,false,nullptr,nullptr);}
// Only the checked-source driver calls this after compiler and hash validation.
extern "C" coax_t_ScResult ag_bridge_compiled(const coax_string *sources,int64_t count,const coax_t_ScType *types,int64_t type_count,coax_string entry,coax_t_AgGuard *guards,int64_t capacity,int64_t *guard_count){return ag_bridge_impl(sources,count,types,type_count,entry,guards,capacity,guard_count,true,false,0,nullptr,nullptr,0,false,nullptr,nullptr);}

// Internal driver entry. Classification and compile provenance are supplied only
// after the checked driver binds them to its compiled source and type inventory.
extern "C" coax_t_ScResult ag_bridge_classified(const coax_string *sources,int64_t count,const coax_t_ScType *types,int64_t type_count,coax_string entry,coax_t_AgGuard *guards,int64_t capacity,int64_t *guard_count,uint64_t secret_mask,const coax_string *names,const coax_string *parameter_types,int64_t parameter_count,bool compiled){return ag_bridge_impl(sources,count,types,type_count,entry,guards,capacity,guard_count,compiled,true,secret_mask,names,parameter_types,parameter_count,false,nullptr,nullptr);}

// Internal checked-source bridge: v2 view rows are validated by the driver and
// their live, aligned and disjoint storage is an explicit caller premise.
extern "C" coax_t_ScResult ag_bridge_public_views(const coax_string *sources,int64_t count,const coax_t_ScType *types,int64_t type_count,coax_string entry,coax_t_AgGuard *guards,int64_t capacity,int64_t *guard_count,uint64_t secret_mask,const coax_string *names,const coax_string *parameter_types,int64_t parameter_count,bool compiled,const int64_t *extents,const uint32_t *access){return ag_bridge_impl(sources,count,types,type_count,entry,guards,capacity,guard_count,compiled,true,secret_mask,names,parameter_types,parameter_count,true,extents,access);}
