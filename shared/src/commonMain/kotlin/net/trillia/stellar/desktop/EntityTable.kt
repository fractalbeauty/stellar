package net.trillia.stellar.desktop

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateSet
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import net.trillia.stellar.AttributeKind
import net.trillia.stellar.EntityId
import net.trillia.stellar.desktop.buildEntityTableColumns
import net.trillia.stellar.desktop.table.Table
import net.trillia.stellar.desktop.table.TableCellText
import net.trillia.stellar.desktop.table.TableColumnDefinition
import net.trillia.stellar.formatFloat
import net.trillia.stellar.isCtrlLikePressed
import uniffi.stellar.Core
import uniffi.stellar.CoreTableQuerySubscription
import uniffi.stellar.logDebug
import uniffi.stellar_graph.EntityKind
import uniffi.stellar_graph.EntitySchema
import uniffi.stellar_graph.RelationKind
import uniffi.stellar_graph.SlotValue
import uniffi.stellar_graph.Sort
import uniffi.stellar_graph.SortDirection
import uniffi.stellar_graph.TableQuery
import uniffi.stellar_graph.TableQueryChangeHandler
import uniffi.stellar_graph.Value
import uniffi.stellar_sync.Schema
import kotlin.collections.component1
import kotlin.collections.component2
import kotlin.collections.forEach
import kotlin.math.floor
import kotlin.math.round
import kotlin.time.measureTimedValue

@Composable
fun EntityTable(
    state: EntityTableState,
    selected: SnapshotStateSet<EntityId>,
) {
    val keyboardModifiers by rememberUpdatedState(LocalWindowInfo.current.keyboardModifiers)

    var columns by state.columns

    val handleColumnTap = { columnId: String ->
        val existing = columns.find { it.id == columnId } ?: error("Missing columnId")

        // Toggle sort direction if already sorted
        val newSortDirection =
            when (existing.sort?.second) {
                SortDirection.ASCENDING -> SortDirection.DESCENDING
                SortDirection.DESCENDING -> SortDirection.ASCENDING
                null -> SortDirection.ASCENDING
            }

        val newSortOrder =
            // Keep sort order if already sorted
            existing.sort?.first
                ?: // Add at end of order if ctrl is pressed
                if (keyboardModifiers.isCtrlLikePressed) {
                    val maxOrder = columns.maxOfOrNull { it.sort?.first ?: 0 } ?: 0
                    maxOrder + 1
                } else {
                    0
                }

        columns =
            columns.map {
                if (it.id == columnId) {
                    // Replace clicked column sort
                    it.withSort(newSortOrder to newSortDirection)
                } else {
                    // Keep other sorts if ctrl is pressed
                    if (keyboardModifiers.isCtrlLikePressed) {
                        it
                    } else {
                        it.withSort(null)
                    }
                }
            }
    }

    val data by state.data

    Table(
        data,
        state.tableColumns,
        { row ->
            when (val entityId = row[0]) {
                is SlotValue.SvEntityId -> selected.contains(entityId.v1)
                else -> error("expected EntityId in slot 0")
            }
        },
        { row ->
            when (val entityId = row[0]) {
                is SlotValue.SvEntityId -> selected.add(entityId.v1)
                else -> error("expected EntityId in slot 0")
            }
        },
        onDeselectRow = { row ->
            when (val entityId = row[0]) {
                is SlotValue.SvEntityId -> selected.remove(entityId.v1)
                else -> error("expected EntityId in slot 0")
            }
        },
        onDeselectAllRows = {
            selected.clear()
        },
        onColumnTap = handleColumnTap,
    )
}

class EntityTableState(
    val schema: Schema,
    val entityKind: EntityKind,
    val entitySchema: EntitySchema,
    val columns: MutableState<List<EntityTableColumn>>,
    val data: MutableState<List<List<SlotValue?>>>,
) {
    val queryAndTableColumns by derivedStateOf {
        buildEntityTableQuery(schema, entityKind, entitySchema, columns.value)
    }
    val query: TableQuery
        get() = queryAndTableColumns.first
    val tableColumns: List<TableColumnDefinition<List<SlotValue?>, *>>
        get() = queryAndTableColumns.second

    companion object {
        fun start(
            schema: Schema,
            entityKind: EntityKind,
        ): EntityTableState {
            val entitySchema = schema.graph.entities[entityKind] ?: error("missing entity schema for selected entity kind")

            val initialColumns = buildEntityTableColumns(schema, entityKind, entitySchema)

            return EntityTableState(
                schema,
                entityKind,
                entitySchema,
                columns = mutableStateOf(initialColumns),
                data = mutableStateOf(emptyList()),
            )
        }
    }
}

