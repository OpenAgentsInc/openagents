package android.app;
// A stand-in: the exit records are the system property oa.check.records, newest first, as
// "REASON:TIMESTAMP:STATUS:WORDS:TOMBSTONE_FILE;..." (an empty file name: no tombstone).
public class ActivityManager {
  public java.util.List<ApplicationExitInfo> getHistoricalProcessExitReasons(String packageName, int pid, int maxNum) throws java.io.IOException {
    if (packageName != null || pid != 0) throw new IllegalArgumentException("the reporter asks for its own package's records, every process");
    java.util.ArrayList<ApplicationExitInfo> out = new java.util.ArrayList<>();
    String spec = System.getProperty("oa.check.records", "");
    for (String row : spec.split(";")) {
      if (row.isEmpty() || out.size() >= maxNum) continue;
      String[] f = row.split(":", -1);
      ApplicationExitInfo i = new ApplicationExitInfo();
      i.reason = Integer.parseInt(f[0]); i.when = Long.parseLong(f[1]); i.status = Integer.parseInt(f[2]); i.words = f[3];
      if (!f[4].isEmpty()) i.trace = java.nio.file.Files.readAllBytes(java.nio.file.Paths.get(f[4]));
      out.add(i);
    }
    return out;
  }
}
