package net.trillia.stellar.desktop

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import net.trillia.stellar.AttributeKind
import net.trillia.stellar.desktop.table.Table
import net.trillia.stellar.desktop.table.TableCellText
import net.trillia.stellar.desktop.table.TableColumnDefinition
import net.trillia.stellar.formatFloat
import uniffi.stellar.logDebug
import uniffi.stellar_graph.EntityKind
import uniffi.stellar_graph.EntitySchema
import uniffi.stellar_graph.SlotValue
import uniffi.stellar_graph.Sort
import uniffi.stellar_graph.SortDirection
import uniffi.stellar_graph.TableQuery
import uniffi.stellar_graph.Value
import uniffi.stellar_sync.Schema
import kotlin.collections.component1
import kotlin.collections.component2
import kotlin.math.floor
import kotlin.math.round
import kotlin.time.measureTimedValue

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

    // Sort by first attribute
    val sort =
        entitySchema.attributes.keys.firstOrNull()?.let {
            listOf(
                Sort(
                    output = attributes[it] ?: error("unreachable"),
                    direction = SortDirection.ASCENDING,
                ),
            )
        } ?: emptyList()

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
        val durationOutput = nextOutputIndex()
        val codecOutput = nextOutputIndex()
        val bitrateOutput = nextOutputIndex()
        val sampleRateOutput = nextOutputIndex()
        val bitDepthOutput = nextOutputIndex()
        val channelsOutput = nextOutputIndex()

        val audioResourceAttributes =
            mapOf(
                AttributeKind.AudioResourceLocation to locationOutput.toUShort(),
                AttributeKind.AudioResourceHash to hashOutput.toUShort(),
                AttributeKind.AudioResourceSize to sizeOutput.toUShort(),
                AttributeKind.AudioResourceDuration to durationOutput.toUShort(),
                AttributeKind.AudioResourceCodec to codecOutput.toUShort(),
                AttributeKind.AudioResourceBitrate to bitrateOutput.toUShort(),
                AttributeKind.AudioResourceSampleRate to sampleRateOutput.toUShort(),
                AttributeKind.AudioResourceBitDepth to bitDepthOutput.toUShort(),
                AttributeKind.AudioResourceChannels to channelsOutput.toUShort(),
            )

        columns.add(
            TableColumnDefinition<List<SlotValue?>, String>(
                id = nextColumnId(),
                header = "Duration",
                initialWidth = 80.dp,
                accessor = { row ->
                    val duration = row.getOrNull(durationOutput)
                    formatDurationSlot(duration)
                },
                renderer = { TableCellText(it, textAlign = TextAlign.End) },
            ),
        )

        columns.add(
            TableColumnDefinition<List<SlotValue?>, String>(
                id = nextColumnId(),
                header = "Size",
                initialWidth = 80.dp,
                accessor = { row ->
                    val size = row.getOrNull(sizeOutput)
                    formatSizeSlot(size)
                },
                renderer = { TableCellText(it, textAlign = TextAlign.End) },
            ),
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
                    val duration = row.getOrNull(durationOutput)
                    val codec = row.getOrNull(codecOutput)
                    val bitrate = row.getOrNull(bitrateOutput)
                    val sampleRate = row.getOrNull(sampleRateOutput)
                    val bitDepth = row.getOrNull(bitDepthOutput)
                    val channels = row.getOrNull(channelsOutput)

                    "location=${formatSlotValue(
                        location,
                    )} hash=${formatSlotValue(
                        hash,
                    )} size=${formatSlotValue(
                        size,
                    )} duration=${formatSlotValue(
                        duration,
                    )} codec=${formatSlotValue(
                        codec,
                    )} bitrate=${formatSlotValue(
                        bitrate,
                    )} sampleRate=${formatSlotValue(
                        sampleRate,
                    )} bitDepth=${formatSlotValue(bitDepth)} channels=${formatSlotValue(channels)}"
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
            sort = sort,
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
        is Value.None -> ""
        null -> ""
    }

fun formatDurationSlot(slot: SlotValue?): String =
    when (slot) {
        is SlotValue.SvValue -> {
            formatDurationValue(slot.v1)
        }

        is SlotValue.EntityValues -> {
            slot.v1.values
                .distinct()
                .joinToString(", ") { formatDurationValue(it) }
        }

        null -> {
            formatDurationValue(null)
        }

        else -> {
            error("Unexpected SlotValue for duration")
        }
    }

fun formatDurationValue(value: Value?): String =
    when (value) {
        is Value.Number -> {
            formatDuration(value.v1)
        }

        Value.None -> {
            ""
        }

        null -> {
            ""
        }

        else -> {
            error("Unexpected Value for duration")
        }
    }

fun formatDuration(durationSeconds: Double): String {
    val minutes = floor(durationSeconds / 60).toInt()
    val seconds = round(durationSeconds % 60).toInt()

    return "$minutes:${seconds.toString().padStart(2, '0')}"
}

fun formatSizeSlot(slot: SlotValue?): String =
    when (slot) {
        is SlotValue.SvValue -> {
            formatSizeValue(slot.v1)
        }

        is SlotValue.EntityValues -> {
            slot.v1.values
                .distinct()
                .joinToString(", ") { formatSizeValue(it) }
        }

        null -> {
            formatSizeValue(null)
        }

        else -> {
            error("Unexpected SlotValue for size")
        }
    }

fun formatSizeValue(value: Value?): String =
    when (value) {
        is Value.Number -> {
            formatSize(value.v1)
        }

        Value.None -> {
            ""
        }

        null -> {
            ""
        }

        else -> {
            error("Unexpected Value for size")
        }
    }

fun formatSize(sizeBytes: Double): String {
    val sizeMB = sizeBytes.toFloat() / 1_000_000f
    return "${formatFloat(sizeMB, 1)} MB"
}
