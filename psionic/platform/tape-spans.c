/* Observe native span overlap; differentiation stays in Coax. */
#include "tape-spans.h"
#include "spans.h"
#define OV(a,b) psionic_spans_overlap((a).data,(a).count,sizeof(*(a).data), \
    (b).data,(b).count,sizeof(*(b).data))

coax_t_TpStatus psionic_tp_forward(coax_v_t_TpNode nodes, coax_v_f64 params, coax_v_f64 values) {
    return tp_forward(nodes,params,values,OV(values,params)||OV(values,nodes));
}

coax_t_TpStatus psionic_tp_backward(coax_v_t_TpNode nodes, coax_v_f64 values, coax_v_f64 grads) {
    return tp_backward(nodes,values,grads,OV(grads,values)||OV(grads,nodes));
}

coax_t_TpBuild psionic_tp_tabular(coax_t_TbModel m, coax_v_f64 x, coax_v_f64 y, coax_v_s64 rows,
        coax_v_t_TpNode nodes) {
    return tp_tabular(m,x,y,rows,nodes,OV(nodes,x)||OV(nodes,y)||OV(nodes,rows));
}

coax_t_TpStatus psionic_tp_gradient(coax_t_TbModel m, coax_v_f64 p, coax_v_f64 x, coax_v_f64 y,
        coax_v_s64 rows, coax_v_f64 g, coax_v_t_TpNode nodes, coax_v_f64 values) {
    bool o = OV(g,nodes)||OV(g,values)||OV(nodes,values);
    o = o||OV(g,p)||OV(g,x)||OV(g,y)||OV(g,rows);
    o = o||OV(nodes,p)||OV(nodes,x)||OV(nodes,y)||OV(nodes,rows);
    o = o||OV(values,p)||OV(values,x)||OV(values,y)||OV(values,rows);
    return tp_gradient(m,p,x,y,rows,g,nodes,values,o);
}
