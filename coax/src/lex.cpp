// Representation bridge only. All scanning, numeric conversion, recovery and
// diagnostic text live in lib/compiler/lexer.coax. The checked-in generated C
// seed is used only to build stage0; normal compilers link regenerated Coax C.
#include "coax.h"
#ifdef COAX_BOOTSTRAP_IMPORTER
#include "lexer.h"
#else
#include "proto_importer.h"
#endif
namespace coax {
static_assert((uint8_t)Tok::eof == COAX_LX_Kind_k_eof, "lexer token ABI: eof");
static_assert((uint8_t)Tok::ident == COAX_LX_Kind_k_ident, "lexer token ABI: ident");
static_assert((uint8_t)Tok::keyword == COAX_LX_Kind_k_keyword, "lexer token ABI: keyword");
static_assert((uint8_t)Tok::int_lit == COAX_LX_Kind_k_int_lit, "lexer token ABI: int_lit");
static_assert((uint8_t)Tok::float_lit == COAX_LX_Kind_k_float_lit, "lexer token ABI: float_lit");
static_assert((uint8_t)Tok::string_lit == COAX_LX_Kind_k_string_lit, "lexer token ABI: string_lit");
static_assert((uint8_t)Tok::directive == COAX_LX_Kind_k_directive, "lexer token ABI: directive");
static_assert((uint8_t)Tok::annotation == COAX_LX_Kind_k_annotation, "lexer token ABI: annotation");
static_assert((uint8_t)Tok::lparen == COAX_LX_Kind_k_lparen, "lexer token ABI: lparen");
static_assert((uint8_t)Tok::rparen == COAX_LX_Kind_k_rparen, "lexer token ABI: rparen");
static_assert((uint8_t)Tok::lbrace == COAX_LX_Kind_k_lbrace, "lexer token ABI: lbrace");
static_assert((uint8_t)Tok::rbrace == COAX_LX_Kind_k_rbrace, "lexer token ABI: rbrace");
static_assert((uint8_t)Tok::lbracket == COAX_LX_Kind_k_lbracket, "lexer token ABI: lbracket");
static_assert((uint8_t)Tok::rbracket == COAX_LX_Kind_k_rbracket, "lexer token ABI: rbracket");
static_assert((uint8_t)Tok::comma == COAX_LX_Kind_k_comma, "lexer token ABI: comma");
static_assert((uint8_t)Tok::semicolon == COAX_LX_Kind_k_semicolon, "lexer token ABI: semicolon");
static_assert((uint8_t)Tok::colon == COAX_LX_Kind_k_colon, "lexer token ABI: colon");
static_assert((uint8_t)Tok::coloncolon == COAX_LX_Kind_k_coloncolon, "lexer token ABI: coloncolon");
static_assert((uint8_t)Tok::colonequal == COAX_LX_Kind_k_colonequal, "lexer token ABI: colonequal");
static_assert((uint8_t)Tok::arrow == COAX_LX_Kind_k_arrow, "lexer token ABI: arrow");
static_assert((uint8_t)Tok::dot == COAX_LX_Kind_k_dot, "lexer token ABI: dot");
static_assert((uint8_t)Tok::dotdot == COAX_LX_Kind_k_dotdot, "lexer token ABI: dotdot");
static_assert((uint8_t)Tok::dotdotlt == COAX_LX_Kind_k_dotdotlt, "lexer token ABI: dotdotlt");
static_assert((uint8_t)Tok::equal == COAX_LX_Kind_k_equal, "lexer token ABI: equal");
static_assert((uint8_t)Tok::plus_equal == COAX_LX_Kind_k_plus_equal, "lexer token ABI: plus_equal");
static_assert((uint8_t)Tok::minus_equal == COAX_LX_Kind_k_minus_equal, "lexer token ABI: minus_equal");
static_assert((uint8_t)Tok::star_equal == COAX_LX_Kind_k_star_equal, "lexer token ABI: star_equal");
static_assert((uint8_t)Tok::slash_equal == COAX_LX_Kind_k_slash_equal, "lexer token ABI: slash_equal");
static_assert((uint8_t)Tok::percent_equal == COAX_LX_Kind_k_percent_equal, "lexer token ABI: percent_equal");
static_assert((uint8_t)Tok::amp_equal == COAX_LX_Kind_k_amp_equal, "lexer token ABI: amp_equal");
static_assert((uint8_t)Tok::pipe_equal == COAX_LX_Kind_k_pipe_equal, "lexer token ABI: pipe_equal");
static_assert((uint8_t)Tok::caret_equal == COAX_LX_Kind_k_caret_equal, "lexer token ABI: caret_equal");
static_assert((uint8_t)Tok::shl_equal == COAX_LX_Kind_k_shl_equal, "lexer token ABI: shl_equal");
static_assert((uint8_t)Tok::shr_equal == COAX_LX_Kind_k_shr_equal, "lexer token ABI: shr_equal");
static_assert((uint8_t)Tok::plus == COAX_LX_Kind_k_plus, "lexer token ABI: plus");
static_assert((uint8_t)Tok::minus == COAX_LX_Kind_k_minus, "lexer token ABI: minus");
static_assert((uint8_t)Tok::star == COAX_LX_Kind_k_star, "lexer token ABI: star");
static_assert((uint8_t)Tok::slash == COAX_LX_Kind_k_slash, "lexer token ABI: slash");
static_assert((uint8_t)Tok::percent == COAX_LX_Kind_k_percent, "lexer token ABI: percent");
static_assert((uint8_t)Tok::amp == COAX_LX_Kind_k_amp, "lexer token ABI: amp");
static_assert((uint8_t)Tok::pipe == COAX_LX_Kind_k_pipe, "lexer token ABI: pipe");
static_assert((uint8_t)Tok::caret == COAX_LX_Kind_k_caret, "lexer token ABI: caret");
static_assert((uint8_t)Tok::shl == COAX_LX_Kind_k_shl, "lexer token ABI: shl");
static_assert((uint8_t)Tok::shr == COAX_LX_Kind_k_shr, "lexer token ABI: shr");
static_assert((uint8_t)Tok::eqeq == COAX_LX_Kind_k_eqeq, "lexer token ABI: eqeq");
static_assert((uint8_t)Tok::noteq == COAX_LX_Kind_k_noteq, "lexer token ABI: noteq");
static_assert((uint8_t)Tok::lt == COAX_LX_Kind_k_lt, "lexer token ABI: lt");
static_assert((uint8_t)Tok::le == COAX_LX_Kind_k_le, "lexer token ABI: le");
static_assert((uint8_t)Tok::gt == COAX_LX_Kind_k_gt, "lexer token ABI: gt");
static_assert((uint8_t)Tok::ge == COAX_LX_Kind_k_ge, "lexer token ABI: ge");
static_assert((uint8_t)Tok::andand == COAX_LX_Kind_k_andand, "lexer token ABI: andand");
static_assert((uint8_t)Tok::oror == COAX_LX_Kind_k_oror, "lexer token ABI: oror");
static_assert((uint8_t)Tok::bang == COAX_LX_Kind_k_bang, "lexer token ABI: bang");
static_assert((uint8_t)Tok::dollar == COAX_LX_Kind_k_dollar, "lexer token ABI: dollar");
const char *tok_name(Tok kind) {return LX_name((uint32_t)kind).data;}
bool lex(Arena *arena,const Source *src,std::vector<Token> *out) {
  const auto input=coax_string{src->text.data(),(int64_t)src->text.size()};
  const auto caps=LX_capacities(input.count);
  std::unique_ptr<uint8_t[]> decoded(new uint8_t[(size_t)caps.decoded]), message(new uint8_t[(size_t)caps.diagnostic]);
  auto state=LX_begin(input.count);const int32_t errors_before=g_error_count;
  for (;;) {
    const auto item=LX_next(input,{&state,1},{decoded.get(),caps.decoded},{message.get(),caps.diagnostic});
    if(item.diagnostic.code.count) {
      const auto &d=item.diagnostic;
      const std::string code(d.code.data,(size_t)d.code.count);
      error_at(src,{(int32_t)d.loc.line,(int32_t)d.loc.column,(int32_t)d.loc.offset},code.c_str(),
          std::string(d.message.data,(size_t)d.message.count),std::string(d.fix.data ? d.fix.data : "",(size_t)d.fix.count));
    }
    if(!item.has_token) continue;
    const auto &v=item.token;Token token;
    token.kind=(Tok)v.kind;token.loc={(int32_t)v.loc.line,(int32_t)v.loc.column,(int32_t)v.loc.offset};
    token.text={input.data+v.text_begin,(int32_t)(v.text_end-v.text_begin)};
    token.int_value=v.int_value;memcpy(&token.float_value,&v.float_value,sizeof(token.float_value));
    token.float_bits=(uint8_t)v.float_bits;
    if(v.decoded) {
      char *bytes=(char*)arena_alloc(arena,(size_t)v.decoded_count+1);
      if(v.decoded_count) memcpy(bytes,decoded.get(),(size_t)v.decoded_count);
      bytes[v.decoded_count]=0;token.string_value={bytes,(int32_t)v.decoded_count};
    }
    out->push_back(token);if(token.kind==Tok::eof) break;
  }
  return g_error_count==errors_before;
}
} // namespace coax
