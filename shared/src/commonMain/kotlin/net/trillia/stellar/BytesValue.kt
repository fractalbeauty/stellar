package net.trillia.stellar

data class BytesValue(
    val bytes: ByteArray,
) : Comparable<BytesValue> {
    companion object {
        fun fromByteArray(bytes: ByteArray): BytesValue = BytesValue(bytes)
    }

    fun toByteArray(): ByteArray = this.bytes

    override fun compareTo(other: BytesValue): Int {
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
        if (other !is BytesValue) return false
        return this.bytes.contentEquals(other.bytes)
    }

    override fun hashCode(): Int = this.bytes.contentHashCode()
}