@Composable
fun rememberEntityTableState(
    core: Core,
    schema: Schema,
    entityKind: EntityKind,
): EntityTableState {
    val coroutineScope = rememberCoroutineScope()

    val state = remember(schema, entityKind) { EntityTableState.start(schema, entityKind) }

    // Create the subscription on mount and whenever the query changes
    val subscription =
        remember(state.query) {
            lateinit var subscription: CoreTableQuerySubscription
            val (created, elapsed) =
                measureTimedValue {
                    core.subscribeTableQuery(
                        state.query,
                        // Called from a background thread, so state updates need to be
                        // launched on the dispatcher for the main thread.
                        object : TableQueryChangeHandler {
                            override fun onChange() {
                                coroutineScope.launch(Dispatchers.Main) {
                                    state.data.value = subscription.rows()
                                }
                            }
                        },
                    )
                }
            subscription = created

            state.data.value = subscription.rows()
            logDebug(
                "TableQuery subscription returned ${state.data.value.size} rows in $elapsed (${elapsed / state.data.value.size} per row)",
            )

            subscription
        }

    // Cancel the previous subscription whenever the subscription changes, and on unmount.
    DisposableEffect(subscription) {
        onDispose {
            subscription.cancel()
        }
    }

    return state
}

fun buildEntityTableColumns(
    schema: Schema,
    entityKind: EntityKind,
    entitySchema: EntitySchema,
): List<EntityTableColumn> {
    val columns = mutableListOf<EntityTableColumn>()

    // Add columns for all attributes
    entitySchema.attributes.entries.forEach { (attributeKind, attributeSchema) ->
        columns.add(
            EntityTableColumn.Attribute(
                id = "Attribute-$attributeKind",
                label = attributeSchema.name,
                sort = null,
                attributeKind = attributeKind,
            ),
        )
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

    outgoingRelations.forEach { (relationKind, relationSchema) ->
        relationSchema.attributes.forEach { (attributeKind, attributeSchema) ->
            columns.add(
                EntityTableColumn.RelationAttribute(
                    id = "RelationAttribute-$relationKind-$attributeKind",
                    label = "${relationSchema.name} ${attributeSchema.name}",
                    sort = null,
                    relationKind = relationKind,
                    attributeKind = attributeKind,
                    relationDirection = RelationDirection.OUTGOING,
                ),
            )
        }

        val otherSchema = schema.graph.entities[relationSchema.target] ?: return@forEach
        otherSchema.attributes.forEach { (attributeKind, attributeSchema) ->
            columns.add(
                EntityTableColumn.RelationEntityAttribute(
                    id = "RelationEntityAttribute-$relationKind-$attributeKind",
                    label = "${otherSchema.name} ${attributeSchema.name}",
                    sort = null,
                    relationKind = relationKind,
                    attributeKind = attributeKind,
                    relationDirection = RelationDirection.OUTGOING,
                ),
            )
        }
    }
    incomingRelations.forEach { (relationKind, relationSchema) ->
        relationSchema.attributes.forEach { (attributeKind, attributeSchema) ->
            columns.add(
                EntityTableColumn.RelationAttribute(
                    id = "RelationAttribute-$relationKind-$attributeKind",
                    label = "${relationSchema.name} ${attributeSchema.name}",
                    sort = null,
                    relationKind = relationKind,
                    attributeKind = attributeKind,
                    relationDirection = RelationDirection.INCOMING,
                ),
            )
        }

        val otherSchema = schema.graph.entities[relationSchema.source] ?: return@forEach
        otherSchema.attributes.forEach { (attributeKind, attributeSchema) ->
            columns.add(
                EntityTableColumn.RelationEntityAttribute(
                    id = "RelationEntityAttribute-$relationKind-$attributeKind",
                    label = "${otherSchema.name} ${attributeSchema.name}",
                    sort = null,
                    relationKind = relationKind,
                    attributeKind = attributeKind,
                    relationDirection = RelationDirection.INCOMING,
                ),
            )
        }
    }

    return columns
}

sealed class EntityTableColumn(
    val id: String,
    val header: String,
    val sort: Pair<Int, SortDirection>?,
) {
    abstract fun withSort(sort: Pair<Int, SortDirection>?): EntityTableColumn

    class Attribute(
        id: String,
        label: String,
        sort: Pair<Int, SortDirection>?,
        val attributeKind: AttributeKind,
    ) : EntityTableColumn(id, label, sort) {
        fun copy(
            label: String = this.header,
            sort: Pair<Int, SortDirection>? = this.sort,
            attributeKind: AttributeKind = this.attributeKind,
        ) = Attribute(id, label, sort, attributeKind)

        override fun withSort(sort: Pair<Int, SortDirection>?) = copy(sort = sort)
    }

    class RelationAttribute(
        id: String,
        label: String,
        sort: Pair<Int, SortDirection>?,
        val relationKind: RelationKind,
        val attributeKind: AttributeKind,
        val relationDirection: RelationDirection,
    ) : EntityTableColumn(id, label, sort) {
        fun copy(
            label: String = this.header,
            sort: Pair<Int, SortDirection>? = this.sort,
            relationKind: RelationKind = this.relationKind,
            attributeKind: AttributeKind = this.attributeKind,
            relationDirection: RelationDirection = this.relationDirection,
        ) = RelationAttribute(id, label, sort, relationKind, attributeKind, relationDirection)

        override fun withSort(sort: Pair<Int, SortDirection>?) = copy(sort = sort)
    }

    class RelationEntityAttribute(
        id: String,
        label: String,
        sort: Pair<Int, SortDirection>?,
        val relationKind: RelationKind,
        val attributeKind: AttributeKind,
        val relationDirection: RelationDirection,
    ) : EntityTableColumn(id, label, sort) {
        fun copy(
            label: String = this.header,
            sort: Pair<Int, SortDirection>? = this.sort,
            relationKind: RelationKind = this.relationKind,
            attributeKind: AttributeKind = this.attributeKind,
            relationDirection: RelationDirection = this.relationDirection,
        ) = RelationEntityAttribute(id, label, sort, relationKind, attributeKind, relationDirection)

        override fun withSort(sort: Pair<Int, SortDirection>?) = copy(sort = sort)
    }
}

enum class RelationDirection {
    OUTGOING,
    INCOMING,
}

fun buildEntityTableQuery(
    schema: Schema,
    entityKind: EntityKind,
    entitySchema: EntitySchema,
    columns: List<EntityTableColumn>,
): Pair<TableQuery, List<TableColumnDefinition<List<SlotValue?>, *>>> {
    var nextOutputIndexInner = 0
    val nextOutputIndex = {
        val outputIndex = nextOutputIndexInner
        nextOutputIndexInner += 1
        outputIndex
    }

    val entityIdOutputIndex = nextOutputIndex()

    val tableColumns = mutableListOf<TableColumnDefinition<List<SlotValue?>, *>>()

    val sortedOutputs = mutableMapOf<Int, Sort>()

    val attributes = mutableMapOf<AttributeKind, UShort>()
    val outgoingRelationAttributes = mutableMapOf<RelationKind, MutableMap<AttributeKind, UShort>>()
    val outgoingRelationEntityAttributes = mutableMapOf<RelationKind, MutableMap<AttributeKind, UShort>>()
    val incomingRelationAttributes = mutableMapOf<RelationKind, MutableMap<AttributeKind, UShort>>()
    val incomingRelationEntityAttributes = mutableMapOf<RelationKind, MutableMap<AttributeKind, UShort>>()

    columns.forEach { column ->
        when (column) {
            is EntityTableColumn.Attribute -> {
                val outputIndex = nextOutputIndex()

                tableColumns.add(
                    TableColumnDefinition<List<SlotValue?>, String>(
                        id = column.id,
                        header = column.header,
                        initialWidth = 200.dp,
                        sort = column.sort,
                        accessor = { row -> formatSlotValue(row.getOrNull(outputIndex)) },
                        renderer = { TableCellText(it) },
                    ),
                )

                column.sort?.let { (order, direction) ->
                    sortedOutputs[order] =
                        Sort(
                            output = outputIndex.toUShort(),
                            direction = direction,
                        )
                }

                attributes[column.attributeKind] = outputIndex.toUShort()
            }

            is EntityTableColumn.RelationAttribute -> {
                val outputIndex = nextOutputIndex()

                tableColumns.add(
                    TableColumnDefinition<List<SlotValue?>, String>(
                        id = column.id,
                        header = column.header,
                        initialWidth = 200.dp,
                        sort = column.sort,
                        accessor = { row -> formatSlotValue(row.getOrNull(outputIndex)) },
                        renderer = { TableCellText(it) },
                    ),
                )

                column.sort?.let { (order, direction) ->
                    sortedOutputs[order] =
                        Sort(
                            output = outputIndex.toUShort(),
                            direction = direction,
                        )
                }

                when (column.relationDirection) {
                    RelationDirection.OUTGOING -> {
                        outgoingRelationAttributes
                            .getOrPut(
                                column.relationKind,
                            ) { mutableMapOf() }
                            .getOrPut(column.attributeKind) { outputIndex.toUShort() }
                    }

                    RelationDirection.INCOMING -> {
                        incomingRelationAttributes
                            .getOrPut(
                                column.relationKind,
                            ) { mutableMapOf() }
                            .getOrPut(column.attributeKind) { outputIndex.toUShort() }
                    }
                }
            }

            is EntityTableColumn.RelationEntityAttribute -> {
                val outputIndex = nextOutputIndex()

                tableColumns.add(
                    TableColumnDefinition<List<SlotValue?>, String>(
                        id = column.id,
                        header = column.header,
                        initialWidth = 200.dp,
                        sort = column.sort,
                        accessor = { row -> formatSlotValue(row.getOrNull(outputIndex)) },
                        renderer = { TableCellText(it) },
                    ),
                )

                column.sort?.let { (order, direction) ->
                    sortedOutputs[order] =
                        Sort(
                            output = outputIndex.toUShort(),
                            direction = direction,
                        )
                }

                when (column.relationDirection) {
                    RelationDirection.OUTGOING -> {
                        outgoingRelationEntityAttributes
                            .getOrPut(
                                column.relationKind,
                            ) { mutableMapOf() }
                            .getOrPut(column.attributeKind) { outputIndex.toUShort() }
                    }

                    RelationDirection.INCOMING -> {
                        incomingRelationEntityAttributes
                            .getOrPut(
                                column.relationKind,
                            ) { mutableMapOf() }
                            .getOrPut(column.attributeKind) { outputIndex.toUShort() }
                    }
                }
            }
        }
    }

    val nextColumnId = {
        tableColumns.size.toString()
    }

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

        tableColumns.add(
            TableColumnDefinition<List<SlotValue?>, String>(
                id = nextColumnId(),
                header = "Duration",
                initialWidth = 80.dp,
                sort = null,
                accessor = { row ->
                    val duration = row.getOrNull(durationOutput)
                    formatDurationSlot(duration)
                },
                renderer = { TableCellText(it, textAlign = TextAlign.End) },
            ),
        )

        tableColumns.add(
            TableColumnDefinition<List<SlotValue?>, String>(
                id = nextColumnId(),
                header = "Size",
                initialWidth = 80.dp,
                sort = null,
                accessor = { row ->
                    val size = row.getOrNull(sizeOutput)
                    formatSizeSlot(size)
                },
                renderer = { TableCellText(it, textAlign = TextAlign.End) },
            ),
        )

        tableColumns.add(
            TableColumnDefinition<List<SlotValue?>, String>(
                id = nextColumnId(),
                header = "Audio Resource",
                initialWidth = 200.dp,
                sort = null,
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
            outgoingRelationEntityAttributes[audioResourceRelation.key] = audioResourceAttributes.toMutableMap()
        } else {
            incomingRelationEntityAttributes[audioResourceRelation.key] = audioResourceAttributes.toMutableMap()
        }
    }

    val tableQuerySort = sortedOutputs.entries.sortedBy { it.key }.map { it.value }

    val query =
        TableQuery(
            entity = entityKind,
            id = entityIdOutputIndex.toUShort(),
            attributes = attributes,
            outgoingRelationAttributes = outgoingRelationAttributes,
            outgoingRelationEntityAttributes = outgoingRelationEntityAttributes,
            outgoingRelationOthers = emptyMap(),
            incomingRelationAttributes = incomingRelationAttributes,
            incomingRelationEntityAttributes = incomingRelationEntityAttributes,
            incomingRelationOthers = emptyMap(),
            sort = tableQuerySort,
        )

    return query to tableColumns
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
