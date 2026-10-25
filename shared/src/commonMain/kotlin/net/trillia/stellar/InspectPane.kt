package net.trillia.stellar

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateSetOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import net.trillia.stellar.desktop.EntityInspector
import net.trillia.stellar.desktop.EntityTable
import net.trillia.stellar.desktop.rememberEntityTableState
import uniffi.stellar.Core
import uniffi.stellar_graph.EntityKind

@Composable
fun InspectPane(
    core: Core,
    schemaManager: SchemaManager,
    selectedEntity: EntityKind,
) {
    val schemaNullable by schemaManager.schemaState.collectAsState()
    val schema = schemaNullable ?: return

    val state = rememberEntityTableState(core, schema, entityKind = selectedEntity)

    val selected = remember(selectedEntity) { mutableStateSetOf<EntityId>() }

    Row {
        Box(Modifier.weight(1f)) {
            EntityTable(
                state = state,
                selected = selected,
            )
        }
        if (selected.isNotEmpty()) {
            EntityInspector(
                schema = schema,
                selectedEntity = selectedEntity,
                entityTable = state,
                selected = selected,
                onSetEntityAttribute = core::setEntityAttribute,
                onSetRelationAttribute = core::setRelationAttribute,
            )
        }
    }
}

// @Composable
// @Preview
// fun InspectPanePreview() {
//    Box(modifier = Modifier.padding(16.dp)) {
//        InspectPane()
//    }
// }
