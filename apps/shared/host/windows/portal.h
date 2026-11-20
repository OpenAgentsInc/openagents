// Firm reason: 6 (the Windows file dialog's COM ABI).
// The file chooser on Windows (GitHub #13782): the same contract as Linux's (../linux/portal.h),
// served by portal.c with the system's file dialog.
#include "../linux/portal.h"
