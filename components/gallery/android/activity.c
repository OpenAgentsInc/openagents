// Firm reason: 6 (the NDK's NativeActivity ABI; every behaviour is the gallery's Coax).
// The component library as an Android app (components/gallery/android/build.sh --apk): the
// activity's android_main copies the APK's assets (the shader, fonts/ and icons/) into the app's
// own files folder, hands the activity to the shared Android host (ui-host-android.h) and runs
// the gallery's main (components/gallery/linux/main.c, compiled as gallery_main) in its window.
#include "ui-host-android.h"
#include <android/asset_manager.h>
#include <android_native_app_glue.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

int gallery_main(int argc, char **argv);

static void copy_asset(AAssetManager *m, const char *name, const char *into) {
  AAsset *a = AAssetManager_open(m, name, AASSET_MODE_STREAMING);
  if (!a) return;
  char path[1024]; snprintf(path, sizeof path, "%s/%s", into, name);
  FILE *f = fopen(path, "wb");
  if (f) {
    char buf[65536]; int n;
    while ((n = AAsset_read(a, buf, sizeof buf)) > 0) fwrite(buf, 1, (size_t)n, f);
    fclose(f);
  }
  AAsset_close(a);
}
static void copy_dir(AAssetManager *m, const char *dir, const char *into) {
  char path[1024]; snprintf(path, sizeof path, "%s/%s", into, dir);
  mkdir(path, 0700);
  AAssetDir *d = AAssetManager_openDir(m, dir);
  const char *file;
  while (d && (file = AAssetDir_getNextFileName(d))) { char name[512]; snprintf(name, sizeof name, "%s/%s", dir, file); copy_asset(m, name, into); }
  if (d) AAssetDir_close(d);
}

void android_main(struct android_app *app) {
  const char *files = app->activity->internalDataPath;
  AAssetManager *m = app->activity->assetManager;
  copy_asset(m, "components.spv", files);
  copy_dir(m, "fonts", files);
  copy_dir(m, "icons", files);
  setenv("UI_RESOURCES", files, 1);
  ui_android_app(app);
  char *argv[] = {"Components", NULL};
  gallery_main(1, argv);
  ANativeActivity_finish(app->activity);
}
