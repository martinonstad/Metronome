package no.onstad.metronom

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import uniffi.metronom_ffi.BandGroup
import uniffi.metronom_ffi.LibraryException
import uniffi.metronom_ffi.SetlistUseRecord
import uniffi.metronom_ffi.SongRecord

/**
 * The title, notes and setlists of [song], or of a new song when [song] is `null`. Opened from
 * the Metronome tab.
 *
 * Saving keeps the song's saved tempo and beats (those are saved with the Save button next to Tap
 * tempo); a new song takes [bpm] and [beats], what the metronome is set to. Ticked setlists get
 * the song added at the end. A title that is empty or already taken is refused and nothing is
 * written. [onSaved] gets the song's title; [onDeleted] is called after the song was deleted.
 */
@Composable
internal fun SongDetailsDialog(
  store: LibraryStore,
  song: SongRecord?,
  bpm: Double,
  beats: Int,
  onDismiss: () -> Unit,
  onSaved: (title: String) -> Unit,
  onDeleted: () -> Unit,
) {
  var title by rememberSaveable { mutableStateOf(song?.title ?: "") }
  var notes by rememberSaveable { mutableStateOf(song?.notes ?: "") }
  // The setlists (file names) the song is added to when it is saved.
  var picked by rememberSaveable { mutableStateOf(emptyList<String>()) }
  var titleError by remember { mutableStateOf<String?>(null) }
  var saving by remember { mutableStateOf(false) }
  var showSetlists by remember { mutableStateOf(false) }
  var savedTitle by remember { mutableStateOf<String?>(null) }
  var notAdded by remember { mutableStateOf<List<String>?>(null) }
  var deleteImpact by remember { mutableStateOf<List<SetlistUseRecord>?>(null) }
  var problem by remember { mutableStateOf<String?>(null) }
  val scope = rememberCoroutineScope()

  // Where the song is used already (an existing song only).
  var uses by remember { mutableStateOf<List<SetlistUseRecord>>(emptyList()) }
  LaunchedEffect(song?.title, store.revision) {
    uses = song?.let { s -> store.read { it.setlistsUsing(s.title) } } ?: emptyList()
  }

  val setlists = store.setlistGroups.flatMap { it.setlists }
  fun nameOf(stem: String) = setlists.firstOrNull { it.stem == stem }?.name ?: stem

  fun save() {
    if (saving) return
    if (title.isBlank()) {
      titleError = "Enter a title"
      return
    }
    saving = true
    val record =
      if (song == null) {
        SongRecord(title.trim(), bpm, beats.toUInt(), notes.trim())
      } else {
        SongRecord(title.trim(), song.bpm, song.beats, notes.trim())
      }
    val targets = picked.filter { stem -> setlists.any { it.stem == stem } }.associateWith(::nameOf)
    scope.launch {
      // One change: the song, then the setlists. If the song cannot be saved nothing is written;
      // a setlist that cannot be changed is reported and does not undo the song.
      val outcome =
        store.change { library ->
          if (song == null) library.addSong(record) else library.updateSong(song.title, record)
          targets.mapNotNull { (stem, name) ->
            try {
              library.addSongToSetlist(stem, record.title)
              null
            } catch (e: LibraryException) {
              "$name: ${describe(e)}"
            }
          }
        }
      when (outcome) {
        is Outcome.Failed -> {
          titleError = outcome.message
          saving = false
        }
        is Outcome.Ok -> {
          val saved = store.song(record.title)?.title ?: record.title
          if (outcome.value.isEmpty()) onSaved(saved) else {
            savedTitle = saved
            notAdded = outcome.value
          }
        }
      }
    }
  }

  AlertDialog(
    onDismissRequest = onDismiss,
    title = { Text(stringResource(if (song == null) R.string.new_song else R.string.song_details)) },
    text = {
      Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        OutlinedTextField(
          value = title,
          onValueChange = {
            title = it
            titleError = null
          },
          label = { Text(stringResource(R.string.title_label)) },
          singleLine = true,
          isError = titleError != null,
          supportingText = { titleError?.let { Text(it) } },
          modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
          value = notes,
          onValueChange = { notes = it.replace('\n', ' ') },
          label = { Text(stringResource(R.string.notes_label)) },
          placeholder = { Text(stringResource(R.string.notes_hint)) },
          minLines = 2,
          maxLines = 3,
          modifier = Modifier.fillMaxWidth(),
        )
        Column(Modifier.fillMaxWidth()) {
          Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.setlists), style = MaterialTheme.typography.labelLarge, modifier = Modifier.weight(1f))
            TextButton(onClick = { showSetlists = true }) { Text(stringResource(R.string.add_to_setlist)) }
          }
          if (uses.isNotEmpty()) {
            Text(
              stringResource(R.string.song_in_setlists, uses.joinToString { if (it.count > 1u) "${it.name} (×${it.count})" else it.name }),
              style = MaterialTheme.typography.bodyMedium,
              color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
          }
          if (picked.isNotEmpty()) {
            Text(
              stringResource(R.string.song_will_be_added, picked.joinToString(transform = ::nameOf)),
              style = MaterialTheme.typography.bodyMedium,
              color = MaterialTheme.colorScheme.primary,
            )
          }
        }
        problem?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        if (song != null) {
          TextButton(
            onClick = { scope.launch { deleteImpact = store.read { it.setlistsUsing(song.title) } ?: emptyList() } },
            modifier = Modifier.padding(top = 4.dp),
          ) {
            Text(stringResource(R.string.delete_song), color = MaterialTheme.colorScheme.error)
          }
        }
      }
    },
    confirmButton = { TextButton(onClick = { save() }, enabled = !saving) { Text(stringResource(R.string.save)) } },
    dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
  )

  if (showSetlists) {
    PickSetlistsDialog(
      groups = store.setlistGroups,
      picked = picked,
      uses = uses,
      onToggle = { stem -> picked = if (stem in picked) picked - stem else picked + stem },
      onDone = { showSetlists = false },
    )
  }

  // The song is saved by now, so the only way on is out of the dialog.
  notAdded?.let { lines ->
    val done = { onSaved(savedTitle ?: title.trim()) }
    AlertDialog(
      onDismissRequest = done,
      title = { Text(stringResource(R.string.song_saved)) },
      text = { Text(stringResource(R.string.song_not_added) + lines.joinToString("") { "\n• $it" }) },
      confirmButton = { TextButton(onClick = done) { Text(stringResource(R.string.ok)) } },
    )
  }

  val impact = deleteImpact
  if (impact != null && song != null) {
    AlertDialog(
      onDismissRequest = { deleteImpact = null },
      title = { Text(stringResource(R.string.delete_song_title, song.title)) },
      text = {
        if (impact.isEmpty()) {
          Text(stringResource(R.string.delete_song_unused))
        } else {
          Text(
            stringResource(R.string.delete_song_used) +
              impact.joinToString("") { "\n• ${it.name} (×${it.count})" }
          )
        }
      },
      confirmButton = {
        TextButton(
          onClick = {
            deleteImpact = null
            scope.launch {
              when (val outcome = store.change { it.deleteSong(song.title) }) {
                is Outcome.Ok -> onDeleted()
                is Outcome.Failed -> problem = outcome.message
              }
            }
          }
        ) {
          Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error)
        }
      },
      dismissButton = { TextButton(onClick = { deleteImpact = null }) { Text(stringResource(R.string.cancel)) } },
    )
  }
}

