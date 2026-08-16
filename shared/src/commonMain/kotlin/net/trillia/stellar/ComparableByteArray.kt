package net.trillia.stellar

class ComparableByteArray(
    val bytes: ByteArray,
) : Comparable<ComparableByteArray> {
    override fun compareTo(other: ComparableByteArray): Int {
        if (this.bytes.size != other.bytes.size) {
            return this.bytes.size.compareTo(other.bytes.size)
        }

        for (i in this.bytes.indices) {
            val a = this.bytes[i].toUByte()
            val b = other.bytes[i].toUByte()
            if (a != b) return a.compareTo(b)
        }

        return 0
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ComparableByteArray) return false
        return this.bytes.contentEquals(other.bytes)
    }

    override fun hashCode(): Int = this.bytes.contentHashCode()
}

fun ByteArray.comparable() = ComparableByteArray(this)
