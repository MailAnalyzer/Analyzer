use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use chrono::DateTime;
use rocket::serde::json::Value;

#[derive(Debug)]
pub enum WikidataInvestFailure {
    UnsupportedValueType,
    StdError(Box<dyn std::error::Error>),
}

impl<E> From<E> for WikidataInvestFailure
where
    E: std::error::Error + 'static,
{
    fn from(value: E) -> Self {
        Self::StdError(Box::new(value))
    }
}

impl Display for WikidataInvestFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}


#[derive(Debug)]
pub struct WikidataEntity {
    pub aliases: Vec<String>,
    pub name: String,
    pub claims: HashMap<String, Vec<WikidataClaim>>,
}

#[derive(Debug)]
pub struct WikidataClaim {
    pub claim_name: String,
    pub value: WikidataValue,
    pub references: Vec<WikidataReference>,
}

#[derive(Debug)]
pub struct Snak {
    pub name: String,
    pub value: WikidataValue,
}

#[derive(Debug)]
pub enum WikidataValue {
    String(String),
    Time(DateTime<chrono::Utc>),
    WikidataRef(String),
    Quantity(i64),
}

impl TryFrom<&Value> for WikidataValue {
    type Error = WikidataInvestFailure;

    fn try_from(value: &Value) -> Result<Self, WikidataInvestFailure> {
        let value_type = value.get("type").unwrap().as_str().unwrap();
        let value = value.get("value").unwrap();
        let result = match value_type {
            "string" => WikidataValue::String(value.as_str().unwrap().to_string()),
            "wikibase-entityid" => WikidataValue::WikidataRef(value.get("id").unwrap().as_str().unwrap().to_string()),
            "time" => {
                let time = value.get("time").unwrap().as_str().unwrap();
                let time = time.replace("00", "01");
                WikidataValue::Time(DateTime::from_str(&time).unwrap_or_else(|_| panic!("unable to parse {time}")))
            }
            "quantity" => WikidataValue::Quantity(i64::from_str(value.get("amount").unwrap().as_str().unwrap()).unwrap()),
            _ => return Err(WikidataInvestFailure::UnsupportedValueType)
        };
        Ok(result)
    }
}

#[derive(Debug)]
pub struct WikidataReference {
    pub snaks: Vec<Snak>,
}
