package net.trillia.stellar.desktop

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import net.trillia.stellar.Button
import uniffi.stellar.Core
import uniffi.stellar.CoreException
import uniffi.stellar.logError
import uniffi.stellar.pickFolders
import uniffi.stellar_import.ImportEventHandler
import uniffi.stellar_import.ImportEventScannedFile

@Composable
fun Importer(core: Core) {
    val coroutineScope = rememberCoroutineScope()

    val files = remember { mutableStateMapOf<String, Map<String, String>?>() }

    LazyColumn(
        modifier = Modifier.fillMaxSize().padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button("start import", onClick = {
                    coroutineScope.launch {
                        try {
                            val roots = pickFolders()

                            core.startImport(
                                roots,
                                object : ImportEventHandler {
                                    override fun onPendingFile(path: String) {
                                        if (!files.containsKey(path)) {
                                            files[path] = null
                                        }
                                    }

                                    override fun onScannedFile(file: ImportEventScannedFile) {
                                        files[file.path] = file.tags
                                    }

                                    override fun onScanFinished() {
                                    }
                                },
                            )
                        } catch (e: CoreException) {
                            logError("$e")
                        }
                    }
                })
            }
        }

        files.entries.sortedBy { it.key }.forEach { (file, tags) ->
            item {
                Column {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(file)

                        if (tags == null) {
                            Text("[...]")
                        }
                    }

                    Column(modifier = Modifier.padding(start = 8.dp)) {
                        tags?.map { (tag, value) ->
                            Text("$tag: $value")
                        }
                    }
                }
            }
        }
    }
}
