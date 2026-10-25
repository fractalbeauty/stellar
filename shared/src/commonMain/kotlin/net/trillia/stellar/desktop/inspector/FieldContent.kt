package net.trillia.stellar.desktop.inspector

sealed interface FieldContent<out T> {
    data object None : FieldContent<Nothing>

    data object Mixed : FieldContent<Nothing>

    data class Value<T>(
        val value: T,
    ) : FieldContent<T>

    data class Invalid(
        val invalidValue: String,
        val reason: String,
    ) : FieldContent<Nothing>
}
