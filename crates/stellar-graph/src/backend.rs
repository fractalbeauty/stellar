use anyhow::Context;
use fjall::{Database, Keyspace, Slice};
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

/// Backend for the store. Either Fjall or in-memory.
#[derive(Clone)]
pub enum Backend {
    Fjall {
        // `fjall::Database` needs to be kept alive while the keyspace is in use
        _database: Database,
        keyspace: Keyspace,
    },
    Memory(MemoryBackend),
}

impl Backend {
    pub fn get(&self, key: impl AsRef<[u8]>) -> Result<Option<Slice>, anyhow::Error> {
        match self {
            Backend::Fjall { keyspace, .. } => Ok(keyspace.get(key.as_ref())?),
            Backend::Memory(memory) => Ok(memory.get(key.as_ref()).map(Slice::from)),
        }
    }

    pub fn insert(&self, key: impl AsRef<[u8]>, value: Vec<u8>) -> Result<(), anyhow::Error> {
        match self {
            Backend::Fjall { keyspace, .. } => {
                keyspace.insert(key.as_ref(), value)?;
            }
            Backend::Memory(memory) => memory.insert(key.as_ref().to_vec(), value),
        }
        Ok(())
    }

    pub fn prefix(&self, prefix: impl AsRef<[u8]>) -> PrefixIter {
        match self {
            Backend::Fjall { keyspace, .. } => PrefixIter::Fjall(keyspace.prefix(prefix.as_ref())),
            Backend::Memory(memory) => PrefixIter::Memory(memory.prefix(prefix.as_ref())),
        }
    }
}

#[derive(Clone, Default)]
pub struct MemoryBackend {
    entries: Arc<RwLock<BTreeMap<Vec<u8>, Vec<u8>>>>,
}

impl MemoryBackend {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.entries
            .read()
            .expect("lock poisoned")
            .get(key)
            .cloned()
    }

    fn insert(&self, key: Vec<u8>, value: Vec<u8>) {
        self.entries
            .write()
            .expect("lock poisoned")
            .insert(key, value);
    }

    fn prefix(&self, prefix: &[u8]) -> std::vec::IntoIter<(Vec<u8>, Vec<u8>)> {
        let entries = self.entries.read().expect("lock poisoned");
        entries
            .range(prefix.to_vec()..)
            .take_while(|(key, _)| key.starts_with(prefix))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>()
            .into_iter()
    }
}

pub enum PrefixIter {
    Fjall(fjall::Iter),
    Memory(std::vec::IntoIter<(Vec<u8>, Vec<u8>)>),
}

impl Iterator for PrefixIter {
    type Item = Result<(Slice, Slice), anyhow::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            PrefixIter::Fjall(iter) => Some(
                iter.next()?
                    .into_inner()
                    .context("Fjall error reading entry"),
            ),
            PrefixIter::Memory(iter) => {
                let (key, value) = iter.next()?;
                Some(Ok((Slice::from(key), Slice::from(value))))
            }
        }
    }
}
