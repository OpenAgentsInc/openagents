// This host owns file effects and buffer lifetimes. It does no request policy.
#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "workflow.h"

static void codec_tests(void) {
    const uint8_t golden[] = {8,1,18,4,98,111,111,107,26,7,112,114,101,118,105,101,119,34,12,98,111,111,107,45,114,101,113,117,101,115,116,40,7};
    uint8_t out[128];
    for (int64_t n = 0; n < (int64_t)sizeof golden; ++n)
        assert(workflow_packet(7, (coax_v_u8){out,n}) == -1);
    int64_t n = workflow_packet(7, (coax_v_u8){out,sizeof golden});
    assert(n == sizeof golden && memcmp(out,golden,sizeof golden) == 0);
    n = workflow_packet(0, (coax_v_u8){out,sizeof out});
    coax_t_WorkflowRead read = workflow_read((coax_v_u8){out,n});
    assert(read.error == 0 && read.revision == 0);
    n = workflow_packet(UINT32_MAX, (coax_v_u8){out,sizeof out});
    read = workflow_read((coax_v_u8){out,n});
    assert(read.error == 0 && read.revision == UINT32_MAX);
    uint8_t bad[] = {255,255};
    assert(workflow_read((coax_v_u8){bad,sizeof bad}).error != 0);
}

int main(int argc, char **argv) {
    assert(argc == 2);
    codec_tests();
    coax_t_Request zero = {0};
    coax_t_Request first = request_begin(zero,10,20);
    coax_t_Request second = request_begin(request_cancel(first),11,30);
    // A native task would capture this epoch before starting its asynchronous I/O.
    uint32_t callback_epoch = second.epoch;
    uint8_t packet[128];
    int64_t n = workflow_packet(callback_epoch,(coax_v_u8){packet,sizeof packet});
    assert(n > 0);
    FILE *f = fopen(argv[1],"wb");
    assert(f);
    assert(fwrite(packet,1,(size_t)n,f) == (size_t)n);
    assert(fclose(f) == 0);
    uint8_t retained[128];
    f = fopen(argv[1],"rb");
    assert(f);
    size_t got = fread(retained,1,sizeof retained,f);
    assert(!ferror(f));
    assert(fclose(f) == 0 && got == (size_t)n);
    coax_t_WorkflowRead read = workflow_read((coax_v_u8){retained,(int64_t)got});
    assert(read.error == 0 && read.revision == callback_epoch);
    // A delayed callback from the first task must not replace the current result.
    coax_t_Request stale = request_complete(second,first.epoch,12,1,99);
    assert(stale.phase == COAX_RequestPhase_running && stale.value == 0);
    coax_t_Request done = request_complete(stale,callback_epoch,13,1,42);
    assert(done.phase == COAX_RequestPhase_succeeded && done.value == 42);
    coax_t_Request failed = request_complete(request_begin(done,20,30),3,21,0,0);
    assert(failed.phase == COAX_RequestPhase_failed);
    coax_t_Request expired = request_tick(request_begin(done,20,30),30);
    assert(expired.phase == COAX_RequestPhase_expired);
    printf("file: %zu bytes, revision %llu; result %lld; stale ignored; failure and timeout checked\n",
        got,(unsigned long long)read.revision,(long long)done.value);
    return 0;
}
