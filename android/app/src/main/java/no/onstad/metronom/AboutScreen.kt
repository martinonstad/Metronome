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
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** One notice from `assets/licenses.txt`: a title and its text. */
private class LicenseBlock(val title: String, val text: String)

/** The file is blocks that start with a line `## title`; the text follows (see scripts/generate-licenses.py). */
private fun parseLicenses(file: String): List<LicenseBlock> =
  file.split(Regex("(?m)^## ")).drop(1).map { block ->
    val title = block.substringBefore('\n').trim()
    LicenseBlock(title, block.substringAfter('\n', "").trim())
  }

/**
 * Version, license and privacy statement of the app, and the notices of the libraries inside it
 * (their licenses ask for these to come with the app). A notice opens when you tap its title.
 */
@Composable
fun AboutScreen(onBack: () -> Unit) {
  val context = LocalContext.current
  val version = remember { context.packageManager.getPackageInfo(context.packageName, 0).versionName ?: "" }
  var blocks by remember { mutableStateOf<List<LicenseBlock>?>(null) }
  var open by rememberSaveable { mutableIntStateOf(-1) }
  LaunchedEffect(Unit) {
    blocks =
      withContext(Dispatchers.IO) {
        context.assets.open("licenses.txt").bufferedReader().use { parseLicenses(it.readText()) }
      }
  }

  Column(Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = 24.dp, vertical = 12.dp)) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
      TextButton(onClick = onBack) { Text("‹ ${stringResource(R.string.back)}") }
      Spacer(Modifier.weight(1f))
      Text(stringResource(R.string.about), style = MaterialTheme.typography.titleMedium)
    }
    LazyColumn(Modifier.weight(1f)) {
      item {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(vertical = 8.dp)) {
          Text(stringResource(R.string.app_name), style = MaterialTheme.typography.headlineMedium)
          Text(
            stringResource(R.string.about_version, version),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
          )
          Text(stringResource(R.string.about_license), style = MaterialTheme.typography.bodyMedium)
          Text(stringResource(R.string.about_privacy_title), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
          Text(stringResource(R.string.about_privacy), style = MaterialTheme.typography.bodyMedium)
          Text(stringResource(R.string.about_libraries), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
          Text(
            stringResource(R.string.about_libraries_hint),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
          )
        }
        HorizontalDivider()
      }
      itemsIndexed(blocks.orEmpty()) { index, block ->
        Column(
          Modifier.fillMaxWidth().clickable(role = Role.Button) { open = if (open == index) -1 else index }.padding(vertical = 12.dp)
        ) {
          Text(block.title, style = MaterialTheme.typography.bodyMedium)
          if (open == index) {
            Spacer(Modifier.height(8.dp))
            Text(block.text, fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
          }
        }
        HorizontalDivider()
      }
    }
  }
}
