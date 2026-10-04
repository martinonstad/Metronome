package no.onstad.metronom

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import uniffi.metronom_ffi.SongRecord

/** Every song in one list: search, tap a song to edit it, add a new one. */
@Composable
fun SongsScreen(store: LibraryStore, onBack: () -> Unit, onOpen: (title: String?) -> Unit) {
  var query by rememberSaveable { mutableStateOf("") }
  var showWarnings by remember { mutableStateOf(false) }
  val songs = store.songs
  val shown = remember(songs, query) { songs.filter { it.title.contains(query.trim(), ignoreCase = true) } }

  Column(Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = 24.dp, vertical = 12.dp)) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
      TextButton(onClick = onBack) { Text("‹ ${stringResource(R.string.back)}") }
      Spacer(Modifier.weight(1f))
      Text(
        pluralStringResource(R.plurals.songs_count, songs.size, songs.size),
        style = MaterialTheme.typography.titleMedium,
      )
    }

    when (val state = store.state) {
      is LibraryStore.State.Opening -> Text(stringResource(R.string.opening_library), Modifier.padding(top = 24.dp))
      is LibraryStore.State.Failed ->
        Text(state.message, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(top = 24.dp))
      is LibraryStore.State.Ready -> {
        if (store.warnings.isNotEmpty()) {
          TextButton(onClick = { showWarnings = true }, modifier = Modifier.fillMaxWidth()) {
            Text(
              pluralStringResource(R.plurals.warnings_banner, store.warnings.size, store.warnings.size),
              color = MaterialTheme.colorScheme.tertiary,
            )
          }
        }
        if (songs.isEmpty()) {
          EmptySongs(onAdd = { onOpen(null) }, modifier = Modifier.weight(1f))
        } else {
          OutlinedTextField(
            value = query,
            onValueChange = { query = it },
            placeholder = { Text(stringResource(R.string.search_songs)) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
          )
          Spacer(Modifier.height(8.dp))
          if (shown.isEmpty()) {
            Text(stringResource(R.string.no_matches), Modifier.padding(top = 16.dp))
            Spacer(Modifier.weight(1f))
          } else {
            LazyColumn(Modifier.weight(1f)) {
              items(shown, key = { it.title }) { song ->
                SongRow(song) { onOpen(song.title) }
                HorizontalDivider()
              }
            }
          }
          Spacer(Modifier.height(12.dp))
          Button(onClick = { onOpen(null) }, modifier = Modifier.fillMaxWidth().height(56.dp)) {
            Text(stringResource(R.string.add_song))
          }
        }
      }
    }
  }

  if (showWarnings) {
    AlertDialog(
      onDismissRequest = { showWarnings = false },
      title = { Text(stringResource(R.string.warnings_title)) },
      text = {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
          for (w in store.warnings.take(12)) {
            val where = if (w.line != null) "${w.file}, line ${w.line}" else w.file
            Text("$where: ${w.detail}", style = MaterialTheme.typography.bodySmall)
          }
          if (store.warnings.size > 12) Text("…", style = MaterialTheme.typography.bodySmall)
        }
      },
      confirmButton = { TextButton(onClick = { showWarnings = false }) { Text(stringResource(R.string.close)) } },
    )
  }
}

@Composable
private fun SongRow(song: SongRecord, onClick: () -> Unit) {
  Row(
    Modifier.fillMaxWidth().clickable(onClick = onClick).padding(vertical = 16.dp),
    verticalAlignment = Alignment.CenterVertically,
  ) {
    Text(song.title, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
    Text(
      "${song.bpm.roundToInt()} · ${song.beats}",
      style = MaterialTheme.typography.bodyMedium,
      color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
  }
}

@Composable
private fun EmptySongs(onAdd: () -> Unit, modifier: Modifier = Modifier) {
  Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
    Text(stringResource(R.string.songs_empty_title), style = MaterialTheme.typography.titleLarge)
    Spacer(Modifier.height(8.dp))
    Text(stringResource(R.string.songs_empty_body), color = MaterialTheme.colorScheme.onSurfaceVariant)
    Spacer(Modifier.height(24.dp))
    Button(onClick = onAdd, modifier = Modifier.fillMaxWidth().height(56.dp)) { Text(stringResource(R.string.add_song)) }
  }
}
