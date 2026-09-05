package net.trillia.stellar

import uniffi.stellar.logDebug

class FakeHandler(
    val label: String,
) : Function0<Unit>,
    Function1<Any?, Unit>,
    Function2<Any?, Any?, Unit>,
    Function3<Any?, Any?, Any?, Unit> {
    private fun log(args: List<Any?>) {
        if (args.isNotEmpty()) {
            logDebug("$label: $args")
        } else {
            logDebug(label)
        }
    }

    override fun invoke(): Unit = log(emptyList())

    override fun invoke(p1: Any?): Unit = log(listOf(p1))

    override fun invoke(
        p1: Any?,
        p2: Any?,
    ): Unit = log(listOf(p1, p2))

    override fun invoke(
        p1: Any?,
        p2: Any?,
        p3: Any?,
    ): Unit = log(listOf(p1, p2, p3))
}
