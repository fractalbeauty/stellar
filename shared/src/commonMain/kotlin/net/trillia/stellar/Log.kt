package net.trillia.stellar

import uniffi.stellar.CoreException
import uniffi.stellar.logError

fun logCoreError(
    e: CoreException,
    context: String? = null,
) {
    logError(if (context != null) "$context: ${e.message()}" else e.message())
}
