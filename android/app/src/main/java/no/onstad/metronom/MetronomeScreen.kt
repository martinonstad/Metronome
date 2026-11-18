package no.onstad.metronom

import android.content.pm.ApplicationInfo
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uniffi.metronom_ffi.Metronome
import uniffi.metronom_ffi.SongRecord

/**
 * The Metronome tab: the metronome, and the editor of the current song, in one view.
 *
 * At the top is the current song ([currentSong], or "No song"): tap it or ✎ for its title, notes
 * and setlists; ✕ puts it away and keeps the tempo. Below are the beat flash, the beats per bar
 * (− and + around the dots), the tempo (tap the number to type it, the slider, − and +), tap
 * tempo and Save, and Start/Stop. Save stores the tempo and beats in the song; with no song it
 * reads "Save as song" and names a new one.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun MetronomeScreen(
  metronome: Metronome,
  store: LibraryStore,
  currentSong: String?,
  onCurrentSongChange: (String?) -> Unit,
  onStart: () -> Unit,
  onStop: () -> Unit,
) {
  val playback = rememberPlayback(metronome, onStart, onStop)
  val running = playback.running
  val state = rememberMetronomeState(metronome)
  val settings = store.settings
  val song = currentSong?.let(store::song)
  var showDetails by remember { mutableStateOf(false) }
  var problem by remember { mutableStateOf<String?>(null) }
  var saving by remember { mutableStateOf(false) }
  val scope = rememberCoroutineScope()
  var diagnostics by remember { mutableStateOf("") }
  val debuggable = (LocalContext.current.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE) != 0

  val beatState = rememberBeatState(metronome, running, settings.visualOffsetMs)

  // Keep the display awake while the click is running (the `keep_screen_on` setting).
  KeepScreenOn(running && settings.keepScreenOn)

  // Debug builds show the audio diagnostics under the Start button.
  LaunchedEffect(debuggable) {
    while (debuggable) {
      diagnostics = "${metronome.diagnostics()} | ${metronome.sound()} vol ${"%.2f".format(metronome.volume())}"
      delay(500)
    }
  }

  val changed = song != null && (state.exactBpm != song.bpm || state.beats != song.beats.toInt())

  fun saveTempo(song: SongRecord) {
    if (saving) return
    saving = true
    problem = null
    val updated = SongRecord(song.title, state.exactBpm, state.beats.toUInt(), song.notes)
    scope.launch {
      (store.change { it.updateSong(song.title, updated) } as? Outcome.Failed)?.let { problem = it.message }
      saving = false
    }
  }

  // The content is spread over the screen when it fits and scrolls when it does not (a large
  // font setting, a small phone), so nothing is ever cut off.
  BoxWithConstraints(Modifier.fillMaxSize()) {
    val available = maxHeight
    Column(
      modifier =
        Modifier.fillMaxWidth()
          .verticalScroll(rememberScrollState())
          .padding(horizontal = 24.dp, vertical = 12.dp)
          .heightIn(min = available - 24.dp),
      horizontalAlignment = Alignment.CenterHorizontally,
      verticalArrangement = Arrangement.SpaceBetween,
    ) {
      Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        SongHeader(song, onEdit = { showDetails = true }, onClear = { onCurrentSongChange(null) })
        Spacer(Modifier.height(14.dp))
        FlashBar(beatState)
        Spacer(Modifier.height(14.dp))
        BeatsControl(state, beatState)
      }

      TempoControls(state, Modifier.padding(vertical = 12.dp))

      Column(Modifier.fillMaxWidth().padding(vertical = 12.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        FlowRow(
          horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterHorizontally),
          verticalArrangement = Arrangement.Center,
        ) {
          OutlinedButton(onClick = state::tap) { Text(stringResource(R.string.tap)) }
          when {
            song == null -> OutlinedButton(onClick = { showDetails = true }) { Text(stringResource(R.string.save_as_song)) }
            changed -> FilledTonalButton(onClick = { saveTempo(song) }, enabled = !saving) { Text(stringResource(R.string.save)) }
            else -> OutlinedButton(onClick = {}, enabled = false) { Text(stringResource(R.string.saved)) }
          }
        }
        problem?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(top = 8.dp)) }
      }

      Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        Button(onClick = playback::toggle, modifier = Modifier.fillMaxWidth().height(72.dp)) {
          Text(stringResource(if (running) R.string.stop else R.string.start), fontSize = 24.sp)
        }
        if (debuggable) {
          Spacer(Modifier.height(8.dp))
          Text(
            diagnostics,
            fontFamily = FontFamily.Monospace,
            fontSize = 11.sp,
            textAlign = TextAlign.Center,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
          )
        }
      }
    }
  }

  if (showDetails) {
    SongDetailsDialog(
      store = store,
      song = song,
      bpm = state.exactBpm,
      beats = state.beats,
      onDismiss = { showDetails = false },
      onSaved = {
        showDetails = false
        onCurrentSongChange(it)
      },
      onDeleted = {
        showDetails = false
        onCurrentSongChange(null)
      },
    )
  }
}

/** The current song's title and notes ("No song" when there is none), ✎ to edit them and ✕ to put the song away. */
@Composable
private fun SongHeader(song: SongRecord?, onEdit: () -> Unit, onClear: () -> Unit) {
  val editLabel = stringResource(R.string.edit_song_details)
  Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
    Column(Modifier.weight(1f).clickable(role = Role.Button, onClickLabel = editLabel, onClick = onEdit).padding(vertical = 4.dp)) {
      Text(
        song?.title ?: stringResource(R.string.no_song),
        style = MaterialTheme.typography.headlineSmall,
        color = if (song == null) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
        maxLines = 2,
        overflow = TextOverflow.Ellipsis,
      )
      if (song != null && song.notes.isNotBlank()) {
        Text(
          song.notes,
          style = MaterialTheme.typography.bodyMedium,
          color = MaterialTheme.colorScheme.onSurfaceVariant,
          maxLines = 2,
          overflow = TextOverflow.Ellipsis,
        )
      }
    }
    IconButton(onClick = onEdit) { Icon(Icons.Filled.Edit, contentDescription = editLabel) }
    if (song != null) {
      IconButton(onClick = onClear) { Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.clear_song)) }
    }
  }
}
