# libGDX loads several classes reflectively from native code and from its own backends, so they
# must survive shrinking. Without these the release build crashes on start-up while the debug
# build works, which is a slow failure to diagnose.
-keep class com.badlogic.gdx.** { *; }
-keep class com.badlogic.gdx.backends.android.** { *; }
-keepclassmembers class com.badlogic.gdx.backends.android.AndroidInput* { <init>(...); }
-keep class com.badlogic.gdx.graphics.g3d.particles.** { *; }
-dontwarn com.badlogic.gdx.**
-dontwarn com.badlogic.gdx.jnigen.**
-dontwarn org.lwjgl.**

# The game's own code holds no reflection: the simulation, content loader and save format are all
# explicit, precisely so GWT and RoboVM work. Nothing extra needs keeping here.
