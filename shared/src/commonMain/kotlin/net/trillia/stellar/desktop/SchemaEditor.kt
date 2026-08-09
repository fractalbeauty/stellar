package net.trillia.stellar.desktop

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import uniffi.stellar.Core
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
        modifier = Modifier.fillMaxSize().padding(16.dp).verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        schema.entities.map { (entityKind, entitySchema) ->
            Column {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(entitySchema.name)

                    Button("new attribute", onClick = {
                        coroutineScope.launch {
                            core.createSchemaEntityAttribute(entityKind, "Attribute", ValueKind.TEXT)
                        }
                    })

                    Button("delete entity", onClick = {
                        coroutineScope.launch {
                            core.deleteSchemaEntity(entityKind)
                        }
                    })
                }

                Column(modifier = Modifier.padding(start = 8.dp)) {
                    entitySchema.attributes.map { (attributeKind, attributeSchema) ->
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Text("${attributeSchema.name}: ${attributeSchema.value}")

                            Button("delete attribute", onClick = {
                                coroutineScope.launch {
                                    core.deleteSchemaEntityAttribute(entityKind, attributeKind)
                                }
                            })
                        }
                    }
                }
            }
        }

        Button("new entity", onClick = {
            coroutineScope.launch {
                core.createSchemaEntity("Entity")
            }
        })
    }
}
