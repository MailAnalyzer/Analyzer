use log::debug;
use reqwest::Client;
use crate::entity::Entity;
use crate::investigation::wikidata::data::{WikidataEntity, WikidataInvestFailure};
use crate::investigation::wikidata::data_request::{get_entity, search_by_name};

mod data_request;
mod data;
mod resolver;

pub async fn get_wikidata_entity(
    entity: &Entity,
) -> Result<WikidataEntity, WikidataInvestFailure> {
    let client = Client::new();
    let entities_id = search_by_name(&client, &entity.name).await?;

    debug!(
        "Found {} entities from wikidata for search query '{}'",
        entities_id.len(),
        entity.name
    );

    let entity_id = entities_id.first().unwrap();

    debug!("Keeping entity ID {entity_id} as it is the first one in the list");

    get_entity(&client, entity_id).await
}

#[cfg(test)]
mod test {
    use crate::entity::Entity;
    use crate::investigation::wikidata::get_wikidata_entity;

    #[tokio::test]
    async fn test_wikidata_entity() {
        let result = get_wikidata_entity(&Entity {
            name: String::from("Pluralsight"),
            kind: String::from("company"),
            additional_info: vec![],
        })
            .await;
        println!("{result:#?}");
    }
}
