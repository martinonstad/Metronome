package no.onstad.metronom

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.roundToInt
import kotlinx.coroutines.launch
import uniffi.metronom_ffi.EntryRecord
import uniffi.metronom_ffi.Metronome
import uniffi.metronom_ffi.SetlistDetail

/**
 * Play a setlist. The current song's tempo and bar length are loaded into the metronome; **Next
 * song** is one tap and the click keeps running. A song that is not in the library is skipped.
 * [index] is the current position in the setlist and is kept by the caller, so coming back to the
 * same setlist resumes where it was.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GigScreen(
  metronome: Metronome,
  store: LibraryStore,
  stem: String,
  index: Int,
  onIndexChange: (Int) -> Unit,
  onStart: () -> Unit,
  onStop: () -> Unit,
  onBack: () -> Unit,
  onEdit: () -> Unit,
) {
  val runningState = rememberRunning(metronome)
  val running by runningState
  val beatState = rememberBeatState(metronome, running)

  var detail by remember { mutableStateOf<SetlistDetail?>(null) }
  var loaded by remember { mutableStateOf(false) }
  var keepScreenOn by remember { mutableStateOf(true) }
  var showList by remember { mutableStateOf(false) }

  LaunchedEffect(stem, store.revision, store.state) {
    if (store.state is LibraryStore.State.Ready) {
      detail = store.read { it.setlist(stem) }
      keepScreenOn = store.read { it.settings().keepScreenOn } ?: true
      loaded = true
    }
  }

  // Keep the display awake while the setlist is open, so it can be read from the music stand.
  val view = LocalView.current
  DisposableEffect(keepScreenOn) {
    view.keepScreenOn = keepScreenOn
    onDispose { view.keepScreenOn = false }
  }

  val entries = detail?.entries.orEmpty()
  fun playable(i: Int) = entries.getOrNull(i)?.song != null
  val current = index.coerceIn(0, maxOf(entries.lastIndex, 0))
  val song = entries.getOrNull(current)?.song

  // Opening the setlist makes the current song the click, unless it already is (a rotation, or
  // coming back from another screen), so a running click is not restarted for nothing. A song
  // that is missing from the library is stepped over.
  var entered by remember { mutableStateOf(false) }
  LaunchedEffect(detail) {
    if (entered || detail == null) return@LaunchedEffect
    entered = true
    val start = (current..entries.lastIndex).firstOrNull(::playable) ?: (current downTo 0).firstOrNull(::playable)
    val first = start?.let { entries[it].song } ?: return@LaunchedEffect
    if (start != current) onIndexChange(start)
    if (metronome.bpm() != first.bpm || metronome.beatsPerBar() != first.beats) metronome.load(first)
  }

  fun go(target: Int) {
    val next = entries.getOrNull(target)?.song ?: return
    onIndexChange(target)
    metronome.load(next)
  }
  val previous = (current - 1 downTo 0).firstOrNull(::playable)
  val next = (current + 1..entries.lastIndex).firstOrNull(::playable)

  Column(
    Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = 24.dp, vertical = 12.dp),
    horizontalAlignment = Alignment.CenterHorizontally,
  ) {
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
      Text(
        if (entries.isEmpty()) "" else stringResource(R.string.gig_position, current + 1, entries.size),
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 8.dp),
      )
    }

    if (!loaded) {
      Text(stringResource(R.string.opening_library), Modifier.padding(top = 24.dp))
      return@Column
    }
    if (detail == null) {
      Text(stringResource(R.string.setlist_gone), Modifier.padding(top = 24.dp))
      return@Column
    }
    if (song == null) {
      Column(
        Modifier.weight(1f).fillMaxWidth(),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
      ) {
        Text(stringResource(R.string.gig_nothing_to_play), style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.height(16.dp))
        OutlinedButton(onClick = onEdit) { Text(stringResource(R.string.edit_setlist)) }
      }
      return@Column
    }

    Spacer(Modifier.height(10.dp))
    FlashBar(beatState)
    Spacer(Modifier.height(10.dp))
    BeatDots(song.beats.toInt(), beatState)

    Spacer(Modifier.weight(1f))
    Text(
      song.title,
      style = MaterialTheme.typography.headlineMedium,
      textAlign = TextAlign.Center,
      maxLines = 2,
      overflow = TextOverflow.Ellipsis,
    )
    val tempoDescription = stringResource(R.string.tempo_description_fixed, song.bpm.roundToInt())
    Text(
      song.bpm.roundToInt().toString(),
      fontSize = 88.sp,
      style = MaterialTheme.typography.displayLarge,
      modifier = Modifier.semantics { contentDescription = tempoDescription },
    )
    Text(
      "BPM · " + pluralStringResource(R.plurals.beats_count, song.beats.toInt(), song.beats.toInt()),
      style = MaterialTheme.typography.bodyMedium,
      color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    if (song.notes.isNotBlank()) {
      Spacer(Modifier.height(6.dp))
      Text(
        song.notes,
        style = MaterialTheme.typography.bodyMedium,
        textAlign = TextAlign.Center,
        maxLines = 2,
        overflow = TextOverflow.Ellipsis,
      )
    }

    Spacer(Modifier.weight(1f))
    val upcoming = next?.let { entries[it].song }
    Text(
      if (upcoming == null) {
        stringResource(R.string.gig_last_song)
      } else {
        stringResource(R.string.gig_next_up, upcoming.title, upcoming.bpm.roundToInt())
      },
      style = MaterialTheme.typography.bodyMedium,
      color = MaterialTheme.colorScheme.onSurfaceVariant,
      maxLines = 1,
      overflow = TextOverflow.Ellipsis,
    )
    Spacer(Modifier.height(10.dp))
    FilledTonalButton(
      onClick = { if (running) onStop() else onStart() },
      modifier = Modifier.fillMaxWidth().height(64.dp),
    ) {
      Text(stringResource(if (running) R.string.stop else R.string.start), fontSize = 22.sp)
    }
    Spacer(Modifier.height(10.dp))
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
      val previousDescription = stringResource(R.string.gig_previous)
      OutlinedButton(
        onClick = { previous?.let(::go) },
        enabled = previous != null,
        modifier = Modifier.width(72.dp).height(72.dp).semantics { contentDescription = previousDescription },
      ) {
        Text("‹", fontSize = 28.sp)
      }
      Button(
        onClick = { next?.let(::go) },
        enabled = next != null,
        modifier = Modifier.weight(1f).height(72.dp),
      ) {
        Text(stringResource(if (next != null) R.string.gig_next else R.string.gig_end), fontSize = 24.sp)
      }
    }

    SongListHandle(onOpen = { showList = true })
  }

  if (showList) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = rememberCoroutineScope()
    ModalBottomSheet(onDismissRequest = { showList = false }, sheetState = sheetState) {
      SongList(
        entries = entries,
        current = current,
        onPick = { picked ->
          go(picked)
          scope.launch { sheetState.hide() }.invokeOnCompletion { showList = false }
        },
      )
    }
  }
}

/** The strip at the bottom: tap it or swipe up to open the list of songs. */
@Composable
private fun SongListHandle(onOpen: () -> Unit) {
  val description = stringResource(R.string.gig_open_list)
  Column(
    Modifier.fillMaxWidth()
      .padding(top = 4.dp)
      .draggable(
        orientation = Orientation.Vertical,
        state = rememberDraggableState {},
        onDragStopped = { velocity -> if (velocity < -400f) onOpen() },
      )
      .clickable(onClick = onOpen)
      .semantics { contentDescription = description }
      .padding(vertical = 8.dp),
    horizontalAlignment = Alignment.CenterHorizontally,
  ) {
    Box(
      Modifier.width(40.dp).height(4.dp).clip(CircleShape).background(MaterialTheme.colorScheme.outline)
    )
    Spacer(Modifier.height(4.dp))
    Text(
      stringResource(R.string.gig_song_list),
      style = MaterialTheme.typography.bodySmall,
      color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
  }
}

/** Every song in the setlist; tapping one jumps straight to it. A song that is missing is greyed out. */
@Composable
private fun SongList(entries: List<EntryRecord>, current: Int, onPick: (Int) -> Unit) {
  val listState = rememberLazyListState(initialFirstVisibleItemIndex = (current - 2).coerceAtLeast(0))
  LazyColumn(state = listState, modifier = Modifier.padding(horizontal = 16.dp)) {
    itemsIndexed(entries) { i, entry ->
      val song = entry.song
      val shape = RoundedCornerShape(12.dp)
      Row(
        Modifier.fillMaxWidth()
          .clip(shape)
          .background(if (i == current) MaterialTheme.colorScheme.secondaryContainer else Color.Transparent)
          .then(if (song != null) Modifier.clickable { onPick(i) } else Modifier)
          .padding(horizontal = 12.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
      ) {
        Text(
          "${i + 1}",
          style = MaterialTheme.typography.bodyMedium,
          color = MaterialTheme.colorScheme.onSurfaceVariant,
          modifier = Modifier.width(32.dp),
        )
        Column(Modifier.weight(1f)) {
          Text(entry.title, style = MaterialTheme.typography.bodyLarge)
          if (song == null) {
            Text(
              stringResource(R.string.missing_song),
              style = MaterialTheme.typography.bodySmall,
              color = MaterialTheme.colorScheme.error,
            )
          }
        }
        if (song != null) {
          Text(
            "${song.bpm.roundToInt()} · ${song.beats}",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
          )
        }
      }
    }
  }
}
