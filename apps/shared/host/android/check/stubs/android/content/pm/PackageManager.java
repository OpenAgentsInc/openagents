package android.content.pm;
public class PackageManager {
  public PackageInfo getPackageInfo(String name, int flags) {
    if (!"com.openagents.everglade".equals(name) || flags != 0) throw new IllegalArgumentException("another package");
    return new PackageInfo();
  }
}
