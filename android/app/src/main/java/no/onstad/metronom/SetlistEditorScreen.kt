package no.onstad.metronom

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import kotlinx.coroutines.launch
import uniffi.metronom_ffi.EntryRecord
import uniffi.metronom_ffi.SetlistDetail
import uniffi.metronom_ffi.SongLibrary

/**
 * One setlist: its name and band, and its songs in order. Every change is saved at once, so
 * there is no Save button. [stem] is the setlist's file name without `.md`.
 */
@Composable
fun SetlistEditorScreen(store: LibraryStore, stem: String, onBack: () -> Unit, onOpen: (stem: String) -> Unit) {
  var detail by remember { mutableStateOf<SetlistDetail?>(null) }
  var loaded by remember { mutableStateOf(false) }
  var busy by remember { mutableStateOf(false) }
  var problem by remember { mutableStateOf<String?>(null) }
  var showDetails by remember { mutableStateOf(false) }
  var showAdd by remember { mutableStateOf(false) }
  var confirmDelete by remember { mutableStateOf(false) }
  val scope = rememberCoroutineScope()

  // A setlist that is gone (deleted, or its file removed by hand) takes you back to the list.
  LaunchedEffect(stem, store.revision, store.state) {
    if (store.state is LibraryStore.State.Ready) {
      detail = store.read { it.setlist(stem) }
      loaded = true
    }
  }
  LaunchedEffect(loaded, detail) { if (loaded && detail == null) onBack() }

  /** One change at a time: a second tap meant for the old order must not land on the new one. */
  fun act(block: (SongLibrary) -> Unit) {
    if (busy) return
    busy = true
    problem = null
    scope.launch {
      when (val outcome = store.change { block(it) }) {
        is Outcome.Ok -> detail = store.read { it.setlist(stem) } ?: detail
        is Outcome.Failed -> problem = outcome.message
      }
      busy = false
    }
  }

  val current = detail
  Column(Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = 24.dp, vertical = 12.dp)) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
      TextButton(onClick = onBack) { Text("‹ ${stringResource(R.string.back)}") }
      Spacer(Modifier.weight(1f))
      if (current != null) {
        Text(
          pluralStringResource(R.plurals.songs_count, current.entries.size, current.entries.size),
          style = MaterialTheme.typography.titleMedium,
        )
      }
    }

    if (current == null) {
      Text(stringResource(R.string.opening_library), Modifier.padding(top = 24.dp))
      return@Column
    }

    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
      Column(Modifier.weight(1f)) {
        Text(current.name, style = MaterialTheme.typography.headlineSmall)
        Text(
          current.band ?: stringResource(R.string.no_band),
          style = MaterialTheme.typography.bodyMedium,
          color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
      }
      TextButton(onClick = { showDetails = true }) { Text(stringResource(R.string.edit)) }
    }
    problem?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(vertical = 4.dp)) }
    Spacer(Modifier.height(8.dp))
    HorizontalDivider()

    if (current.entries.isEmpty()) {
      Text(
        stringResource(R.string.setlist_empty_songs),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.weight(1f).padding(top = 24.dp),
      )
    } else {
      LazyColumn(Modifier.weight(1f)) {
        itemsIndexed(current.entries) { index, entry ->
          EntryRow(
            index = index,
            entry = entry,
            last = index == current.entries.lastIndex,
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
              is Outcome.Ok -> onOpen(outcome.value)
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

  if (showDetails && current != null) {
    SetlistDetailsDialog(
      title = stringResource(R.string.setlist_details_title),
      confirmLabel = stringResource(R.string.save),
      initialName = current.name,
      initialBand = current.band ?: "",
      bands = store.bands,
      onDismiss = { showDetails = false },
      onConfirm = { name, band ->
        val outcome =
          store.change {
            if (name != current.name) it.renameSetlist(stem, name)
            if (band != current.band) it.setSetlistBand(stem, band)
          }
        when (outcome) {
          is Outcome.Ok -> {
            showDetails = false
            null
          }
          is Outcome.Failed -> outcome.message
        }
      },
    )
  }

  if (showAdd && current != null) {
    AddSongsDialog(store = store, entries = current.entries, onAdd = { title -> act { it.addSongToSetlist(stem, title) } }) {
      showAdd = false
    }
  }

  if (confirmDelete && current != null) {
    AlertDialog(
      onDismissRequest = { confirmDelete = false },
      title = { Text(stringResource(R.string.delete_setlist_title, current.name)) },
      text = { Text(stringResource(R.string.delete_setlist_body)) },
      confirmButton = {
        TextButton(
          onClick = {
            confirmDelete = false
            scope.launch {
              when (val outcome = store.change { it.deleteSetlist(stem) }) {
                is Outcome.Ok -> onBack()
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
              itemsIndexed(shown, key = { _, song -> song.title }) { _, song ->
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
