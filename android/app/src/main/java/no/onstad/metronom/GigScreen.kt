package no.onstad.metronom

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import kotlinx.coroutines.delay
import uniffi.metronom_ffi.EntryRecord
import uniffi.metronom_ffi.Metronome
import uniffi.metronom_ffi.SetlistDetail

/** What the metronome is set to. */
private data class Loaded(val bpm: Double, val beats: UInt)

private fun Metronome.loaded() = Loaded(bpm(), beatsPerBar())

/**
 * Play a setlist: the whole list of songs, with the beat flash above it. Tap a song and the click
 * starts with its tempo and bar length; tap another and the click keeps running and switches on
 * the next beat; tap the song that is playing and the click stops. A song that is not in the
 * library cannot be tapped.
 *
 * [selected] is the song tapped last (-1 for none) and is kept by the caller, so the row is still
 * marked when you come back to the same setlist. Opening the page changes nothing in the
 * metronome.
 */
@Composable
fun GigScreen(
  metronome: Metronome,
  store: LibraryStore,
  stem: String,
  selected: Int,
  onSelect: (Int) -> Unit,
  onStart: () -> Unit,
  onStop: () -> Unit,
  onBack: () -> Unit,
  onEdit: () -> Unit,
) {
  val playback = rememberPlayback(metronome, onStart, onStop)
  val beatState = rememberBeatState(metronome, playback.running, store.settings.visualOffsetMs)

  var detail by remember { mutableStateOf<SetlistDetail?>(null) }
  var ready by remember { mutableStateOf(false) }
  LaunchedEffect(stem, store.revision, store.state) {
    if (store.state is LibraryStore.State.Ready) {
      detail = store.read { it.setlist(stem) }
      ready = true
    }
  }

  // Keep the display awake while the setlist is open, so it can be read from the music stand.
  KeepScreenOn(store.settings.keepScreenOn)

  // What the metronome is set to right now. It is followed so that a row only shows as playing
  // while the click really has that song's tempo and bar length: a tempo changed on the main
  // screen, or a click started there, does not light up a song.
  var loaded by remember { mutableStateOf(metronome.loaded()) }
  LaunchedEffect(metronome) {
    while (true) {
      loaded = metronome.loaded()
      delay(500)
    }
  }

  val entries = detail?.entries.orEmpty()
  val current = entries.getOrNull(selected)?.song
  val playing =
    if (playback.running && current != null && current.bpm == loaded.bpm && current.beats == loaded.beats) selected else -1

  fun tap(index: Int) {
    val song = entries.getOrNull(index)?.song ?: return
    if (index == playing) {
      playback.stop()
      return
    }
    // A running click switches on the next beat, which becomes beat 1 of this song.
    metronome.load(song)
    loaded = metronome.loaded()
    onSelect(index)
    playback.start()
  }

  Column(Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = 24.dp, vertical = 12.dp)) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
      TextButton(onClick = onBack) { Text("‹ ${stringResource(R.string.back)}") }
      Text(
        listOfNotNull(detail?.band, detail?.name).joinToString(" · "),
        style = MaterialTheme.typography.titleMedium,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        textAlign = TextAlign.Center,
        modifier = Modifier.weight(1f),
      )
      if (detail != null) TextButton(onClick = onEdit) { Text(stringResource(R.string.edit)) }
    }

    if (!ready) {
      Text(stringResource(R.string.opening_library), Modifier.padding(top = 24.dp))
    } else if (detail == null) {
      Text(stringResource(R.string.setlist_gone), Modifier.padding(top = 24.dp))
    } else if (entries.none { it.song != null }) {
      Column(
        Modifier.fillMaxWidth().padding(top = 48.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
      ) {
        Text(stringResource(R.string.gig_nothing_to_play), style = MaterialTheme.typography.titleMedium)
        OutlinedButton(onClick = onEdit) { Text(stringResource(R.string.edit_setlist)) }
      }
    } else {
      Column(Modifier.fillMaxWidth().padding(vertical = 10.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        FlashBar(beatState)
        Spacer(Modifier.height(10.dp))
        BeatDots(loaded.beats.toInt(), beatState)
      }
      // Opens with the song tapped last in view.
      val listState = rememberLazyListState(initialFirstVisibleItemIndex = (selected - 2).coerceAtLeast(0))
      LazyColumn(state = listState, modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        itemsIndexed(entries) { index, entry ->
          SongRow(
            index = index,
            entry = entry,
            playing = index == playing,
            marked = index == selected,
            onTap = { tap(index) },
          )
        }
      }
    }
  }
}

/**
 * One song of the setlist: its number, title and notes, with the tempo and beats on the right.
 * The song that is playing is highlighted and shows a stop sign in place of its number; the one
 * tapped last stays lightly marked when the click is stopped. A missing song is greyed out.
 */
@Composable
private fun SongRow(index: Int, entry: EntryRecord, playing: Boolean, marked: Boolean, onTap: () -> Unit) {
  val song = entry.song
  val colors = MaterialTheme.colorScheme
  val background =
    when {
      playing -> colors.primaryContainer
      marked -> colors.surfaceVariant
      else -> Color.Transparent
    }
  val content =
    when {
      song == null -> colors.onSurfaceVariant
      playing -> colors.onPrimaryContainer
      else -> colors.onSurface
    }
  val detailColor = if (playing) colors.onPrimaryContainer else colors.onSurfaceVariant
  val playingLabel = stringResource(R.string.playing)
  val action = stringResource(if (playing) R.string.gig_stop_song else R.string.gig_start_song)
  Row(
    Modifier.fillMaxWidth()
      .clip(RoundedCornerShape(12.dp))
      .background(background)
      .then(
        if (song != null) {
          Modifier.clickable(role = Role.Button, onClickLabel = action, onClick = onTap).semantics {
            if (playing) stateDescription = playingLabel
          }
        } else {
          Modifier
        }
      )
      .heightIn(min = 64.dp)
      .padding(horizontal = 12.dp, vertical = 10.dp),
    verticalAlignment = Alignment.CenterVertically,
  ) {
    Text(
      if (playing) "■" else "${index + 1}",
      style = MaterialTheme.typography.titleMedium,
      color = detailColor,
      modifier = Modifier.width(36.dp),
    )
    Column(Modifier.weight(1f)) {
      Text(entry.title, style = MaterialTheme.typography.titleLarge, color = content)
      if (song == null) {
        Text(stringResource(R.string.missing_song), style = MaterialTheme.typography.bodySmall, color = colors.error)
      } else if (song.notes.isNotBlank()) {
        Text(
          song.notes,
          style = MaterialTheme.typography.bodyMedium,
          color = detailColor,
          maxLines = 2,
          overflow = TextOverflow.Ellipsis,
        )
      }
    }
    if (song != null) {
      Column(Modifier.padding(start = 12.dp), horizontalAlignment = Alignment.End) {
        Text(song.bpm.roundToInt().toString(), style = MaterialTheme.typography.headlineSmall, color = content)
        Text(
          pluralStringResource(R.plurals.beats_count, song.beats.toInt(), song.beats.toInt()),
          style = MaterialTheme.typography.bodySmall,
          color = detailColor,
        )
      }
    }
  }
}
