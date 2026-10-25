package net.trillia.stellar.desktop

import net.trillia.stellar.AttributeKind
import net.trillia.stellar.EntityId
import net.trillia.stellar.EntityKind
import net.trillia.stellar.RelationId
import net.trillia.stellar.RelationKind
import uniffi.stellar_graph.AttributeSchema
import uniffi.stellar_graph.EntitySchema
import uniffi.stellar_graph.GraphSchema
import uniffi.stellar_graph.RelationSchema
import uniffi.stellar_graph.ValueKind
import uniffi.stellar_import.Rule
import uniffi.stellar_import.Rules
import uniffi.stellar_sync.Schema

val fakeSongKind = EntityKind.fromByteArray(byteArrayOf(1, 1, 0, 0, 0))
val fakeArtistKind = EntityKind.fromByteArray(byteArrayOf(2, 1, 0, 0, 0))

val fakeArtistRelationKind = RelationKind.fromByteArray(byteArrayOf(1, 1, 0, 0, 0))

val fakeTitleAttribute = AttributeKind.fromByteArray(byteArrayOf(1, 1, 0, 0, 0))
val fakeDurationAttribute = AttributeKind.fromByteArray(byteArrayOf(2, 1, 0, 0, 0))
val fakeArtistNameAttribute = AttributeKind.fromByteArray(byteArrayOf(3, 1, 0, 0, 0))

fun makeFakeEntityId(n: Byte) = EntityId.fromByteArray(byteArrayOf(n, 0, 0, 0, 0))

fun makeFakeRelationId(n: Byte) = RelationId.fromByteArray(byteArrayOf(n, 0, 0, 0, 0))

fun makeFakeSchema(): Schema =
    Schema(
        graph =
            GraphSchema(
                entities =
                    mapOf(
                        fakeSongKind to
                            EntitySchema(
                                name = "Song",
                                attributes =
                                    mapOf(
                                        fakeTitleAttribute to AttributeSchema("Title", ValueKind.TEXT),
                                        fakeDurationAttribute to AttributeSchema("Duration", ValueKind.NUMBER),
                                    ),
                            ),
                        fakeArtistKind to
                            EntitySchema(
                                name = "Artist",
                                attributes =
                                    mapOf(
                                        fakeArtistNameAttribute to AttributeSchema("Name", ValueKind.TEXT),
                                    ),
                            ),
                    ),
                relations =
                    mapOf(
                        fakeArtistRelationKind to
                            RelationSchema(
                                name = "Artist",
                                source = fakeSongKind,
                                target = fakeArtistKind,
                                attributes = emptyMap(),
                            ),
                    ),
            ),
        importRules =
            Rules(
                Rule(emptyList(), emptyList()),
                emptyMap(),
                emptyMap(),
                fakeSongKind,
            ),
    )
