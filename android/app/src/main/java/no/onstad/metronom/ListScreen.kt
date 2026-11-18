package no.onstad.metronom

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uniffi.metronom_ffi.EntryRecord
import uniffi.metronom_ffi.Metronome
import uniffi.metronom_ffi.SetlistDetail
import uniffi.metronom_ffi.SongLibrary
import uniffi.metronom_ffi.SongRecord

/**
 * One list: every song ([ListId.AllSongs]) or one setlist, with the beat flash above it.
 *
 * Tap a song and the click starts with its tempo and bar length; tap another and the click keeps
 * running and switches on the next beat; tap the song that is playing and the click stops. A
 * tapped song becomes the current song (the one the Metronome tab shows). ✎ makes it current and
 * opens the Metronome tab to edit it. A song that is not in the library cannot be played or edited.
 *
 * A setlist has an Edit mode ([editing]): move, remove and add songs, change the name and band,
 * copy or delete the setlist. All songs has a search field and "New song" instead.
 *
 * [selected] is the row tapped last in this list (-1 for none), kept by the caller so it is still
 * marked when you come back. A row is shown as playing while the click runs, it is [selected], and
 * its song is still the [currentSong].
 */
@Composable
fun ListScreen(
  metronome: Metronome,
  store: LibraryStore,
  list: ListId,
  editing: Boolean,
  selected: Int,
  currentSong: String?,
  onStart: () -> Unit,
  onStop: () -> Unit,
  onPick: (index: Int, song: SongRecord) -> Unit,
  onEditSong: (index: Int, song: SongRecord) -> Unit,
  onClearSelection: () -> Unit,
  onEditingChange: (Boolean) -> Unit,
  onOpenList: (ListId, editing: Boolean) -> Unit,
  onClosed: () -> Unit,
  onNewSong: () -> Unit,
) {
  val playback = rememberPlayback(metronome, onStart, onStop)
  val beatState = rememberBeatState(metronome, playback.running, store.settings.visualOffsetMs)

  // Keep the display awake while a list is open, so it can be read from the music stand.
  KeepScreenOn(store.settings.keepScreenOn)

  // The setlist's lines, or every song as lines of the "All songs" list.
  var detail by remember { mutableStateOf<SetlistDetail?>(null) }
  var ready by remember { mutableStateOf(false) }
  LaunchedEffect(list, store.revision, store.state) {
    if (store.state is LibraryStore.State.Ready) {
      detail = (list as? ListId.Setlist)?.let { setlist -> store.read { it.setlist(setlist.stem) } }
      ready = true
    }
  }
  val entries: List<EntryRecord> =
    when (list) {
      ListId.AllSongs -> store.songs.map { EntryRecord(it.title, it) }
      is ListId.Setlist -> detail?.entries.orEmpty()
    }

  // The bar length the click has now, for the dots.
  var beats by remember { mutableIntStateOf(metronome.beatsPerBar().toInt()) }
  LaunchedEffect(metronome) {
    while (true) {
      beats = metronome.beatsPerBar().toInt()
      delay(500)
    }
  }

  val playing =
    entries.getOrNull(selected)?.song.let { song ->
      if (playback.running && song != null && song.title == currentSong) selected else -1
    }

  fun tap(index: Int) {
    val song = entries.getOrNull(index)?.song ?: return
    if (index == playing) {
      playback.stop()
      return
    }
    // A running click switches on the next beat, which becomes beat 1 of this song.
    onPick(index, song)
    beats = song.beats.toInt()
    playback.start()
  }

  // Back leaves Edit mode first.
  BackHandler(enabled = editing) { onEditingChange(false) }

  Column(Modifier.fillMaxSize().padding(horizontal = 24.dp, vertical = 12.dp)) {
    val setlist = detail
    Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
      Text(
        when (list) {
          ListId.AllSongs -> stringResource(R.string.all_songs)
          is ListId.Setlist -> listOfNotNull(setlist?.band, setlist?.name).joinToString(" · ")
        },
        style = MaterialTheme.typography.titleMedium,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        modifier = Modifier.weight(1f),
      )
      if (list is ListId.Setlist && setlist != null) {
        TextButton(onClick = { onEditingChange(!editing) }) {
          Text(stringResource(if (editing) R.string.done else R.string.edit))
        }
      }
    }

    when {
      !ready -> Text(stringResource(R.string.opening_library), Modifier.padding(top = 24.dp))
      list is ListId.Setlist && setlist == null -> Text(stringResource(R.string.setlist_gone), Modifier.padding(top = 24.dp))
      list is ListId.Setlist && editing && setlist != null ->
        EditSetlist(
          store = store,
          stem = list.stem,
          detail = setlist,
          onChanged = {
            detail = it
            onClearSelection()
          },
          onCopied = { onOpenList(ListId.Setlist(it), true) },
          onDeleted = onClosed,
        )
      list is ListId.Setlist && entries.none { it.song != null } -> {
        Column(
          Modifier.fillMaxWidth().padding(top = 48.dp),
          verticalArrangement = Arrangement.spacedBy(16.dp),
          horizontalAlignment = Alignment.CenterHorizontally,
        ) {
          Text(stringResource(R.string.gig_nothing_to_play), style = MaterialTheme.typography.titleMedium)
          OutlinedButton(onClick = { onEditingChange(true) }) { Text(stringResource(R.string.edit_setlist)) }
        }
      }
      list is ListId.AllSongs && entries.isEmpty() -> {
        Column(
          Modifier.fillMaxWidth().weight(1f),
          verticalArrangement = Arrangement.Center,
          horizontalAlignment = Alignment.CenterHorizontally,
        ) {
          Text(stringResource(R.string.songs_empty_title), style = MaterialTheme.typography.titleLarge)
          Spacer(Modifier.height(8.dp))
          Text(
            stringResource(R.string.songs_empty_body),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
          )
          Spacer(Modifier.height(24.dp))
          Button(onClick = onNewSong, modifier = Modifier.fillMaxWidth().height(56.dp)) { Text(stringResource(R.string.new_song)) }
        }
      }
      else -> {
        Column(Modifier.fillMaxWidth().padding(vertical = 10.dp), horizontalAlignment = Alignment.CenterHorizontally) {
          FlashBar(beatState)
          Spacer(Modifier.height(10.dp))
          BeatDots(beats, beatState)
        }
        var query by rememberSaveable { mutableStateOf("") }
        if (list is ListId.AllSongs) {
          OutlinedTextField(
            value = query,
            onValueChange = { query = it },
            placeholder = { Text(stringResource(R.string.search_songs)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp),
          )
        }
        val shown = entries.withIndex().filter { query.isBlank() || it.value.title.contains(query.trim(), ignoreCase = true) }
        // Opens with the song tapped last in view.
        val listState = rememberLazyListState(initialFirstVisibleItemIndex = (selected - 2).coerceAtLeast(0))
        LazyColumn(state = listState, modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
          items(shown, key = { it.index }) { (index, entry) ->
            SongRow(
              number = if (list is ListId.Setlist) "${index + 1}" else "",
              entry = entry,
              playing = index == playing,
              marked = index == selected,
              onTap = { tap(index) },
              onEdit = { entry.song?.let { onEditSong(index, it) } },
            )
          }
          if (shown.isEmpty()) {
            item { Text(stringResource(R.string.no_matches), Modifier.padding(top = 16.dp)) }
          }
        }
        if (list is ListId.AllSongs) {
          Spacer(Modifier.height(8.dp))
          OutlinedButton(onClick = onNewSong, modifier = Modifier.fillMaxWidth().height(48.dp)) {
            Text(stringResource(R.string.new_song))
          }
        }
      }
    }
  }
}

