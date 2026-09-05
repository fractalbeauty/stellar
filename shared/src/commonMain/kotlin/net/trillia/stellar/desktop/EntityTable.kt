package net.trillia.stellar.desktop

import net.trillia.stellar.formatFloat
import uniffi.stellar_graph.SlotValue
import uniffi.stellar_graph.Value
import kotlin.math.floor
import kotlin.math.round

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
