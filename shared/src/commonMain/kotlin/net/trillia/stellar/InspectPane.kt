package net.trillia.stellar

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import net.trillia.stellar.desktop.table.Table
import net.trillia.stellar.desktop.table.TableCellText
import net.trillia.stellar.desktop.table.TableColumnDefinition
import uniffi.stellar.Core
import uniffi.stellar.logDebug
import uniffi.stellar_graph.EntityKind
import uniffi.stellar_graph.EntitySchema
import uniffi.stellar_graph.SlotValue
import uniffi.stellar_graph.TableQuery
import uniffi.stellar_graph.Value
import uniffi.stellar_sync.Schema
import kotlin.collections.component1
import kotlin.collections.component2
import kotlin.time.measureTimedValue

@Composable
fun InspectPane(
    core: Core,
    schemaManager: SchemaManager,
    selectedEntity: EntityKind,
) {
//    val entities =
//        remember {
//            try {
//                core.getEntities()
//            } catch (e: CoreException) {
//                logCoreError(e)
//                throw e
//            }
//        }
    val schemaNullable by schemaManager.schemaState.collectAsState()

    val schema = schemaNullable ?: return

//    var selectedEntity by remember { mutableStateOf(schema.importRules.songEntity) }

    val selectedEntitySchema = schema.graph.entities[selectedEntity] ?: return

//    val selectedEntityColumns =
//        remember(selectedEntitySchema) {
//            selectedEntitySchema.attributes.map { it ->
//                TableColumnDefinition<Map<String, String>, String>(
//                    id = it.value.name,
//                    header = it.value.name,
//                    initialWidth = 300.dp,
//                    accessor = { row -> row[it.value.name].orEmpty() },
//                    renderer = { TableCellText(it) },
//                )
//            }
//        }
//
//    val selectedEntityEntities = entities.filterValues { it.kind == selectedEntity }
//    val selectedEntityData =
//        selectedEntityEntities
//            .map {
//                it.value.attributes.entries.associate { attributeEntry ->
//                    val name = selectedEntitySchema.attributes[attributeEntry.key]?.name ?: attributeEntry.key.toString()
//                    val value =
//                        when (val attributeValue = attributeEntry.value.value) {
//                            is Value.Bytes -> "<bytes>"
//                            is Value.Number -> attributeValue.v1.toString()
//                            is Value.Text -> attributeValue.v1
//                            is Value.Bool -> attributeValue.v1.toString()
//                        }
//                    name to value
//                }
//            }

    Column {
//        Row {
//            schema.graph.entities.forEach {
//                Button(it.value.name, onClick = { selectedEntity = it.key })
//            }
//        }

        EntityTable(schema = schema, entityKind = selectedEntity, runTableQuery = { core.tableQuery(it) })

//        selected?.let { idx -> data.getOrNull(idx)?.let { Inspector(it) } }
    }
}

@Composable
fun EntityTable(
    schema: Schema,
    entityKind: EntityKind,
    runTableQuery: (TableQuery) -> List<List<SlotValue?>>,
) {
    val entitySchema = schema.graph.entities[entityKind] ?: return

    val (query, columns) =
        remember(entitySchema) {
            buildEntityTableQuery(schema, entityKind, entitySchema)
        }

    val data =
        remember(query) {
            val (data, elapsed) = measureTimedValue { runTableQuery(query) }
            logDebug("TableQuery returned ${data.size} rows in $elapsed (${elapsed / data.size} per row)")
            data
        }

    var selected by remember(entityKind) { mutableStateOf<Int?>(null) }

    Table(data, columns, selected, { selected = it })
}

