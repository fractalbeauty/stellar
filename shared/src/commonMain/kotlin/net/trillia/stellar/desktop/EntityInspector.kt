package net.trillia.stellar.desktop

import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import net.trillia.stellar.AttributeKind
import net.trillia.stellar.EntityId
import net.trillia.stellar.EntityKind
import net.trillia.stellar.PreviewWrapper
import net.trillia.stellar.RelationId
import net.trillia.stellar.desktop.inspector.Inspector
import uniffi.stellar_graph.SlotValue
import uniffi.stellar_graph.TableQuery
import uniffi.stellar_graph.Value
import uniffi.stellar_sync.Schema

@Composable
fun EntityInspector(
    schema: Schema,
    selectedEntity: EntityKind,
    entityTable: EntityTableState,
    selected: Set<EntityId>,
    onSetEntityAttribute: (EntityId, AttributeKind, Value) -> Unit,
    onSetRelationAttribute: (RelationId, AttributeKind, Value) -> Unit,
) {
    val entitySchema = schema.graph.entities[selectedEntity] ?: error("missing entity schema for selected entity kind")

    val query = entityTable.query
    val idOutputIndex = checkNotNull(query.id) { "EntityTableState query missing id output" }.toInt()

    val selectedRows by remember(entityTable, selected, idOutputIndex) {
        derivedStateOf {
            entityTable.data.value.filter { it.slot<SlotValue.SvEntityId>(idOutputIndex)?.v1 in selected }
        }
    }
    val selectedIds by remember(idOutputIndex) {
        derivedStateOf { selectedRows.mapNotNullTo(mutableSetOf()) { it.slot<SlotValue.SvEntityId>(idOutputIndex)?.v1 } }
    }

    Inspector {
        entitySchema.attributes.forEach { (attributeKind, attributeSchema) ->
            val attributeOutputIndex = query.attributes.getValue(attributeKind).toInt()
            EntityInspectorField(
                fieldKey = selectedIds to attributeKind,
                attributeSchema = attributeSchema,
                values = selectedRows.map { it.slot<SlotValue.SvValue>(attributeOutputIndex)?.v1 },
                onSet = { value -> selectedIds.forEach { onSetEntityAttribute(it, attributeKind, value) } },
            )
        }

        RelationDirection.entries.forEach { direction ->
            EntityInspectorRelations(schema, selectedEntity, direction, query, selectedRows, onSetEntityAttribute, onSetRelationAttribute)
        }
    }
}

/** Renders all relations in a given direction for the inspector. */
@Composable
private fun EntityInspectorRelations(
    schema: Schema,
    selectedEntity: EntityKind,
    direction: RelationDirection,
    query: TableQuery,
    selectedRows: List<List<SlotValue?>>,
    onSetEntityAttribute: (EntityId, AttributeKind, Value) -> Unit,
    onSetRelationAttribute: (RelationId, AttributeKind, Value) -> Unit,
) {
    val (relationAttributes, relationEntityAttributes, relationOthers) =
        when (direction) {
            RelationDirection.OUTGOING -> {
                Triple(query.outgoingRelationAttributes, query.outgoingRelationEntityAttributes, query.outgoingRelationOthers)
            }

            RelationDirection.INCOMING -> {
                Triple(query.incomingRelationAttributes, query.incomingRelationEntityAttributes, query.incomingRelationOthers)
            }
        }

    val visibleRelations =
        schema.graph.relations.mapNotNull { (relationKind, relationSchema) ->
            val (self, other) =
                when (direction) {
                    RelationDirection.OUTGOING -> relationSchema.source to relationSchema.target
                    RelationDirection.INCOMING -> relationSchema.target to relationSchema.source
                }
            if (self != selectedEntity || other == EntityKind.AudioResource) return@mapNotNull null
            val otherSchema = schema.graph.entities[other] ?: return@mapNotNull null
            Triple(relationKind, relationSchema, otherSchema)
        }

    visibleRelations.forEach { (relationKind, relationSchema, otherSchema) ->
        val othersOutputIndex = relationOthers.getValue(relationKind).toInt()

        selectedRows
            .flatMap {
                it
                    .slot<SlotValue.RelationOthers>(othersOutputIndex)
                    ?.v1
                    ?.toList()
                    .orEmpty()
            }.forEach { (relationId, otherEntityId) ->
                Text(otherSchema.name, fontWeight = FontWeight.Bold)

                relationSchema.attributes.forEach { (attributeKind, attributeSchema) ->
                    val outputIndex = relationAttributes.getValue(relationKind).getValue(attributeKind).toInt()
                    EntityInspectorField(
                        fieldKey = listOf(direction, relationKind, relationId, otherEntityId, attributeKind),
                        attributeSchema = attributeSchema,
                        values =
                            listOf(
                                selectedRows.firstNotNullOfOrNull { it.slot<SlotValue.RelationValues>(outputIndex)?.v1?.get(relationId) },
                            ),
                        onSet = { onSetRelationAttribute(relationId, attributeKind, it) },
                    )
                }

                otherSchema.attributes.forEach { (attributeKind, attributeSchema) ->
                    val outputIndex = relationEntityAttributes.getValue(relationKind).getValue(attributeKind).toInt()
                    EntityInspectorField(
                        fieldKey = listOf(direction, relationKind, relationId, otherEntityId, attributeKind),
                        attributeSchema = attributeSchema,
                        values =
                            listOf(
                                selectedRows.firstNotNullOfOrNull { it.slot<SlotValue.EntityValues>(outputIndex)?.v1?.get(otherEntityId) },
                            ),
                        onSet = { onSetEntityAttribute(otherEntityId, attributeKind, it) },
                    )
                }
            }
    }
}

