package android.content;
// A stand-in for crash-reports-check.sh: the three calls the crash reporter makes on its activity.
public abstract class Context {
  public abstract Object getSystemService(String name);
  public abstract android.content.pm.PackageManager getPackageManager();
  public abstract String getPackageName();
}
