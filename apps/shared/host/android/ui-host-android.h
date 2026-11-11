// What the Android host (ui-host.c) adds to the shared contract (../linux/ui-host.h) for a window.
// An app built as a NativeActivity (the NDK's android_native_app_glue, with UI_ANDROID_WINDOW
// defined when ui-host.c is compiled) hands the host its android_app from android_main before
// ui_open(app, 0); the host then draws to the activity's window and reads its events in ui_frame.
#ifndef UI_HOST_ANDROID_H
#define UI_HOST_ANDROID_H
struct android_app;
void ui_android_app(struct android_app *app);
#endif
