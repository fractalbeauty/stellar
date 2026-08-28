package net.trillia.stellar

import kotlin.contracts.ExperimentalContracts
import kotlin.contracts.InvocationKind
import kotlin.contracts.contract

@OptIn(ExperimentalContracts::class)
inline fun <T> T.mapIf(
    condition: Boolean,
    block: (T) -> T,
): T {
    contract {
        callsInPlace(block, InvocationKind.AT_MOST_ONCE)
    }
    return if (condition) {
        block(this)
    } else {
        this
    }
}
