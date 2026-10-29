#include "decision-calibration-memory.h"
/* pc_load only reads both views; the caller keeps them valid for the call. */
coax_t_PcCalibration psionic_calibration(uint8_t kind,coax_v_u8 readout,coax_v_u8 calibration){return pc_load(kind,readout,calibration);}