fun buildEntityTableQuery(
    schema: Schema,
    entityKind: EntityKind,
    entitySchema: EntitySchema,
): Pair<TableQuery, List<TableColumnDefinition<List<SlotValue?>, *>>> {
    var nextOutputIndexInner = 0
    val nextOutputIndex = {
        val outputIndex = nextOutputIndexInner
        nextOutputIndexInner += 1
        outputIndex
    }

    val columns = mutableListOf<TableColumnDefinition<List<SlotValue?>, *>>()

    val nextColumnId = {
        columns.size.toString()
    }

    // Add queries/columns for all attributes
    val attributes =
        entitySchema.attributes.entries.associate { (attribute, schema) ->
            val outputIndex = nextOutputIndex()
            columns.add(
                TableColumnDefinition<List<SlotValue?>, String>(
                    id = nextColumnId(),
                    header = schema.name,
                    initialWidth = 200.dp,
                    accessor = { row -> formatSlotValue(row.getOrNull(outputIndex)) },
                    renderer = { TableCellText(it) },
                ),
            )
            attribute to outputIndex.toUShort()
        }

    // Add queries/columns for all relations except with AudioResource
    val outgoingRelations =
        schema.graph.relations.filterValues { schema ->
            schema.source == entityKind &&
                schema.target != EntityKind.AudioResource
        }
    val incomingRelations =
        schema.graph.relations.filterValues { schema ->
            schema.target == entityKind &&
                schema.source != EntityKind.AudioResource
        }

    val outgoingRelationAttributes =
        outgoingRelations.entries
            .associate { (relation, relationSchema) ->
                relation to
                    relationSchema.attributes.entries.associate { (attribute, attributeSchema) ->
                        val outputIndex = nextOutputIndex()
                        columns.add(
                            TableColumnDefinition<List<SlotValue?>, String>(
                                id = nextColumnId(),
                                header = "${relationSchema.name}.${attributeSchema.name}",
                                initialWidth = 200.dp,
                                accessor = { row -> formatSlotValue(row.getOrNull(outputIndex)) },
                                renderer = { TableCellText(it) },
                            ),
                        )
                        attribute to outputIndex.toUShort()
                    }
            }.toMutableMap()

    val outgoingRelationEntityAttributes =
        outgoingRelations.entries
            .associate { (relation, relationSchema) ->
                val otherSchema = schema.graph.entities[relationSchema.target] ?: return@associate relation to emptyMap()
                relation to
                    otherSchema.attributes.entries.associate { (attribute, attributeSchema) ->
                        val outputIndex = nextOutputIndex()
                        columns.add(
                            TableColumnDefinition<List<SlotValue?>, String>(
                                id = nextColumnId(),
                                header = "${otherSchema.name}.${attributeSchema.name}",
                                initialWidth = 200.dp,
                                accessor = { row -> formatSlotValue(row.getOrNull(outputIndex)) },
                                renderer = { TableCellText(it) },
                            ),
                        )
                        attribute to outputIndex.toUShort()
                    }
            }.toMutableMap()

    val incomingRelationAttributes =
        incomingRelations.entries
            .associate { (relation, relationSchema) ->
                relation to
                    relationSchema.attributes.entries.associate { (attribute, attributeSchema) ->
                        val outputIndex = nextOutputIndex()
                        columns.add(
                            TableColumnDefinition<List<SlotValue?>, String>(
                                id = nextColumnId(),
                                header = "${relationSchema.name}.${attributeSchema.name}",
                                initialWidth = 200.dp,
                                accessor = { row -> formatSlotValue(row.getOrNull(outputIndex)) },
                                renderer = { TableCellText(it) },
                            ),
                        )
                        attribute to outputIndex.toUShort()
                    }
            }.toMutableMap()
    val incomingRelationEntityAttributes =
        incomingRelations.entries
            .associate { (relation, relationSchema) ->
                val otherSchema = schema.graph.entities[relationSchema.source] ?: return@associate relation to emptyMap()
                relation to
                    otherSchema.attributes.entries.associate { (attribute, attributeSchema) ->
                        val outputIndex = nextOutputIndex()
                        columns.add(
                            TableColumnDefinition<List<SlotValue?>, String>(
                                id = nextColumnId(),
                                header = "${otherSchema.name}.${attributeSchema.name}",
                                initialWidth = 200.dp,
                                accessor = { row -> formatSlotValue(row.getOrNull(outputIndex)) },
                                renderer = { TableCellText(it) },
                            ),
                        )
                        attribute to outputIndex.toUShort()
                    }
            }.toMutableMap()

    // Add queries/column for AudioResource relation
    val audioResourceRelation =
        schema.graph.relations.entries
            .find {
                (it.value.source == entityKind && it.value.target == EntityKind.AudioResource) ||
                    (it.value.source == EntityKind.AudioResource && it.value.target == entityKind)
            }
    if (audioResourceRelation != null) {
        val locationOutput = nextOutputIndex()
        val hashOutput = nextOutputIndex()
        val sizeOutput = nextOutputIndex()
        val qualityOutput = nextOutputIndex()
        val durationOutput = nextOutputIndex()

        val audioResourceAttributes =
            mapOf(
                AttributeKind.AudioResourceLocation to locationOutput.toUShort(),
                AttributeKind.AudioResourceHash to hashOutput.toUShort(),
                AttributeKind.AudioResourceSize to sizeOutput.toUShort(),
                AttributeKind.AudioResourceQuality to qualityOutput.toUShort(),
                AttributeKind.AudioResourceDuration to durationOutput.toUShort(),
            )

        columns.add(
            TableColumnDefinition<List<SlotValue?>, String>(
                id = nextColumnId(),
                header = "Audio Resource",
                initialWidth = 200.dp,
                accessor = { row ->
                    val location = row.getOrNull(locationOutput)
                    val hash = row.getOrNull(hashOutput)
                    val size = row.getOrNull(sizeOutput)
                    val quality = row.getOrNull(qualityOutput)
                    val duration = row.getOrNull(durationOutput)

                    "location=${formatSlotValue(
                        location,
                    )} hash=${formatSlotValue(
                        hash,
                    )} size=${formatSlotValue(size)} quality=${formatSlotValue(quality)} duration=${formatSlotValue(duration)}"
                },
                renderer = { TableCellText(it) },
            ),
        )

        if (audioResourceRelation.value.target == EntityKind.AudioResource) {
            outgoingRelationEntityAttributes[audioResourceRelation.key] = audioResourceAttributes
        } else {
            incomingRelationEntityAttributes[audioResourceRelation.key] = audioResourceAttributes
        }
    }

    val query =
        TableQuery(
            entity = entityKind,
            id = null,
            attributes = attributes,
            outgoingRelationAttributes = outgoingRelationAttributes,
            outgoingRelationEntityAttributes = outgoingRelationEntityAttributes,
            outgoingRelationOthers = emptyMap(),
            incomingRelationAttributes = incomingRelationAttributes,
            incomingRelationEntityAttributes = incomingRelationEntityAttributes,
            incomingRelationOthers = emptyMap(),
        )

    return query to columns
}

