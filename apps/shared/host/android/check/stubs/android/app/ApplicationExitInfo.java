package android.app;
// A stand-in: one exit record, as the check's records say it (ActivityManager below).
public class ApplicationExitInfo {
  int reason; long when; int status; String words; byte[] trace;
  public int getReason() { return reason; }
  public long getTimestamp() { return when; }
  public int getStatus() { return status; }
  public String getDescription() { return words; }
  public java.io.InputStream getTraceInputStream() throws java.io.IOException {
    if (words != null && words.equals("unreadable")) throw new java.io.IOException("no trace");
    return trace == null ? null : new java.io.ByteArrayInputStream(trace);
  }
}
