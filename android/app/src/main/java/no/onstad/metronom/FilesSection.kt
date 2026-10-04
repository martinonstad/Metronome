package no.onstad.metronom

import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.time.LocalDate
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.metronom_ffi.ImportOptionsRecord
import uniffi.metronom_ffi.ImportReportRecord
import uniffi.metronom_ffi.SetlistConflict
import uniffi.metronom_ffi.SettingsConflict
import uniffi.metronom_ffi.SongConflict

/** The largest file the app will even read; the library itself allows 20 MB once unzipped. */
private const val MAX_ARCHIVE_BYTES = 25 * 1024 * 1024

/** A zip the user picked, read into memory and waiting for them to choose what to do with duplicates. */
private class PickedArchive(val name: String, val bytes: ByteArray)

/**
 * Export your library to a zip and import one. Export builds the zip first and only then asks
 * where to save it, so a failure never leaves an empty file behind. Import asks what to do with
 * songs, setlists and settings that already exist, then shows what was added, replaced, kept or
 * ignored.
 */
@Composable
internal fun FilesSection(store: LibraryStore, onMessage: (String) -> Unit) {
  val context = LocalContext.current
  val scope = rememberCoroutineScope()
  var busy by remember { mutableStateOf(false) }
  var pendingExport by remember { mutableStateOf<ByteArray?>(null) }
  var picked by remember { mutableStateOf<PickedArchive?>(null) }
  var report by remember { mutableStateOf<ImportReportRecord?>(null) }

  val saveLauncher =
    rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/zip")) { uri ->
      val bytes = pendingExport
      pendingExport = null
      if (uri != null && bytes != null) {
        scope.launch {
          busy = true
          try {
            writeTo(context, uri, bytes)
            onMessage(context.getString(R.string.files_exported))
          } catch (e: IOException) {
            onMessage(context.getString(R.string.files_write_failed, e.message ?: ""))
          }
          busy = false
        }
      }
    }

  val openLauncher =
    rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
      if (uri != null) {
        scope.launch {
          busy = true
          try {
            val bytes = readFrom(context, uri)
            if (bytes == null) {
              onMessage(context.getString(R.string.files_too_large))
            } else {
              picked = PickedArchive(displayName(context, uri), bytes)
            }
          } catch (e: IOException) {
            onMessage(context.getString(R.string.files_read_failed, e.message ?: ""))
          }
          busy = false
        }
      }
    }

  Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
    Text(stringResource(R.string.files_intro), style = MaterialTheme.typography.bodyMedium)
    OutlinedButton(
      onClick = {
        scope.launch {
          busy = true
          when (val outcome = store.change { it.exportArchive() }) {
            is Outcome.Ok -> {
              pendingExport = outcome.value
              saveLauncher.launch("Metronom-${LocalDate.now()}.zip")
            }
            is Outcome.Failed -> onMessage(outcome.message)
          }
          busy = false
        }
      },
      enabled = !busy,
      modifier = Modifier.fillMaxWidth().height(48.dp),
    ) {
      Text(stringResource(R.string.files_export))
    }
    OutlinedButton(
      onClick = {
        openLauncher.launch(arrayOf("application/zip", "application/x-zip-compressed", "application/octet-stream"))
      },
      enabled = !busy,
      modifier = Modifier.fillMaxWidth().height(48.dp),
    ) {
      Text(stringResource(R.string.files_import))
    }
  }

  picked?.let { archive ->
    ImportOptionsDialog(
      name = archive.name,
      // On a fresh install (settings never changed) the old phone's settings are what you want.
      settingsDefault = if (store.settings == DEFAULT_SETTINGS) SettingsConflict.OVERWRITE else SettingsConflict.SKIP,
      onDismiss = { picked = null },
      onImport = { options ->
        picked = null
        scope.launch {
          busy = true
          when (val outcome = store.change { it.importArchive(archive.bytes, options) }) {
            is Outcome.Ok -> report = outcome.value
            is Outcome.Failed -> onMessage(outcome.message)
          }
          busy = false
        }
      },
    )
  }

  report?.let { ImportReportDialog(it) { report = null } }
}

@Composable
private fun ImportOptionsDialog(
  name: String,
  settingsDefault: SettingsConflict,
  onDismiss: () -> Unit,
  onImport: (ImportOptionsRecord) -> Unit,
) {
  var songs by remember { mutableStateOf(SongConflict.SKIP_EXISTING) }
  var setlists by remember { mutableStateOf(SetlistConflict.KEEP_BOTH) }
  var settings by remember { mutableStateOf(settingsDefault) }

  AlertDialog(
    onDismissRequest = onDismiss,
    title = { Text(stringResource(R.string.import_title)) },
    text = {
      Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(stringResource(R.string.import_file, name), style = MaterialTheme.typography.bodyMedium)
        Text(stringResource(R.string.import_new_added), style = MaterialTheme.typography.bodySmall)
        Choice(
          R.string.import_songs,
          listOf(
            SongConflict.SKIP_EXISTING to R.string.import_keep_mine,
            SongConflict.OVERWRITE to R.string.import_replace,
          ),
          songs,
        ) { songs = it }
        Choice(
          R.string.import_setlists,
          listOf(
            SetlistConflict.KEEP_BOTH to R.string.import_keep_both,
            SetlistConflict.SKIP to R.string.import_keep_mine,
            SetlistConflict.OVERWRITE to R.string.import_replace,
          ),
          setlists,
        ) { setlists = it }
        Choice(
          R.string.import_settings,
          listOf(
            SettingsConflict.SKIP to R.string.import_keep_mine,
            SettingsConflict.OVERWRITE to R.string.import_replace,
          ),
          settings,
        ) { settings = it }
      }
    },
    confirmButton = {
      TextButton(onClick = { onImport(ImportOptionsRecord(settings, setlists, songs)) }) {
        Text(stringResource(R.string.files_import_action))
      }
    },
    dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
  )
}

