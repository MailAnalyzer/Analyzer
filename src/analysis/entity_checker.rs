use rocket::futures::StreamExt;
use crate::analysis::{AnalysisSetup, AnalysisVerdict, MailAnalyzer};
use crate::command::AnalysisCommand;
use crate::email::OwnedEmail;
use crate::entity::Entity;
use crate::pipeline::Pipeline;
use rocket::serde::json::serde_json;
use serde::Serialize;
pub struct EntityChecker;
#[derive(Serialize)]
struct EntityInvestigationResult {
    entity: Entity,
    is_known_on_internet: bool,
}

impl MailAnalyzer for EntityChecker {
    fn name(&self) -> String {
        String::from("Entity Investigator")
    }

    fn analyze(&self, _email: OwnedEmail, command: AnalysisCommand) -> AnalysisSetup {
        command.spawn_pipeline(Pipeline::once_root(|cmd: AnalysisCommand| async move {
            cmd.catch_all_verdicts("entity").await
                .map(|v| serde_json::from_value::<Entity>(v.value).unwrap())
                //execute the analysis using spawn here to let the command announce the analyse_entity task
                .for_each(|e| async { cmd.spawn(analyse_entity(e)) })
                .await;
        }));

        command.validate()
    }
}

async fn analyse_entity(entity: Entity) -> AnalysisVerdict {
    println!("Analyse entity : {entity:?}");
    AnalysisVerdict::new(
        "entity-investigation",
        EntityInvestigationResult {
            entity,
            is_known_on_internet: rand::random(),
        },
    )
}