/**
 * One song of a list: its number, title and notes, the tempo and beats, and ✎. The song that is
 * playing is highlighted and shows a stop sign in place of its number; the one tapped last stays
 * lightly marked when the click is stopped. A missing song is greyed out and has no ✎.
 *
 * The title grows at most 1.3× with the phone's text size, and when the row is narrow for the
 * text size (a small phone, very large text) the tempo moves under the title, so a title is not
 * broken in the middle of a word.
 */
@Composable
private fun SongRow(
  number: String,
  entry: EntryRecord,
  playing: Boolean,
  marked: Boolean,
  onTap: () -> Unit,
  onEdit: () -> Unit,
) {
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
  BoxWithConstraints(Modifier.fillMaxWidth()) {
  val stacked = maxWidth / LocalDensity.current.fontScale < 260.dp
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
      .padding(start = 12.dp, top = 10.dp, bottom = 10.dp),
    verticalAlignment = Alignment.CenterVertically,
  ) {
    // Always there, so the title does not move when the row starts playing.
    Text(
      if (playing) "■" else number,
      style = MaterialTheme.typography.titleMedium,
      color = detailColor,
      modifier = Modifier.width(36.dp),
    )
    Column(Modifier.weight(1f)) {
      Text(entry.title, style = MaterialTheme.typography.titleLarge, fontSize = numeralSize(22), color = content)
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
      if (song != null && stacked) {
        Text(
          "${song.bpm.roundToInt()} BPM · " + pluralStringResource(R.plurals.beats_count, song.beats.toInt(), song.beats.toInt()),
          style = MaterialTheme.typography.bodyMedium,
          color = content,
        )
      }
    }
    if (song != null && !stacked) {
      Column(Modifier.padding(start = 12.dp), horizontalAlignment = Alignment.End) {
        Text(song.bpm.roundToInt().toString(), style = MaterialTheme.typography.headlineSmall, color = content)
        Text(
          pluralStringResource(R.plurals.beats_count, song.beats.toInt(), song.beats.toInt()),
          style = MaterialTheme.typography.bodySmall,
          color = detailColor,
        )
      }
    }
    if (song != null) {
      val editLabel = stringResource(R.string.edit_song_named, entry.title)
      IconButton(onClick = onEdit, modifier = Modifier.semantics { contentDescription = editLabel }) {
        Icon(Icons.Filled.Edit, contentDescription = null, tint = detailColor)
      }
    } else {
      Spacer(Modifier.width(48.dp))
    }
  }
  }
}

