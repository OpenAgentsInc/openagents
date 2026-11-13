package android.app;
public class NativeActivity extends android.content.Context {
  public Object getSystemService(String name) { return "activity".equals(name) ? new ActivityManager() : null; }
  public android.content.pm.PackageManager getPackageManager() { return new android.content.pm.PackageManager(); }
  public String getPackageName() { return "com.openagents.everglade"; }
}