private inline fun <reified T : SlotValue> List<SlotValue?>.slot(outputIndex: Int): T? = getOrNull(outputIndex) as? T

private fun EntityTableState.makeFakeSongRow(
    id: EntityId,
    title: String,
    duration: Double,
): MutableList<SlotValue?> {
    // Hardcoded to be more slots than the query actually uses
    val row = MutableList<SlotValue?>(64) { null }
    row[query.id!!.toInt()] = SlotValue.SvEntityId(id)
    row[query.attributes.getValue(fakeTitleAttribute).toInt()] = SlotValue.SvValue(Value.Text(title))
    row[query.attributes.getValue(fakeDurationAttribute).toInt()] = SlotValue.SvValue(Value.Number(duration))
    return row
}

@Composable
private fun PreviewTemplate(rows: EntityTableState.() -> List<List<SlotValue?>>) {
    val schema = remember { makeFakeSchema() }
    val state =
        remember {
            EntityTableState.start(schema, fakeSongKind).apply { data.value = rows() }
        }
    val idOutputIndex = checkNotNull(state.query.id).toInt()
    val selected =
        state.data.value
            .mapNotNull { it.slot<SlotValue.SvEntityId>(idOutputIndex)?.v1 }
            .toSet()

    PreviewWrapper(padded = true) {
        EntityInspector(
            schema = schema,
            selectedEntity = fakeSongKind,
            entityTable = state,
            selected = selected,
            onSetEntityAttribute = { _, _, _ -> },
            onSetRelationAttribute = { _, _, _ -> },
        )
    }
}

@Composable
@Preview
private fun `Preview EntityInspector Default`() {
    PreviewTemplate {
        listOf(makeFakeSongRow(makeFakeEntityId(1), "Song Title", 180.0))
    }
}

@Composable
@Preview
private fun `Preview EntityInspector With multiple selected`() {
    PreviewTemplate {
        listOf(
            makeFakeSongRow(makeFakeEntityId(1), "Song One", 180.0),
            makeFakeSongRow(makeFakeEntityId(2), "Song Two", 215.0),
        )
    }
}

@Composable
@Preview
private fun `Preview EntityInspector With relations`() {
    PreviewTemplate {
        val artist1 = makeFakeEntityId(6)
        val artist2 = makeFakeEntityId(7)
        val relation1 = makeFakeRelationId(8)
        val relation2 = makeFakeRelationId(9)

        val row = makeFakeSongRow(makeFakeEntityId(1), "Song Title", 180.0)

        row[query.outgoingRelationOthers.getValue(fakeArtistRelationKind).toInt()] =
            SlotValue.RelationOthers(mapOf(relation1 to artist1, relation2 to artist2))
        query.outgoingRelationEntityAttributes.getValue(fakeArtistRelationKind).values.forEach { outputIndex ->
            row[outputIndex.toInt()] =
                SlotValue.EntityValues(
                    mapOf(
                        artist1 to Value.Text("Artist One"),
                        artist2 to Value.Text("Artist Two"),
                    ),
                )
        }

        listOf(row)
    }
}
