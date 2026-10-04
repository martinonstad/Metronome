package no.onstad.metronom

import androidx.compose.runtime.saveable.Saver

/** The screens of the app. Navigation is a single current screen; Back goes to its parent. */
sealed interface Screen {
  /** The manual metronome, the home screen. */
  data object Manual : Screen

  data object Songs : Screen

  data object Setlists : Screen

  /** Playing a setlist; [stem] is the setlist's file name without `.md`. */
  data class Gig(val stem: String) : Screen

  /** The setlist editor; [stem] is the setlist's file name without `.md`. */
  data class EditSetlist(val stem: String) : Screen

  /** The song editor; [title] is the song being edited, or `null` for a new song. */
  data class EditSong(val title: String?) : Screen

  /** Where Back goes from here. */
  val parent: Screen?
    get() =
      when (this) {
        Manual -> null
        Songs -> Manual
        Setlists -> Manual
        is EditSetlist -> Setlists
        is Gig -> Manual
        is EditSong -> Songs
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
        is Screen.EditSetlist -> "setlist:${it.stem}"
        is Screen.Gig -> "gig:${it.stem}"
        is Screen.EditSong -> if (it.title == null) "new" else "edit:${it.title}"
      }
    },
    restore = {
      when {
        it == "songs" -> Screen.Songs
        it == "setlists" -> Screen.Setlists
        it.startsWith("gig:") -> Screen.Gig(it.removePrefix("gig:"))
        it.startsWith("setlist:") -> Screen.EditSetlist(it.removePrefix("setlist:"))
        it == "new" -> Screen.EditSong(null)
        it.startsWith("edit:") -> Screen.EditSong(it.removePrefix("edit:"))
        else -> Screen.Manual
      }
    },
  )

