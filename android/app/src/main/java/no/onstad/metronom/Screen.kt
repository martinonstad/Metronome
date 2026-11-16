package no.onstad.metronom

import androidx.compose.runtime.saveable.Saver

/** The screens of the app. Navigation is a single current screen; Back goes to its parent. */
sealed interface Screen {
  /** The manual metronome, the home screen. */
  data object Manual : Screen

  data object Songs : Screen

  data object Setlists : Screen

  /** Version, license, privacy statement and the notices of the libraries inside the app. */
  data object About : Screen

  /** Sound, volume, flash offset, keep-awake, and export/import of your files. */
  data object Settings : Screen

  /** Playing a setlist from its list of songs; [stem] is the setlist's file name without `.md`. */
  data class Gig(val stem: String) : Screen

  /** The setlist editor; [stem] is the setlist's file name without `.md`. */
  data class EditSetlist(val stem: String) : Screen

  /**
   * The song page; [title] is the song being edited, or `null` for a new song. [fromManual] is
   * set when "Save as song" on the main screen opened it, so that Back and Save return there.
   */
  data class EditSong(val title: String?, val fromManual: Boolean = false) : Screen

  /** Where Back goes from here. */
  val parent: Screen?
    get() =
      when (this) {
        Manual -> null
        Songs -> Manual
        Setlists -> Manual
        Settings -> Manual
        About -> Settings
        is EditSetlist -> Setlists
        is Gig -> Manual
        is EditSong -> if (fromManual) Manual else Songs
      }
}

/** Keeps the current screen across rotation and the activity being recreated. */
val ScreenSaver =
  Saver<Screen, String>(
    save = {
      when (it) {
        Screen.Manual -> "manual"
        Screen.Songs -> "songs"
        Screen.Setlists -> "setlists"
        Screen.Settings -> "settings"
        Screen.About -> "about"
        is Screen.EditSetlist -> "setlist:${it.stem}"
        is Screen.Gig -> "gig:${it.stem}"
        is Screen.EditSong ->
          when {
            it.title != null -> "edit:${it.title}"
            it.fromManual -> "new:manual"
            else -> "new"
          }
      }
    },
    restore = {
      when {
        it == "songs" -> Screen.Songs
        it == "setlists" -> Screen.Setlists
        it == "settings" -> Screen.Settings
        it == "about" -> Screen.About
        it.startsWith("gig:") -> Screen.Gig(it.removePrefix("gig:"))
        it.startsWith("setlist:") -> Screen.EditSetlist(it.removePrefix("setlist:"))
        it == "new" -> Screen.EditSong(null)
        it == "new:manual" -> Screen.EditSong(null, fromManual = true)
        it.startsWith("edit:") -> Screen.EditSong(it.removePrefix("edit:"))
        else -> Screen.Manual
      }
    },
  )

