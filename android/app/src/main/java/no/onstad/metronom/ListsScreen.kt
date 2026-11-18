package no.onstad.metronom

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import uniffi.metronom_ffi.SetlistSummary

/**
 * The first page of the Lists tab: "All songs" at the top, then the setlists grouped by band.
 * Tapping a row opens the list; a setlist is renamed, copied or deleted in its own Edit mode.
 * "New setlist" creates one and opens it in Edit mode.
 */
@Composable
fun ListsScreen(store: LibraryStore, onOpen: (ListId) -> Unit, onCreated: (stem: String) -> Unit) {
  var showNew by remember { mutableStateOf(false) }
  var showWarnings by remember { mutableStateOf(false) }
  val groups = store.setlistGroups
  val total = groups.sumOf { it.setlists.size }

  Column(Modifier.fillMaxSize().padding(horizontal = 24.dp, vertical = 12.dp)) {
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
        LazyColumn(Modifier.weight(1f)) {
          item(key = "all") {
            ListRow(
              name = stringResource(R.string.all_songs),
              detail = pluralStringResource(R.plurals.songs_count, store.songs.size, store.songs.size),
              missing = 0,
              onOpen = { onOpen(ListId.AllSongs) },
            )
            HorizontalDivider()
          }
          if (total == 0) {
            item(key = "empty") {
              Column(Modifier.fillMaxWidth().padding(vertical = 24.dp)) {
                Text(stringResource(R.string.setlists_empty_title), style = MaterialTheme.typography.titleMedium)
                Spacer(Modifier.height(8.dp))
                Text(stringResource(R.string.setlists_empty_body), color = MaterialTheme.colorScheme.onSurfaceVariant)
              }
            }
          }
          for (group in groups) {
            item(key = "band:${group.band}") {
              Text(
                group.band ?: stringResource(R.string.no_band),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(top = 16.dp, bottom = 4.dp),
              )
            }
            items(group.setlists, key = { it.stem }) { setlist: SetlistSummary ->
              ListRow(
                name = setlist.name,
                detail = pluralStringResource(R.plurals.songs_count, setlist.songCount.toInt(), setlist.songCount.toInt()),
                missing = setlist.missingCount.toInt(),
                onOpen = { onOpen(ListId.Setlist(setlist.stem)) },
              )
              HorizontalDivider()
            }
          }
        }
        Spacer(Modifier.height(12.dp))
        Button(onClick = { showNew = true }, modifier = Modifier.fillMaxWidth().height(56.dp)) {
          Text(stringResource(R.string.new_setlist))
        }
      }
    }
  }

  if (showNew) {
    SetlistDetailsDialog(
      title = stringResource(R.string.new_setlist),
      confirmLabel = stringResource(R.string.create),
      initialName = "",
      initialBand = "",
      bands = store.bands,
      onDismiss = { showNew = false },
      onConfirm = { name, band ->
        when (val outcome = store.change { it.createSetlist(name, band) }) {
          is Outcome.Ok -> {
            showNew = false
            onCreated(outcome.value)
            null
          }
          is Outcome.Failed -> outcome.message
        }
      },
    )
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

/** One list in the overview: its name, how many songs, and how many are missing. */
@Composable
private fun ListRow(name: String, detail: String, missing: Int, onOpen: () -> Unit) {
  Row(
    Modifier.fillMaxWidth().clickable(role = Role.Button, onClick = onOpen).heightIn(min = 56.dp).padding(vertical = 10.dp),
    verticalAlignment = Alignment.CenterVertically,
  ) {
    Text(name, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
    Column(horizontalAlignment = Alignment.End) {
      Text(detail, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
      if (missing > 0) {
        Text(
          stringResource(R.string.songs_missing, missing),
          style = MaterialTheme.typography.bodySmall,
          color = MaterialTheme.colorScheme.error,
        )
      }
    }
  }
}

/**
 * Name and band of a setlist, for creating one and for editing one. The existing bands are
 * offered as quick choices. [onConfirm] returns a message to show, or `null` when it worked.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun SetlistDetailsDialog(
  title: String,
  confirmLabel: String,
  initialName: String,
  initialBand: String,
  bands: List<String>,
  onDismiss: () -> Unit,
  onConfirm: suspend (name: String, band: String?) -> String?,
) {
  var name by remember { mutableStateOf(initialName) }
  var band by remember { mutableStateOf(initialBand) }
  var error by remember { mutableStateOf<String?>(null) }
  val scope = rememberCoroutineScope()

  val confirm = {
    if (name.isBlank()) {
      error = "Enter a name."
    } else {
      scope.launch { error = onConfirm(name.trim(), band.trim().ifEmpty { null }) }
      Unit
    }
  }

  AlertDialog(
    onDismissRequest = onDismiss,
    title = { Text(title) },
    text = {
      Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        OutlinedTextField(
          value = name,
          onValueChange = {
            name = it
            error = null
          },
          label = { Text(stringResource(R.string.setlist_name_label)) },
          singleLine = true,
          isError = error != null,
          supportingText = { error?.let { Text(it) } },
          modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
          value = band,
          onValueChange = { band = it },
          label = { Text(stringResource(R.string.setlist_band_label)) },
          singleLine = true,
          modifier = Modifier.fillMaxWidth(),
        )
        if (bands.isNotEmpty()) {
          FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for (option in bands) {
              FilterChip(
                selected = band.trim().equals(option, ignoreCase = true),
                onClick = { band = option },
                label = { Text(option) },
              )
            }
          }
        }
      }
    },
    confirmButton = { TextButton(onClick = { confirm() }) { Text(confirmLabel) } },
    dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
  )
}
