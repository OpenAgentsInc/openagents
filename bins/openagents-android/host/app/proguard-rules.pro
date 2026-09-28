# OpenAgents for Android release shrinking (R8).
#
# Rust binds the JNI entry points by name
# (Java_com_openagents_app_OpenAgentsNative_*), so the class and its native
# methods keep their names. Rust calls back only into java.lang.String.
-keep class com.openagents.app.OpenAgentsNative { native <methods>; }
-keepclasseswithmembernames,includedescriptorclasses class * { native <methods>; }

# Keep line numbers in stack traces; the mapping file stays with the build.
-keepattributes SourceFile,LineNumberTable
-renamesourcefileattribute SourceFile