/**
 * Tick the setlists the song is added to (at the end of each) when it is saved. A setlist that
 * has the song already says so; adding it again puts it in twice.
 */
@Composable
private fun PickSetlistsDialog(
  groups: List<BandGroup>,
  picked: List<String>,
  uses: List<SetlistUseRecord>,
  onToggle: (stem: String) -> Unit,
  onDone: () -> Unit,
) {
  AlertDialog(
    onDismissRequest = onDone,
    title = { Text(stringResource(R.string.add_to_setlist)) },
    text = {
      if (groups.isEmpty()) {
        Text(stringResource(R.string.no_setlists_yet))
      } else {
        LazyColumn(Modifier.heightIn(max = 360.dp)) {
          for (group in groups) {
            item(key = "band:${group.band}") {
              Text(
                group.band ?: stringResource(R.string.no_band),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(top = 12.dp, bottom = 4.dp),
              )
            }
            items(group.setlists, key = { it.stem }) { setlist ->
              val checked = setlist.stem in picked
              val already = uses.firstOrNull { it.stem == setlist.stem }?.count?.toInt() ?: 0
              Row(
                Modifier.fillMaxWidth()
                  .toggleable(value = checked, role = Role.Checkbox, onValueChange = { onToggle(setlist.stem) })
                  .heightIn(min = 48.dp)
                  .padding(vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
              ) {
                Checkbox(checked = checked, onCheckedChange = null)
                Spacer(Modifier.width(12.dp))
                Column {
                  Text(setlist.name, style = MaterialTheme.typography.bodyLarge)
                  if (already > 0) {
                    Text(
                      stringResource(R.string.in_setlist_times, already),
                      style = MaterialTheme.typography.bodySmall,
                      color = MaterialTheme.colorScheme.primary,
                    )
                  }
                }
              }
            }
          }
        }
      }
    },
    confirmButton = { TextButton(onClick = onDone) { Text(stringResource(R.string.done)) } },
  )
}
