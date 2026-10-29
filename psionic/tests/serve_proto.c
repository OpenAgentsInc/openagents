/* The protobuf answer for each JSON body in serve-proto-oracle.h, printed as
   hex for serve_proto_oracle.py --check-answers; plus native refusals. */
#include "serve-proto.h"
#include "serve-proto-oracle.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}

int main(void){
    static uint8_t out[1<<18], arena[1<<21];
    coax_v_u8 o={out,sizeof out}, a={arena,sizeof arena};
    for(size_t i=0;i<sizeof(answer_bodies)/sizeof(*answer_bodies);i++){
        coax_t_SpBody b=sp_answer(str(answer_bodies[i]),o,a);
        assert(b.code==COAX_SvCode_none && b.from==0 && b.to>0);
        const char *head=strstr((const char*)out,"\r\n\r\n");assert(head);
        assert(strstr((const char*)out,"Content-Type: application/x-protobuf\r\n")<head);
        size_t start=(size_t)(head+4-(const char*)out);
        for(size_t k=start;k<(size_t)b.to;k++)printf("%02x",out[k]);
        printf("\n");
    }
    /* Anything that is not a rendered answer body is refused, not guessed at. */
    assert(sp_answer(str("[]"),o,a).code==COAX_SvCode_internal);
    assert(sp_answer(str("{\"model\":\"m\",\"extra\":1}"),o,a).code==COAX_SvCode_internal);
    assert(sp_answer(str(answer_bodies[0]),(coax_v_u8){out,40},a).code==COAX_SvCode_internal);
    assert(sp_answer(str(answer_bodies[0]),o,(coax_v_u8){arena,1000}).code==COAX_SvCode_internal);
    fprintf(stderr,"serve protobuf native: %zu answers encoded, 4 refusals\n",sizeof(answer_bodies)/sizeof(*answer_bodies));
    return 0;
}
