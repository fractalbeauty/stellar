use stellar_graph::entity::EntityId;

struct Queue {}

enum QueueEntry {
    Item(QueueItem),
    Group(QueueGroup),
}

struct QueueItem {
    audio_resource: EntityId,
}

struct QueueGroup {
    items: Vec<QueueEntry>,
}
