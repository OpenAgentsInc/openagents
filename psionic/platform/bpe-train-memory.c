#include "bpe-train-memory.h"
#include "spans.h"
coax_t_PtResult psionic_bt_pack(coax_t_BtModel m,coax_v_u8 classes,coax_v_u8 out,coax_v_u8 stage){return bt_pack(m,classes,out,stage,psionic_spans_overlap(out.data,out.count,1,stage.data,stage.count,1)||psionic_spans_overlap(out.data,out.count,1,classes.data,classes.count,1)||psionic_spans_overlap(stage.data,stage.count,1,classes.data,classes.count,1));}