/**
 * A setlist in Edit mode: its band with "Name and band", the songs with move up, move down and
 * remove, "Add songs", and Copy and Delete. Every change is saved at once, one at a time, so a
 * second tap meant for the old order cannot land on the new one.
 */
@Composable
private fun EditSetlist(
  store: LibraryStore,
  stem: String,
  detail: SetlistDetail,
  onChanged: (SetlistDetail) -> Unit,
  onCopied: (stem: String) -> Unit,
  onDeleted: () -> Unit,
) {
  var busy by remember { mutableStateOf(false) }
  var problem by remember { mutableStateOf<String?>(null) }
  var showDetails by remember { mutableStateOf(false) }
  var showAdd by remember { mutableStateOf(false) }
  var confirmDelete by remember { mutableStateOf(false) }
  val scope = rememberCoroutineScope()

  fun act(block: (SongLibrary) -> Unit) {
    if (busy) return
    busy = true
    problem = null
    scope.launch {
      when (val outcome = store.change { block(it) }) {
        is Outcome.Ok -> store.read { it.setlist(stem) }?.let(onChanged)
        is Outcome.Failed -> problem = outcome.message
      }
      busy = false
    }
  }

  Column(Modifier.fillMaxSize()) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
      Text(
        detail.band ?: stringResource(R.string.no_band),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.weight(1f),
      )
      TextButton(onClick = { showDetails = true }) { Text(stringResource(R.string.name_and_band)) }
    }
    problem?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(vertical = 4.dp)) }
    HorizontalDivider()

    if (detail.entries.isEmpty()) {
      Text(
        stringResource(R.string.setlist_empty_songs),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.weight(1f).padding(top = 24.dp),
      )
    } else {
      LazyColumn(Modifier.weight(1f)) {
        itemsIndexed(detail.entries) { index, entry ->
          EntryRow(
            index = index,
            entry = entry,
            last = index == detail.entries.lastIndex,
            onUp = { act { it.moveSongInSetlist(stem, index.toUInt(), (index - 1).toUInt()) } },
            onDown = { act { it.moveSongInSetlist(stem, index.toUInt(), (index + 1).toUInt()) } },
            onRemove = { act { it.removeSongFromSetlist(stem, index.toUInt()) } },
          )
          HorizontalDivider()
        }
      }
    }

    Spacer(Modifier.height(12.dp))
    Button(onClick = { showAdd = true }, modifier = Modifier.fillMaxWidth().height(56.dp)) {
      Text(stringResource(R.string.add_songs))
    }
    Row(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
      OutlinedButton(
        onClick = {
          if (busy) return@OutlinedButton
          busy = true
          scope.launch {
            when (val outcome = store.change { it.copySetlist(stem) }) {
              is Outcome.Ok -> onCopied(outcome.value)
              is Outcome.Failed -> problem = outcome.message
            }
            busy = false
          }
        },
        modifier = Modifier.weight(1f).height(48.dp),
      ) {
        Text(stringResource(R.string.copy))
      }
      OutlinedButton(onClick = { confirmDelete = true }, modifier = Modifier.weight(1f).height(48.dp)) {
        Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error)
      }
    }
  }

  if (showDetails) {
    SetlistDetailsDialog(
      title = stringResource(R.string.setlist_details_title),
      confirmLabel = stringResource(R.string.save),
      initialName = detail.name,
      initialBand = detail.band ?: "",
      bands = store.bands,
      onDismiss = { showDetails = false },
      onConfirm = { name, band ->
        val outcome =
          store.change {
            if (name != detail.name) it.renameSetlist(stem, name)
            if (band != detail.band) it.setSetlistBand(stem, band)
          }
        when (outcome) {
          is Outcome.Ok -> {
            store.read { it.setlist(stem) }?.let(onChanged)
            showDetails = false
            null
          }
          is Outcome.Failed -> outcome.message
        }
      },
    )
  }

  if (showAdd) {
    AddSongsDialog(store = store, entries = detail.entries, onAdd = { title -> act { it.addSongToSetlist(stem, title) } }) {
      showAdd = false
    }
  }

  if (confirmDelete) {
    AlertDialog(
      onDismissRequest = { confirmDelete = false },
      title = { Text(stringResource(R.string.delete_setlist_title, detail.name)) },
      text = { Text(stringResource(R.string.delete_setlist_body)) },
      confirmButton = {
        TextButton(
          onClick = {
            confirmDelete = false
            scope.launch {
              when (val outcome = store.change { it.deleteSetlist(stem) }) {
                is Outcome.Ok -> onDeleted()
                is Outcome.Failed -> problem = outcome.message
              }
            }
          }
        ) {
          Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error)
        }
      },
      dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text(stringResource(R.string.cancel)) } },
    )
  }
}

