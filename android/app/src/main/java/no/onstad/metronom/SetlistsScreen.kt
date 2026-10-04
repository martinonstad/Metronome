package no.onstad.metronom

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
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
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import uniffi.metronom_ffi.SetlistSummary

/**
 * Every setlist, grouped by band. Tap one to play it; edit, copy and delete are on each row.
 */
@Composable
fun SetlistsScreen(
  store: LibraryStore,
  onBack: () -> Unit,
  onPlay: (stem: String) -> Unit,
  onEdit: (stem: String) -> Unit,
) {
  var showNew by remember { mutableStateOf(false) }
  var toDelete by remember { mutableStateOf<SetlistSummary?>(null) }
  var problem by remember { mutableStateOf<String?>(null) }
  val scope = rememberCoroutineScope()
  val groups = store.setlistGroups
  val total = groups.sumOf { it.setlists.size }

  Column(Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = 24.dp, vertical = 12.dp)) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
      TextButton(onClick = onBack) { Text("‹ ${stringResource(R.string.back)}") }
      Spacer(Modifier.weight(1f))
      Text(pluralStringResource(R.plurals.setlists_count, total, total), style = MaterialTheme.typography.titleMedium)
    }

    when (val state = store.state) {
      is LibraryStore.State.Opening -> Text(stringResource(R.string.opening_library), Modifier.padding(top = 24.dp))
      is LibraryStore.State.Failed ->
        Text(state.message, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(top = 24.dp))
      is LibraryStore.State.Ready -> {
        problem?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(vertical = 8.dp)) }
        if (total == 0) {
          EmptySetlists(modifier = Modifier.weight(1f))
        } else {
          LazyColumn(Modifier.weight(1f)) {
            for (group in groups) {
              item(key = "band:${group.band}") {
                Text(
                  group.band ?: stringResource(R.string.no_band),
                  style = MaterialTheme.typography.labelLarge,
                  color = MaterialTheme.colorScheme.primary,
                  modifier = Modifier.padding(top = 16.dp, bottom = 4.dp),
                )
              }
              items(group.setlists, key = { it.stem }) { setlist ->
                SetlistRow(
                  setlist,
                  onPlay = { onPlay(setlist.stem) },
                  onEdit = { onEdit(setlist.stem) },
                  onCopy = {
                    scope.launch {
                      problem = (store.change { it.copySetlist(setlist.stem) } as? Outcome.Failed)?.message
                    }
                  },
                  onDelete = { toDelete = setlist },
                )
                HorizontalDivider()
              }
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
            onEdit(outcome.value)
            null
          }
          is Outcome.Failed -> outcome.message
        }
      },
    )
  }

  toDelete?.let { setlist ->
    AlertDialog(
      onDismissRequest = { toDelete = null },
      title = { Text(stringResource(R.string.delete_setlist_title, setlist.name)) },
      text = { Text(stringResource(R.string.delete_setlist_body)) },
      confirmButton = {
        TextButton(
          onClick = {
            toDelete = null
            scope.launch {
              problem = (store.change { it.deleteSetlist(setlist.stem) } as? Outcome.Failed)?.message
            }
          }
        ) {
          Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error)
        }
      },
      dismissButton = { TextButton(onClick = { toDelete = null }) { Text(stringResource(R.string.cancel)) } },
    )
  }
}

@Composable
private fun SetlistRow(
  setlist: SetlistSummary,
  onPlay: () -> Unit,
  onEdit: () -> Unit,
  onCopy: () -> Unit,
  onDelete: () -> Unit,
) {
  val tight = PaddingValues(horizontal = 8.dp)
  Row(Modifier.fillMaxWidth().clickable(onClick = onPlay).padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
    Column(Modifier.weight(1f)) {
      Text(setlist.name, style = MaterialTheme.typography.bodyLarge)
      Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
          pluralStringResource(R.plurals.songs_count, setlist.songCount.toInt(), setlist.songCount.toInt()),
          style = MaterialTheme.typography.bodyMedium,
          color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (setlist.missingCount > 0u) {
          Text(
            stringResource(R.string.songs_missing, setlist.missingCount.toInt()),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.error,
          )
        }
      }
    }
    TextButton(onClick = onEdit, contentPadding = tight) { Text(stringResource(R.string.edit)) }
    TextButton(onClick = onCopy, contentPadding = tight) { Text(stringResource(R.string.copy)) }
    TextButton(onClick = onDelete, contentPadding = tight) {
      Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error)
    }
  }
}

@Composable
private fun EmptySetlists(modifier: Modifier = Modifier) {
  Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
    Text(stringResource(R.string.setlists_empty_title), style = MaterialTheme.typography.titleLarge)
    Spacer(Modifier.height(8.dp))
    Text(stringResource(R.string.setlists_empty_body), color = MaterialTheme.colorScheme.onSurfaceVariant)
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
