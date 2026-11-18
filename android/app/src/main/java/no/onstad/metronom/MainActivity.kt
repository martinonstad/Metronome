package no.onstad.metronom

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.core.content.ContextCompat
import kotlinx.coroutines.delay
import no.onstad.metronom.theme.MetronomTheme
import uniffi.metronom_ffi.Metronome
import uniffi.metronom_ffi.SongRecord

class MainActivity : ComponentActivity() {
  // The notification permission only decides whether the "playing" notification is visible;
  // playback starts either way.
  private val notificationPermission =
    registerForActivityResult(ActivityResultContracts.RequestPermission()) { startPlayback() }

  override fun attachBaseContext(newBase: Context) {
    super.attachBaseContext(newBase.withDebugOverrides())
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)

    enableEdgeToEdge()
    setContent {
      MetronomTheme {
        Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) { App() }
      }
    }
  }

  /**
   * The tab bar on top and the tab's page below. Back follows [Nav.back] and leaves the app from
   * the Metronome tab. The app also keeps the current song (the one the Metronome tab shows and
   * edits) and the row tapped last in a list.
   */
  @Composable
  private fun App() {
    val metronome = metronomApp.metronome
    val library = metronomApp.library
    var nav by rememberSaveable(stateSaver = NavSaver) { mutableStateOf(Nav()) }
    var currentSong by rememberSaveable { mutableStateOf<String?>(null) }
    // The row tapped last, in the list with this key; kept so the list still shows it when you come back.
    var selectedList by rememberSaveable { mutableStateOf<String?>(null) }
    var selectedIndex by rememberSaveable { mutableIntStateOf(-1) }

    KeepTempo(metronome)

    // The current song follows the library: a rename keeps it (titles compare ignoring case), a
    // song that is gone (deleted, or replaced by an import) is put away.
    LaunchedEffect(library.songs, library.state) {
      if (library.state is LibraryStore.State.Ready) currentSong = currentSong?.let { library.song(it)?.title }
    }

    fun makeCurrent(list: ListId, index: Int, song: SongRecord) {
      // A running click switches on the next beat, which becomes beat 1 of this song.
      metronome.load(song)
      currentSong = song.title
      selectedList = list.key
      selectedIndex = index
    }

    BackHandler(enabled = nav.back() != null) { nav.back()?.let { nav = it } }

    Column(Modifier.fillMaxSize().safeDrawingPadding()) {
      TabBar(nav.tab, onSelect = { nav = nav.select(it) })
      Box(Modifier.weight(1f).fillMaxWidth()) {
        when (nav.tab) {
          Tab.Metronome ->
            MetronomeScreen(
              metronome = metronome,
              store = library,
              currentSong = currentSong,
              onCurrentSongChange = { currentSong = it },
              onStart = ::onStartRequested,
              onStop = ::stopPlayback,
            )
          Tab.Lists ->
            when (val list = nav.list) {
              null ->
                ListsScreen(
                  store = library,
                  onOpen = { nav = nav.openList(it) },
                  onCreated = { nav = nav.openList(ListId.Setlist(it), edit = true) },
                )
              else ->
                ListScreen(
                  metronome = metronome,
                  store = library,
                  list = list,
                  editing = nav.editing,
                  selected = if (selectedList == list.key) selectedIndex else -1,
                  currentSong = currentSong,
                  onStart = ::onStartRequested,
                  onStop = ::stopPlayback,
                  onPick = { index, song -> makeCurrent(list, index, song) },
                  onEditSong = { index, song ->
                    makeCurrent(list, index, song)
                    nav = nav.toMetronomeFromList()
                  },
                  onClearSelection = { if (selectedList == list.key) selectedIndex = -1 },
                  onEditingChange = { nav = nav.copy(editing = it) },
                  onOpenList = { id, edit -> nav = nav.openList(id, edit) },
                  onClosed = { nav = nav.copy(list = null, editing = false) },
                  onNewSong = {
                    currentSong = null
                    nav = nav.toMetronomeFromList()
                  },
                )
            }
          Tab.Settings ->
            if (nav.about) {
              AboutScreen()
            } else {
              SettingsScreen(store = library, metronome = metronome, onOpenAbout = { nav = nav.copy(about = true) })
            }
        }
      }
    }
  }

  /**
   * Keeps the metronome's tempo and bar length in the saved state. Rotation keeps the metronome,
   * but a new process (Android closed the app in the background) starts with a fresh one at 120
   * and 4; then the values are put back before any page reads them, so the current song never
   * shows, or saves, a tempo you did not set.
   */
  @Composable
  private fun KeepTempo(metronome: Metronome) {
    var bpm by rememberSaveable { mutableStateOf<Double?>(null) }
    var beats by rememberSaveable { mutableIntStateOf(0) }
    remember {
      val saved = bpm
      if (saved != null && (metronome.bpm() != saved || metronome.beatsPerBar().toInt() != beats)) {
        metronome.setBpm(saved)
        metronome.setBeatsPerBar(beats.toUInt())
      }
      true
    }
    LaunchedEffect(metronome) {
      while (true) {
        bpm = metronome.bpm()
        beats = metronome.beatsPerBar().toInt()
        delay(500)
      }
    }
  }

  private fun onStartRequested() {
    val needsPermission =
      Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
        ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) !=
          PackageManager.PERMISSION_GRANTED
    if (needsPermission) {
      notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
    } else {
      startPlayback()
    }
  }

  private fun startPlayback() {
    ContextCompat.startForegroundService(this, Intent(this, PlaybackService::class.java))
  }

  private fun stopPlayback() {
    stopService(Intent(this, PlaybackService::class.java))
  }
}
