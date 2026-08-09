package net.trillia.stellar

import uniffi.stellar.SchemaChangeHandler
import uniffi.stellar_graph.Schema

class SchemaManager : SchemaChangeHandler {
    override fun onChange(schema: Schema) {
        println("schema onChange: $schema")
    }
}
