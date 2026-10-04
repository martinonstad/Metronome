package no.onstad.metronom

import android.content.Context
import android.content.pm.ApplicationInfo
import android.content.res.Configuration

/**
 * Debug builds only: force this app's font scale, light/dark theme and screen density (a larger
 * `density` makes the screen hold fewer dp, like a smaller phone) without touching the phone's
 * own settings, to check layouts at large text sizes, on small screens and in both themes. Put
 * lines such as `fontScale=2.0`, `night=yes` (or `no`) and `density=1.6` into
 * `files/debug_config.txt`:
 *
 *     adb shell 'run-as no.onstad.metronom sh -c "printf \"fontScale=2.0\nnight=no\n\" > files/debug_config.txt"'
 *
 * Remove the file (and restart the app) to go back to the phone's settings. Release builds
 * ignore the file.
 */
internal fun Context.withDebugOverrides(): Context {
  if (applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE == 0) return this
  val file = getFileStreamPath("debug_config.txt")
  if (!file.isFile) return this
  val values =
    file.readLines().mapNotNull { line -> line.split("=", limit = 2).takeIf { it.size == 2 }?.let { it[0].trim() to it[1].trim() } }.toMap()
  val config = Configuration(resources.configuration)
  values["fontScale"]?.toFloatOrNull()?.let { config.fontScale = it }
  values["density"]?.toFloatOrNull()?.let { config.densityDpi = (config.densityDpi * it).toInt() }
  when (values["night"]) {
    "yes" -> config.uiMode = (config.uiMode and Configuration.UI_MODE_NIGHT_MASK.inv()) or Configuration.UI_MODE_NIGHT_YES
    "no" -> config.uiMode = (config.uiMode and Configuration.UI_MODE_NIGHT_MASK.inv()) or Configuration.UI_MODE_NIGHT_NO
  }
  return createConfigurationContext(config)
}
