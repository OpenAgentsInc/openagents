#include "URI_SOURCE"
#define EX(name) __attribute__((export_name(name)))
static uint8_t input[4096];static coax_t_UriParts result;
EX("input") uintptr_t input_pointer(void){return(uintptr_t)input;}
EX("parse") void parse(int32_t n){result=uri_parse((coax_string){(char*)input,n});}
EX("field") int32_t field(int32_t n){switch(n){case 0:return result.ok;case 1:return(int32_t)result.scheme;case 2:return(int32_t)result.host_from;case 3:return(int32_t)result.host_to;case 4:return(int32_t)result.port;case 5:return(int32_t)result.path_from;case 6:return(int32_t)result.path_to;case 7:return result.has_query;default:return(int32_t)result.query_from;}}
