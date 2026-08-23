package net.trillia.stellar

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import uniffi.stellar.SchemaChangeHandler
import uniffi.stellar_sync.Schema

class SchemaManager : SchemaChangeHandler {
    private var _schemaState: MutableStateFlow<Schema?> = MutableStateFlow(null)

    val schemaState: StateFlow<Schema?>
        get() = _schemaState

    override fun onChange(schema: Schema) {
        _schemaState.value = schema
    }
}
