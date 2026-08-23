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
import uniffi.stellar.Core
import uniffi.stellar.CoreException
import uniffi.stellar.logError
import uniffi.stellar_graph.AttributeKind
import uniffi.stellar_graph.AttributeSchema
import uniffi.stellar_graph.RelationKind
import uniffi.stellar_graph.RelationSchema
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
        schema.graph.entities.entries.sortedBy { it.key }.forEach { (entityKind, entitySchema) ->
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
                    Attributes(attributes = entitySchema.attributes, deleteAttribute = { attributeKind ->
                        coroutineScope.launch {
                            try {
                                core.deleteSchemaEntityAttribute(entityKind, attributeKind)
                            } catch (e: CoreException) {
                                logError("$e")
                            }
                        }
                    })

                    schema.graph.relations.entries
                        .filter {
                            it.value.source == entityKind
                        }.sortedBy { it.key }
                        .forEach { (relationKind, relationSchema) ->
                            Relation(
                                core,
                                true,
                                entitySchema.name,
                                schema.graph.entities[relationSchema.target]?.name ?: "?",
                                relationKind,
                                relationSchema,
                            )
                        }

                    schema.graph.relations.entries
                        .filter {
                            it.value.target == entityKind
                        }.sortedBy { it.key }
                        .forEach { (relationKind, relationSchema) ->
                            Relation(
                                core,
                                false,
                                entitySchema.name,
                                schema.graph.entities[relationSchema.source]?.name ?: "?",
                                relationKind,
                                relationSchema,
                            )
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

        Button("reset schema", onClick = {
            coroutineScope.launch {
                try {
                    core.dangerouslyResetSchema()
                } catch (e: CoreException) {
                    logError("$e")
                }
            }
        })
    }
}

@Composable
internal fun Relation(
    core: Core,
    outgoing: Boolean,
    thisName: String,
    otherName: String,
    relationKind: RelationKind,
    relationSchema: RelationSchema,
) {
    val coroutineScope = rememberCoroutineScope()

    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        val icon =
            if (outgoing) {
                "->"
            } else {
                "<-"
            }

        Text("$thisName $icon $otherName: ${relationSchema.name}")

        Button("new attribute", onClick = {
            coroutineScope.launch {
                try {
                    core.createSchemaRelationAttribute(relationKind, "Attribute", ValueKind.TEXT)
                } catch (e: CoreException) {
                    logError("$e")
                }
            }
        })

        Button("delete relation", onClick = {
            coroutineScope.launch {
                try {
                    core.deleteSchemaRelation(relationKind)
                } catch (e: CoreException) {
                    logError("$e")
                }
            }
        })
    }

    Column(modifier = Modifier.padding(start = 16.dp)) {
        Attributes(attributes = relationSchema.attributes, deleteAttribute = { attributeKind ->
            coroutineScope.launch {
                try {
                    core.deleteSchemaRelationAttribute(relationKind, attributeKind)
                } catch (e: CoreException) {
                    logError("$e")
                }
            }
        })
    }
}

@Composable
internal fun Attributes(
    attributes: Map<AttributeKind, AttributeSchema>,
    deleteAttribute: (attributeKind: AttributeKind) -> Unit,
) {
    attributes.entries.sortedBy { it.key }.forEach { (attributeKind, attributeSchema) ->
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("${attributeSchema.name}: ${attributeSchema.value}")

            Button("delete attribute", onClick = {
                deleteAttribute(attributeKind)
            })
        }
    }
}
