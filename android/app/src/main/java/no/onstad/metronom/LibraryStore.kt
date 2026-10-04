package no.onstad.metronom

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.metronom_ffi.BandGroup
import uniffi.metronom_ffi.LibraryException
import uniffi.metronom_ffi.SongLibrary
import uniffi.metronom_ffi.SongRecord
import uniffi.metronom_ffi.WarningRecord

/** The result of a change: the value, or a message the screen can show. */
sealed interface Outcome<out T> {
  data class Ok<T>(val value: T) : Outcome<T>

  data class Failed(val message: String) : Outcome<Nothing>
}

/**
 * Owns the song library for the screens. The library lives in Rust and saves every change at
 * once; this class opens it off the main thread, runs every call on a background thread, and
 * keeps the lists the screens show as Compose state. A change that has started always finishes
 * and refreshes the lists, even if the screen that asked for it is already gone.
 */
class LibraryStore(private val root: File, private val scope: CoroutineScope) {
  sealed interface State {
    data object Opening : State

    data object Ready : State

    data class Failed(val message: String) : State
  }

  private var library: SongLibrary? = null

  var state: State by mutableStateOf(State.Opening)
    private set

  /** Every song, in file order. */
  var songs: List<SongRecord> by mutableStateOf(emptyList())
    private set

  /** The setlists grouped by band (alphabetical, setlists without a band last). */
  var setlistGroups: List<BandGroup> by mutableStateOf(emptyList())
    private set

  /** Counts up with every change, so a screen that shows one setlist knows to reload it. */
  var revision: Int by mutableIntStateOf(0)
    private set

  /** The bands and projects in use, alphabetical. */
  val bands: List<String>
    get() = setlistGroups.mapNotNull { it.band }

  /** What was odd in the files when the library opened (ignored rows, bad values, …). */
  var warnings: List<WarningRecord> by mutableStateOf(emptyList())
    private set

  init {
    scope.launch(Dispatchers.IO) {
      try {
        val opened = SongLibrary.open(root.absolutePath)
        val songList = opened.songs()
        val groups = opened.setlistsByBand()
        val warningList = opened.warnings()
        library = opened
        withContext(Dispatchers.Main) {
          songs = songList
          setlistGroups = groups
          warnings = warningList
          state = State.Ready
        }
      } catch (e: LibraryException) {
        withContext(Dispatchers.Main) { state = State.Failed(describe(e)) }
      }
    }
  }

  /** The song titled [title] (ignoring case), if it exists. */
  fun song(title: String): SongRecord? = songs.firstOrNull { it.title.equals(title.trim(), ignoreCase = true) }

  /**
   * Run [block] on the library off the main thread, then refresh the lists. A library error
   * becomes an [Outcome.Failed] with a message for the user.
   */
  suspend fun <T> change(block: (SongLibrary) -> T): Outcome<T> =
    withContext(Dispatchers.IO + NonCancellable) {
      val lib = library ?: return@withContext Outcome.Failed("The song library is still opening.")
      try {
        val value = block(lib)
        val songList = lib.songs()
        val groups = lib.setlistsByBand()
        withContext(Dispatchers.Main) {
          songs = songList
          setlistGroups = groups
          revision++
        }
        Outcome.Ok(value)
      } catch (e: LibraryException) {
        Outcome.Failed(describe(e))
      }
    }

  /** Run a read-only query off the main thread; `null` while the library is still opening. */
  suspend fun <T> read(block: (SongLibrary) -> T): T? =
    withContext(Dispatchers.IO) {
      val lib = library ?: return@withContext null
      try {
        block(lib)
      } catch (e: LibraryException) {
        null
      }
    }
}

/** A library error as a sentence for the user. */
fun describe(e: LibraryException): String =
  when (e) {
    is LibraryException.DuplicateTitle -> "There's already a song called “${e.title}”."
    is LibraryException.EmptyName -> "Enter a name."
    is LibraryException.NoSuchSong -> "There's no song called “${e.title}”."
    is LibraryException.NoSuchSetlist -> "That setlist no longer exists."
    is LibraryException.PositionOutOfRange -> "That position is outside the setlist."
    is LibraryException.FileUnreadable ->
      "${e.file} isn't valid text, so it was left untouched and can't be changed here. Fix or remove it, then restart the app."
    is LibraryException.ReadOnly -> "${e.file} was written by a newer version of the app and isn't changed."
    is LibraryException.Storage -> "Couldn't read or save your songs: ${e.detail}"
    is LibraryException.BadArchive -> e.detail
  }
