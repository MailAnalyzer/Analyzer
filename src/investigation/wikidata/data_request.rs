use log::warn;
use reqwest::Client;
use rocket::futures::StreamExt;
use rocket::serde::Deserialize;
use rocket::serde::json::Value;
use rocket::futures::FutureExt;
use crate::investigation::wikidata::data::{Snak, WikidataClaim, WikidataEntity, WikidataInvestFailure, WikidataReference, WikidataValue};

#[derive(Deserialize, Debug)]
struct Pid(usize);

impl Pid {
    fn matches(&self, str: &str) -> bool {
        str.starts_with('P') && str[1..] == self.0.to_string()
    }
}

/// Searches on wikidata about a specific entity string
/// and returns the found entities ID
pub async fn search_by_name(
    client: &Client,
    name: &str,
) -> Result<Vec<String>, WikidataInvestFailure> {
    let response = client
        .get(format!("https://www.wikidata.org/w/api.php?action=wbsearchentities&search={name}&language=en&format=json"))
        .send()
        .await
        .map_err(WikidataInvestFailure::from)?;

    let results = response.json::<Value>()
        .await
        .map_err(WikidataInvestFailure::from)?;

    let results = results
        .get("search")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.get("id").unwrap().as_str().unwrap().to_string())
        .collect();

    Ok(results)
}

pub async fn get_entity(
    client: &Client,
    entity_id: &str,
) -> Result<WikidataEntity, WikidataInvestFailure> {
    let response = client
        .get(format!(
            "https://www.wikidata.org/w/api.php?action=wbgetentities&ids={entity_id}&format=json"
        ))
        .send()
        .await?;

    let value = response.json::<Value>().await?;
    let entity = value.get("entities").unwrap().get(entity_id).unwrap();

    let name = get_entity_name(client, entity_id).await?;
    let aliases: Vec<_> = entity
        .get("aliases")
        .unwrap()
        .get("en")
        .map(|v| {
            v.as_array()
                .unwrap()
                .iter()
                .filter(|v| v.get("language").is_some_and(|v| v.as_str() == Some("en")))
                .filter_map(|v| v.get("value").and_then(|v| v.as_str()))
                .map(|v| v.to_string())
                .collect()
        })
        .unwrap_or_default();

    let claims = entity.get("claims").unwrap().as_object().unwrap();

    let claims = tokio_stream::iter(claims.iter())
        .map(|(pid, claims)| {
            extract_claims(client, pid, claims.as_array().unwrap())
                .map(|v| (pid.to_string(), v.unwrap()))
        })
        .buffered(claims.len())
        .collect()
        .await;

    Ok(WikidataEntity {
        name,
        aliases,
        claims,
    })
}

async fn extract_claim(
    client: &Client,
    cpid: &str,
    claim: &Value,
) -> Result<WikidataClaim, WikidataInvestFailure> {
    let Some(value) = claim
        .get("mainsnak")
        .unwrap()
        .get("datavalue")
        .and_then(|v| WikidataValue::try_from(v).ok()) else {
        return Err(WikidataInvestFailure::UnsupportedValueType)
    };

    let references = claim.get("references").map(|v| v.as_array().unwrap().iter()).unwrap_or_default();

    let references = tokio_stream::iter(references)
        .then(|r| async {
            let snaks = r.get("snaks").unwrap().as_object().unwrap();
            WikidataReference {
                snaks: tokio_stream::iter(snaks.iter())
                    .map(|(spid, value)| async move {
                        let values = value.as_array().unwrap();
                        if values.len() > 1 {
                            warn!("Reference {spid} of claim {cpid} has more than once value. Only the first one is kept.")
                        }
                        Ok(Snak {
                            name: get_entity_name(client, spid).await?,
                            value: WikidataValue::try_from(values.first().unwrap().get("datavalue").unwrap())?,
                        })
                    })
                    .buffered(snaks.len())
                    .filter_map(|r: Result<Snak, WikidataInvestFailure>| async { r.ok() })
                    .collect()
                    .await,
            }
        })
        .collect()
        .await;

    Ok(WikidataClaim {
        claim_name: get_entity_name(client, cpid).await?,
        value,
        references,
    })
}

async fn extract_claims(
    client: &Client,
    pid: &str,
    claims: &[Value],
) -> Result<Vec<WikidataClaim>, Box<dyn std::error::Error>> {
    let claims: Vec<WikidataClaim> = tokio_stream::iter(claims.iter())
        .then(|claim| async { extract_claim(client, pid, claim).await.unwrap() })
        .collect()
        .await;
    Ok(claims)
}

async fn get_entity_name(client: &Client, id: &str) -> Result<String, WikidataInvestFailure> {
    let response = client
        .get(format!(
            "https://www.wikidata.org/w/api.php?action=wbgetentities&ids={id}&format=json"
        ))
        .send()
        .await
        .map_err(WikidataInvestFailure::from)?;

    let value = response
        .json::<Value>()
        .await
        .map_err(WikidataInvestFailure::from)?;

    let entity = value.get("entities").unwrap().get(&id).unwrap();
    let name = entity
        .get("labels")
        .unwrap()
        .get("en")
        .unwrap()
        .get("value")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    Ok(name)
}
