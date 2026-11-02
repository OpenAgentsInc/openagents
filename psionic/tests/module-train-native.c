/* Verification-only independent scalar expectations and native IEEE inputs. */
#include "module-train-native.h"
#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static int run(double *master,float *inputs,double *gradient,double *loss) {
    return (int)mtr_native_linear((coax_v_f64){master,2},(coax_v_f32){inputs,4},
                                 (coax_v_f64){gradient,2},(coax_v_f64){loss,1});
}
int main(void) {
    double master[2]={-2,.5},gradient[2]={91,92},loss=-71;
    float inputs[4]={3,-2,81,82};
    assert(run(master,inputs,gradient,&loss)==COAX_MtrStatus_ok);
    assert(loss==-7 && gradient[0]==3 && gradient[1]==-2);
    assert(master[0]==-2 && master[1]==.5 && inputs[2]==-2 && inputs[3]==.5);
    const uint64_t nonfinite[3]={UINT64_C(0x7ff8000000000001),UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000)};
    for(int k=0;k<3;k++) {
        master[0]=1;memcpy(&master[1],&nonfinite[k],8);double before[2];memcpy(before,master,sizeof master);
        inputs[0]=1;inputs[1]=2;inputs[2]=81;inputs[3]=82;gradient[0]=91;gradient[1]=92;loss=-71;
        assert(run(master,inputs,gradient,&loss)==COAX_MtrStatus_number);
        assert(!memcmp(master,before,sizeof master) && inputs[2]==81 && inputs[3]==82);
        assert(gradient[0]==91 && gradient[1]==92 && loss==-71);
    }
    const uint32_t bad_inputs[3]={UINT32_C(0x7fc00001),UINT32_C(0x7f800000),UINT32_C(0xff800000)};
    for(int k=0;k<3;k++) {
        master[0]=1;master[1]=2;memcpy(&inputs[0],&bad_inputs[k],4);inputs[1]=2;gradient[0]=91;gradient[1]=92;loss=-71;
        assert(run(master,inputs,gradient,&loss)==COAX_MtrStatus_number);
        assert(master[0]==1 && master[1]==2 && gradient[0]==91 && gradient[1]==92 && loss==-71);
        float value;memcpy(&value,&bad_inputs[k],4);
        assert(mtr_native_metadata_value(value)==COAX_MtrStatus_number);
    }
    float late[7]={0,1,1e38f,0,0,0,0};double untouched[3]={91,92,93};
    assert(mtr_native_late((coax_v_f32){late,7},(coax_v_f64){untouched,3})==COAX_MtrStatus_number);
    assert(untouched[0]==91 && untouched[1]==92 && untouched[2]==93);
    assert(mtr_native_large());
    puts("module-train native: analytic loss/gradients, 1100-element entry, IEEE refusals and complete publication pass");
    return 0;
}
