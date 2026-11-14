// Firm reason: 6 (namespace the shared Apple SDK host's optional driver).
// phone-drive.c already installs a UIKit driver for the ordinary app. Keep
// that driver intact; the GPU view's optional driver has a separate owner.
#define ui_shot_frame cg_ui_shot_frame
#define ui_shot_write cg_ui_shot_write
#define ui_drive cg_ui_drive
#define ui_drive_listen cg_ui_drive_listen
#define ui_drive_serve cg_ui_drive_serve
#define ui_agent_clock cg_ui_agent_clock
#define ui_real_time cg_ui_real_time
#include "../shared/host/apple/ui-host.m"
