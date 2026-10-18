# UniFFI and JNA use generated JNI declarations. Release minification is off
# until the native binding has a measured shrinker configuration.
-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }
