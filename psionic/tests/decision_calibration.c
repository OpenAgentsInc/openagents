#include "decision-calibration-memory.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
int main(void){
    static const char pinned[]="{\n  \"readout\": \"slot_lm\",\n  \"head_dim\": 256,\n  \"n_slots\": 256\n}";
    unsigned char bytes[64];memcpy(bytes,pinned,63);assert(sizeof(pinned)==64);
    coax_v_u8 readout={bytes,63},none={NULL,0};
    for(uint8_t kind=0;kind<2;kind++){
        coax_t_PcCalibration c=psionic_calibration(kind,readout,none);
        assert(c.status==COAX_PcStatus_ok && c.source==COAX_PcSource_none && c.slots==256 && c.head_dim==256);
        assert(pc_temperature(c,COAX_QrKind_choice,255)==1 && pc_temperature(c,COAX_QrKind_noul,2)==1 && pc_temperature(c,COAX_QrKind_score,10)==1);
        assert(pc_temperature(c,COAX_QrKind_noul,3)==0 && pc_temperature(c,(coax_t_QrKind)3,2)==0);
    }
    assert(psionic_calibration(2,readout,none).status==COAX_PcStatus_kind);
    assert(psionic_calibration(0,readout,(coax_v_u8){bytes,1}).status==COAX_PcStatus_unpinned);
    bytes[30]^=1;assert(psionic_calibration(0,readout,none).status==COAX_PcStatus_readout);
    puts("calibration native: pinned readout bytes, no temperature table, temperature 1.0 and refusals pass");
}
