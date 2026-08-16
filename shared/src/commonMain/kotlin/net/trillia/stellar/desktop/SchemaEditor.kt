package net.trillia.stellar.desktop

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import net.trillia.stellar.Button
import net.trillia.stellar.SchemaManager
import net.trillia.stellar.comparable
import uniffi.stellar.Core
import uniffi.stellar.CoreException
import uniffi.stellar.logError
import uniffi.stellar_graph.ValueKind

@Composable
fun SchemaEditor(
    core: Core,
    schemaManager: SchemaManager,
) {
    val coroutineScope = rememberCoroutineScope()

    val maybeSchema by schemaManager.schemaState.collectAsState()
    val schema = maybeSchema ?: return

    Column(
        modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        schema.entities.entries.sortedBy { it.key.comparable() }.forEach { (entityKind, entitySchema) ->
            Column {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(entitySchema.name)

                    Button("new attribute", onClick = {
                        coroutineScope.launch {
                            try {
                                core.createSchemaEntityAttribute(entityKind, "Attribute", ValueKind.TEXT)
                            } catch (e: CoreException) {
                                logError("$e")
                            }
                        }
                    })

                    Button("delete entity", onClick = {
                        coroutineScope.launch {
                            try {
                                core.deleteSchemaEntity(entityKind)
                            } catch (e: CoreException) {
                                logError("$e")
                            }
                        }
                    })
                }

                Column(modifier = Modifier.padding(start = 8.dp)) {
                    entitySchema.attributes.map { (attributeKind, attributeSchema) ->
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Text("${attributeSchema.name}: ${attributeSchema.value}")

                            Button("delete attribute", onClick = {
                                coroutineScope.launch {
                                    try {
                                        core.deleteSchemaEntityAttribute(entityKind, attributeKind)
                                    } catch (e: CoreException) {
                                        logError("$e")
                                    }
                                }
                            })
                        }
                    }
                }
            }
        }

        Button("new entity", onClick = {
            coroutineScope.launch {
                try {
                    core.createSchemaEntity("Entity")
                } catch (e: CoreException) {
                    logError("$e")
                }
            }
        })
    }
}
