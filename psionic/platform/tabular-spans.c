/* Observe native span overlap; simulation, scaling and training stay in Coax. */
#include "tabular-spans.h"
#include "spans.h"
#define OV(a,b) psionic_spans_overlap((a).data,(a).count,sizeof(*(a).data), \
    (b).data,(b).count,sizeof(*(b).data))

coax_t_TbSim psionic_tb_simulate(uint64_t seed, int64_t sessions, coax_v_f64 x, coax_v_f64 y,
        coax_v_f64 latency, coax_v_s64 groups) {
    bool o = OV(x,y)||OV(x,latency)||OV(x,groups)||OV(y,latency)||OV(y,groups)||OV(latency,groups);
    return tb_simulate(seed,sessions,x,y,latency,groups,o);
}

int64_t psionic_tb_rows(coax_v_s64 groups, int64_t part, coax_v_s64 out) {
    return tb_rows(groups,part,out,OV(groups,out));
}

coax_t_TbStatus psionic_tb_fit_scale(coax_v_f64 x, int64_t d, coax_v_s64 rows, coax_v_f64 mean,
        coax_v_f64 sd) {
    bool o = OV(mean,sd)||OV(mean,x)||OV(sd,x)||OV(mean,rows)||OV(sd,rows);
    return tb_fit_scale(x,d,rows,mean,sd,o);
}

coax_t_TbStatus psionic_tb_apply_scale(coax_v_f64 x, int64_t d, coax_v_f64 mean, coax_v_f64 sd,
        coax_v_f64 out) {
    return tb_apply_scale(x,d,mean,sd,out,OV(out,x)||OV(out,mean)||OV(out,sd));
}

coax_t_TbTrain psionic_tb_train(coax_t_TbModel m, coax_t_TbConfig c, coax_v_f64 x, coax_v_f64 y,
        coax_v_s64 rows, coax_v_f64 params, coax_v_s64 order) {
    bool o = OV(params,x)||OV(params,y)||OV(params,rows)||OV(params,order)
        ||OV(order,x)||OV(order,y)||OV(order,rows);
    return tb_train(m,c,x,y,rows,params,order,o);
}

coax_t_TbStatus psionic_tb_predict(coax_t_TbModel m, coax_v_f64 params, coax_v_f64 x, coax_v_f64 out) {
    return tb_predict(m,params,x,out,OV(out,params)||OV(out,x));
}

coax_t_TbStatus psionic_tb_rule(coax_v_f64 x, coax_v_f64 out) {
    return tb_rule(x,out,OV(out,x));
}
