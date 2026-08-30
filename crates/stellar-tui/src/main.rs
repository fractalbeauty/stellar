use clap::{Args, Parser};
use std::{collections::HashMap, sync::Arc, time::Duration};
use stellar::{
    core::{
        Core, CoreAttribute, CoreEntity, CoreRelation, DevicesChangeHandler, SchemaChangeHandler,
    },
    graph::entity::{AttributeKind, Value},
    sync::{devices::DevicesState, schema::Schema},
};

/// Stellar TUI
#[derive(Parser)]
#[command(about)]
struct Cli {
    #[command(flatten)]
    profile: Profile,

    /// Print all entities as JSON and exit, for debugging.
    #[arg(long)]
    print_entities: bool,

    /// Print all relations as JSON and exit, for debugging.
    #[arg(long)]
    print_relations: bool,
}

#[derive(Args)]
#[group(required = false, multiple = false)]
struct Profile {
    #[arg(long, short)]
    profile: Option<String>,

    #[arg(long, short)]
    anonymous: bool,
}

#[tokio::main]
async fn main() {
    // tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let profile = if let Some(profile) = cli.profile.profile {
        profile
    } else if cli.profile.anonymous {
        let id: String = std::iter::repeat_with(fastrand::lowercase)
            .take(7)
            .collect();
        format!("anonymous-{id}")
    } else {
        "default".to_string()
    };

    let core = Core::spawn(
        profile,
        Arc::new(TuiDevicesChangeHandler),
        Arc::new(TuiSchemaChangeHandler),
    )
    .await
    .expect("Should spawn core");

    if cli.print_entities {
        let entities = core.get_entities().expect("Should get entities");
        let entities: HashMap<String, serde_json::Value> = entities
            .into_iter()
            .map(|(id, entity)| (id.to_string(), entity_to_json(&entity)))
            .collect();
        println!(
            "{}",
            serde_json::to_string(&entities).expect("Should serialize entities")
        );
        return;
    }

    if cli.print_relations {
        let relations = core.get_relations().expect("Should get relations");
        let relations: HashMap<String, serde_json::Value> = relations
            .into_iter()
            .map(|(id, relation)| (id.to_string(), relation_to_json(&relation)))
            .collect();
        println!(
            "{}",
            serde_json::to_string(&relations).expect("Should serialize relations")
        );
        return;
    }

    if std::env::var("STELLAR_ADD_ENTITY").is_ok_and(|var| !var.is_empty()) {
        todo!()
    }
    if std::env::var("STELLAR_ADD_ENTITYKIND").is_ok_and(|var| !var.is_empty()) {
        core.create_schema_entity("Entity".to_string())
            .await
            .unwrap();
    }
    if std::env::var("STELLAR_AUTH").is_ok_and(|var| !var.is_empty()) {
        let verification_uri_complete = core
            .start_device_code_flow()
            .await
            .expect("Should start device code flow");

        println!(
            "Visit the following URL to authorize the device: {}",
            verification_uri_complete
        );
    }

    if let Ok(endpoint_id) = std::env::var("STELLAR_CONNECT_TO") {
        core.add_device(endpoint_id, None).unwrap();
    }

    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn attributes_to_json(attributes: &HashMap<AttributeKind, CoreAttribute>) -> serde_json::Value {
    serde_json::Value::Object(
        attributes
            .iter()
            .map(|(attribute, value)| (attribute.to_string(), value_to_json(&value.value)))
            .collect(),
    )
}

fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Text(text) => serde_json::Value::String(text.clone()),
        Value::Number(number) => serde_json::json!(number.into_inner()),
        Value::Bool(value) => serde_json::Value::Bool(*value),
        Value::Bytes(bytes) => serde_json::Value::String(hex::encode(bytes)),
    }
}

fn entity_to_json(entity: &CoreEntity) -> serde_json::Value {
    serde_json::json!({
        "kind": entity.kind.to_string(),
        "attributes": attributes_to_json(&entity.attributes),
    })
}

fn relation_to_json(relation: &CoreRelation) -> serde_json::Value {
    serde_json::json!({
        "kind": relation.kind.to_string(),
        "source": relation.source.to_string(),
        "target": relation.target.to_string(),
        "attributes": attributes_to_json(&relation.attributes),
    })
}

struct TuiDevicesChangeHandler;
impl DevicesChangeHandler for TuiDevicesChangeHandler {
    fn on_change(&self, devices: DevicesState) {
        tracing::debug!("TuiDevicesChangeHandler on_change, devices: {devices:?}");
    }
}

struct TuiSchemaChangeHandler;
impl SchemaChangeHandler for TuiSchemaChangeHandler {
    fn on_change(&self, schema: Schema) {
        tracing::debug!("TuiSchemaChangeHandler on_change, schema: {schema:?}");
    }
}
