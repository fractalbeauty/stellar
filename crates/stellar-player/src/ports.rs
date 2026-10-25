use async_trait::async_trait;
use std::{path::PathBuf, sync::Arc};
use stellar_graph::{
    database::Database,
    entity::{AuthorId, EntityId, RelationDirection, RelationKind},
};
use stellar_resources::audio::{AUDIO_RESOURCE_ENTITY, AudioResource};

/// Downloads audio from peers.
#[async_trait]
pub trait PlayerPeersPort: Send + Sync {
    /// Downloads an audio resource from a peer and creates a new local audio resource related to
    /// `parent` via `relation`. `relation_direction` is from the audio resource to the parent.
    /// Returns the local path after downloading.
    async fn download_audio_resource(
        &self,
        endpoint_id: AuthorId,
        audio_resource: EntityId,
        parent: EntityId,
        relation: RelationKind,
        relation_direction: RelationDirection,
    ) -> Result<PathBuf, anyhow::Error>;
}

/// Resolves entities to the audio resources related to them.
pub trait PlayerResourcesPort: Send + Sync {
    /// The AuthorId identifying this device, for matching against `AudioResourceLocation::device`.
    fn local_author_id(&self) -> AuthorId;

    /// All audio resources related (by any relation) to `entity`.
    fn list_related_audio_resources(
        &self,
        entity: EntityId,
    ) -> Result<Vec<RelatedAudioResource>, anyhow::Error>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct RelatedAudioResource {
    /// The audio resource entity.
    pub entity: EntityId,
    pub audio_resource: AudioResource,
    pub relation: RelationKind,
    /// Direction of `relation` from the audio resource to the parent.
    pub relation_direction: RelationDirection,
}

pub struct PlayerResourcesAdapter {
    database: Database,
    author_id: AuthorId,
}

impl PlayerResourcesAdapter {
    pub fn new(database: Database, author_id: AuthorId) -> Arc<Self> {
        Arc::new(Self {
            database,
            author_id,
        })
    }
}

impl PlayerResourcesPort for PlayerResourcesAdapter {
    fn local_author_id(&self) -> AuthorId {
        self.author_id
    }

    // TODO: optimize this query
    fn list_related_audio_resources(
        &self,
        entity: EntityId,
    ) -> Result<Vec<RelatedAudioResource>, anyhow::Error> {
        let entities = self.database.get_entities()?;

        let related = self
            .database
            .get_relations()?
            .into_iter()
            .filter(|(_, relation)| !relation.metadata.deleted)
            .filter_map(|(relation_id, relation)| {
                let (resource_entity, relation_direction) = if relation.metadata.source == entity {
                    // Relation from `entity` to `target` audio resource (incoming)
                    (relation.metadata.target, RelationDirection::Incoming)
                } else if relation.metadata.target == entity {
                    // Relation from `source` audio resource to `entity` (outgoing)
                    (relation.metadata.source, RelationDirection::Outgoing)
                } else {
                    return None;
                };

                if resource_entity.kind() != AUDIO_RESOURCE_ENTITY {
                    return None;
                }

                Some((relation_id.kind(), resource_entity, relation_direction))
            })
            .filter_map(|(relation, resource_entity, relation_direction)| {
                let data = entities.get(&resource_entity)?;

                if data.metadata.deleted {
                    return None;
                }

                let audio_resource = AudioResource::try_from_attributes(&data.attributes).ok()?;

                Some(RelatedAudioResource {
                    entity: resource_entity,
                    audio_resource,
                    relation,
                    relation_direction,
                })
            })
            .collect();

        Ok(related)
    }
}