/** A question with one radio button per answer. */
@Composable
private fun <T> Choice(title: Int, options: List<Pair<T, Int>>, selected: T, onSelect: (T) -> Unit) {
  Column {
    Text(stringResource(title), style = MaterialTheme.typography.titleSmall)
    for ((value, label) in options) {
      Row(
        Modifier.fillMaxWidth().clickable { onSelect(value) }.padding(vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
      ) {
        RadioButton(selected = selected == value, onClick = { onSelect(value) })
        Text(stringResource(label), style = MaterialTheme.typography.bodyMedium)
      }
    }
  }
}

@Composable
private fun ImportReportDialog(report: ImportReportRecord, onDismiss: () -> Unit) {
  AlertDialog(
    onDismissRequest = onDismiss,
    title = { Text(stringResource(R.string.import_done)) },
    text = {
      Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        for (line in reportLines(report)) Text(line, style = MaterialTheme.typography.bodyMedium)
        if (report.ignored.isNotEmpty()) {
          Text(stringResource(R.string.import_ignored), style = MaterialTheme.typography.titleSmall)
          for (entry in report.ignored.take(REPORT_DETAILS)) Text(entry, style = MaterialTheme.typography.bodySmall)
          if (report.ignored.size > REPORT_DETAILS) Text("…", style = MaterialTheme.typography.bodySmall)
        }
        if (report.warnings.isNotEmpty()) {
          Text(stringResource(R.string.warnings_title), style = MaterialTheme.typography.titleSmall)
          for (w in report.warnings.take(REPORT_DETAILS)) {
            val where = if (w.line != null) "${w.file}, line ${w.line}" else w.file
            Text("$where: ${w.detail}", style = MaterialTheme.typography.bodySmall)
          }
          if (report.warnings.size > REPORT_DETAILS) Text("…", style = MaterialTheme.typography.bodySmall)
        }
      }
    },
    confirmButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.done)) } },
  )
}

private const val REPORT_DETAILS = 8

/** What an import did, one sentence per kind of thing; kinds that did nothing are left out. */
internal fun reportLines(r: ImportReportRecord): List<String> = buildList {
  val songParts = buildList {
    if (r.songsAdded > 0u) add("${r.songsAdded} added")
    if (r.songsOverwritten > 0u) add("${r.songsOverwritten} replaced")
    if (r.songsSkipped > 0u) add("${r.songsSkipped} already there, yours kept")
  }
  if (songParts.isNotEmpty()) add("Songs: ${songParts.joinToString(", ")}.")

  val setlistParts = buildList {
    if (r.setlistsAdded.isNotEmpty()) add("${r.setlistsAdded.size} added")
    if (r.setlistsOverwritten.isNotEmpty()) add("${r.setlistsOverwritten.size} replaced")
    if (r.setlistsKeptBoth.isNotEmpty()) {
      add("${r.setlistsKeptBoth.size} added next to yours (" + r.setlistsKeptBoth.joinToString { "${it.from} → ${it.to}" } + ")")
    }
    if (r.setlistsSkipped.isNotEmpty()) add("${r.setlistsSkipped.size} already there, yours kept")
  }
  if (setlistParts.isNotEmpty()) add("Setlists: ${setlistParts.joinToString(", ")}.")

  add(if (r.settingsImported) "Settings: replaced with the imported ones." else "Settings: yours kept.")
  if (songParts.isEmpty() && setlistParts.isEmpty()) add("The file had no songs or setlists.")
}

/** Write [bytes] to the document the user chose. */
private suspend fun writeTo(context: android.content.Context, uri: Uri, bytes: ByteArray) =
  withContext(Dispatchers.IO) {
    val out = context.contentResolver.openOutputStream(uri, "w") ?: throw IOException("the file couldn't be opened")
    out.use { it.write(bytes) }
  }

/** The bytes of the document, or `null` when it is larger than [MAX_ARCHIVE_BYTES]. */
private suspend fun readFrom(context: android.content.Context, uri: Uri): ByteArray? =
  withContext(Dispatchers.IO) {
    val input = context.contentResolver.openInputStream(uri) ?: throw IOException("the file couldn't be opened")
    input.use {
      val out = ByteArrayOutputStream()
      val buffer = ByteArray(64 * 1024)
      while (true) {
        val n = it.read(buffer)
        if (n < 0) break
        out.write(buffer, 0, n)
        if (out.size() > MAX_ARCHIVE_BYTES) return@withContext null
      }
      out.toByteArray()
    }
  }

private fun displayName(context: android.content.Context, uri: Uri): String =
  try {
    context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
      if (cursor.moveToFirst()) cursor.getString(0) else null
    }
  } catch (e: SecurityException) {
    null
  } ?: uri.lastPathSegment ?: "the file"
