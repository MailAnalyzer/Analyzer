use std::time::Duration;

/// retrieve persons from a given term

#[derive(Debug)]
struct SnowUser {
    name: String,
    department: String,
    role: String,
}

pub async fn list_persons(term: &str) -> Vec<SnowUser> {
    // simulate high computational task
    tokio::time::sleep(Duration::from_secs(5)).await;

    vec![SnowUser {
        name: term.to_string(),
        department: "ABC".to_string(),
        role: "HO Nap".to_string(),
    }]
}
