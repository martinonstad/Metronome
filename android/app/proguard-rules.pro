# Rules for the release build (R8 shrinking). The Rust library is reached through JNA and the
# Kotlin bindings that UniFFI generates; JNA finds their classes and fields by reflection, so
# neither may be renamed or removed.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keep class uniffi.** { *; }
-dontwarn java.awt.**
