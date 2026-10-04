package no.onstad.metronom

import androidx.compose.runtime.saveable.Saver

/** The screens of the app. Navigation is a single current screen; Back goes to its parent. */
sealed interface Screen {
  /** The manual metronome, the home screen. */
  data object Manual : Screen

  data object Songs : Screen

  /** The song editor; [title] is the song being edited, or `null` for a new song. */
  data class EditSong(val title: String?) : Screen

  /** Where Back goes from here. */
  val parent: Screen?
    get() =
      when (this) {
        Manual -> null
        Songs -> Manual
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
        is Screen.EditSong -> if (it.title == null) "new" else "edit:${it.title}"
      }
    },
    restore = {
      when {
        it == "songs" -> Screen.Songs
        it == "new" -> Screen.EditSong(null)
        it.startsWith("edit:") -> Screen.EditSong(it.removePrefix("edit:"))
        else -> Screen.Manual
      }
    },
  )

