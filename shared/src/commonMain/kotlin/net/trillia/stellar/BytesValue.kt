package net.trillia.stellar

open class BytesValue(
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

class EntityId(
    bytes: ByteArray,
) : BytesValue(bytes) {
    companion object {
        fun fromByteArray(bytes: ByteArray): EntityId = EntityId(bytes)
    }
}

class RelationId(
    bytes: ByteArray,
) : BytesValue(bytes) {
    companion object {
        fun fromByteArray(bytes: ByteArray): RelationId = RelationId(bytes)
    }
}

class EntityKind(
    bytes: ByteArray,
) : BytesValue(bytes) {
    companion object {
        fun fromByteArray(bytes: ByteArray): EntityKind = EntityKind(bytes)

        val AudioResource = fromByteArray(byteArrayOf(1, 0, 0, 0, 0))
    }
}

class RelationKind(
    bytes: ByteArray,
) : BytesValue(bytes) {
    companion object {
        fun fromByteArray(bytes: ByteArray): RelationKind = RelationKind(bytes)
    }
}

class AttributeKind(
    bytes: ByteArray,
) : BytesValue(bytes) {
    companion object {
        fun fromByteArray(bytes: ByteArray): AttributeKind = AttributeKind(bytes)

        val AudioResourceProvider = fromByteArray(byteArrayOf(1, 0, 0, 0, 0))
        val AudioResourceLocation = fromByteArray(byteArrayOf(2, 0, 0, 0, 0))
        val AudioResourceHash = fromByteArray(byteArrayOf(3, 0, 0, 0, 0))
        val AudioResourceSize = fromByteArray(byteArrayOf(4, 0, 0, 0, 0))
        val AudioResourceQuality = fromByteArray(byteArrayOf(5, 0, 0, 0, 0))
        val AudioResourceDuration = fromByteArray(byteArrayOf(6, 0, 0, 0, 0))
    }
}