fun formatSlotValue(slot: SlotValue?): String =
    when (slot) {
        is SlotValue.SvValue -> {
            formatValue(slot.v1)
        }

        is SlotValue.SvEntityId -> {
            slot.v1.toString()
        }

        is SlotValue.SvRelationId -> {
            slot.v1.toString()
        }

        is SlotValue.EntityValues -> {
            slot.v1.values.joinToString { formatValue(it) }
        }

        is SlotValue.RelationOthers -> {
            slot.v1.toString()
        }

        is SlotValue.RelationValues -> {
            slot.v1.values.joinToString { formatValue(it) }
        }

        null -> {
            "null"
        }
    }

fun formatValue(value: Value?): String =
    when (value) {
        is Value.Bool -> value.v1.toString()
        is Value.Bytes -> "<bytes>"
        is Value.Number -> value.v1.toString()
        is Value.Text -> value.v1
        null -> "null"
    }

val inspectorFieldLabelHeight = 24.dp

@Composable
fun Inspector(obj: Map<String, String>) {
    Column {
        obj.entries.forEach {
            InspectorField(it.key, it.value, false, true)
        }
        // dummy field
        Box(
            contentAlignment = Alignment.Center,
            modifier =
                Modifier
                    .padding(top = inspectorFieldLabelHeight)
                    .height(32.dp)
                    .width(200.dp)
                    .background(Color(0xFFCCCCCC)),
        ) {
            Text("+", fontWeight = FontWeight.Black, color = Color.White, fontSize = 2.em, lineHeight = 1.em)
        }
    }
}

@Composable
private fun InspectorField(
    key: String,
    value: String,
    valIsRef: Boolean,
    valIsEditable: Boolean,
) {
    var valueState = rememberTextFieldState(value) // XXX
    Column {
        Text(key, modifier = Modifier.height(inspectorFieldLabelHeight))
        TextField(
            valueState,
            contentPadding = PaddingValues.Zero,
            colors =
                TextFieldDefaults.colors(
                    unfocusedContainerColor = Color(0xFFEEEEEE),
                    focusedContainerColor = Color(0xFFDDDDDD),
                ),
            modifier = Modifier.height(32.dp).width(200.dp),
        )
    }
}

// @Composable
// @Preview
// fun InspectPanePreview() {
//    Box(modifier = Modifier.padding(16.dp)) {
//        InspectPane()
//    }
// }