@Composable
private fun EntryRow(
  index: Int,
  entry: EntryRecord,
  last: Boolean,
  onUp: () -> Unit,
  onDown: () -> Unit,
  onRemove: () -> Unit,
) {
  Row(Modifier.fillMaxWidth().padding(vertical = 2.dp), verticalAlignment = Alignment.CenterVertically) {
    Text(
      "${index + 1}",
      style = MaterialTheme.typography.bodyMedium,
      color = MaterialTheme.colorScheme.onSurfaceVariant,
      modifier = Modifier.width(28.dp),
    )
    Column(Modifier.weight(1f)) {
      Text(entry.title, style = MaterialTheme.typography.bodyLarge)
      val song = entry.song
      if (song == null) {
        Text(
          stringResource(R.string.missing_song),
          style = MaterialTheme.typography.bodySmall,
          color = MaterialTheme.colorScheme.error,
        )
      } else {
        Text(
          "${song.bpm.roundToInt()} · ${song.beats}",
          style = MaterialTheme.typography.bodySmall,
          color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
      }
    }
    RowAction("↑", stringResource(R.string.move_up, entry.title), enabled = index > 0, onClick = onUp)
    RowAction("↓", stringResource(R.string.move_down, entry.title), enabled = !last, onClick = onDown)
    RowAction("✕", stringResource(R.string.remove_from_setlist, entry.title), enabled = true, onClick = onRemove)
  }
}

/** A square 48 dp button with one glyph, described for screen readers. */
@Composable
private fun RowAction(glyph: String, description: String, enabled: Boolean, onClick: () -> Unit) {
  TextButton(
    onClick = onClick,
    enabled = enabled,
    contentPadding = PaddingValues(0.dp),
    modifier = Modifier.size(48.dp).semantics { contentDescription = description },
  ) {
    Text(glyph, style = MaterialTheme.typography.titleMedium)
  }
}

/** Pick songs to add at the end; the dialog stays open so several can be added in a row. */
@Composable
private fun AddSongsDialog(
  store: LibraryStore,
  entries: List<EntryRecord>,
  onAdd: (title: String) -> Unit,
  onDone: () -> Unit,
) {
  var query by remember { mutableStateOf("") }
  val songs = store.songs
  val shown = remember(songs, query) { songs.filter { it.title.contains(query.trim(), ignoreCase = true) } }

  AlertDialog(
    onDismissRequest = onDone,
    title = { Text(stringResource(R.string.add_songs_title)) },
    text = {
      if (songs.isEmpty()) {
        Text(stringResource(R.string.add_songs_none))
      } else {
        Column {
          OutlinedTextField(
            value = query,
            onValueChange = { query = it },
            placeholder = { Text(stringResource(R.string.search_songs)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
          )
          Spacer(Modifier.height(8.dp))
          if (shown.isEmpty()) {
            Text(stringResource(R.string.no_matches))
          } else {
            LazyColumn(Modifier.heightIn(max = 360.dp)) {
              items(shown, key = { it.title }) { song ->
                val already = entries.count { it.title.equals(song.title, ignoreCase = true) }
                Row(
                  Modifier.fillMaxWidth().clickable { onAdd(song.title) }.padding(vertical = 12.dp),
                  verticalAlignment = Alignment.CenterVertically,
                ) {
                  Column(Modifier.weight(1f)) {
                    Text(song.title, style = MaterialTheme.typography.bodyLarge)
                    if (already > 0) {
                      Text(
                        stringResource(R.string.in_setlist_times, already),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.primary,
                      )
                    }
                  }
                  Text(
                    "${song.bpm.roundToInt()} · ${song.beats}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                  )
                }
                HorizontalDivider()
              }
            }
          }
        }
      }
    },
    confirmButton = { TextButton(onClick = onDone) { Text(stringResource(R.string.done)) } },
  )
}
