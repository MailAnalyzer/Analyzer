mod analysis;
mod job;
mod pipeline;
mod state;
mod command;
mod email;
mod entity;
mod splunk;
// mod investigation;

use crate::analysis::{init_analyzers, start_email_analysis, JobEvent, ANALYZERS};
use crate::job::{JobDescription, JobState};
use crate::state::{Jobs, ServerState, ServerStateEvent};
use log::{log, Level};
use mail_parser::MessageParser;
use rocket::data::ByteUnit;
use rocket::futures::StreamExt;
use rocket::http::{Method, Status};
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::{get, launch, post, routes, Data, State};
use rocket_cors::{AllowedOrigins, CorsOptions};
use serde::Serialize;
use std::collections::HashSet;
use std::ops::Index;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::sync::Mutex;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JobCreatedResponse {
    job_id: usize,
}

#[post("/job", data = "<data>")]
async fn submit_mail<'a>(
    state: &State<ServerState>,
    data: Data<'_>,
) -> Result<Json<JobCreatedResponse>, Status> {
    let result = data.open(ByteUnit::Gigabyte(1));

    let file_content = result
        .into_string()
        .await
        .map_err(|_| Status::InternalServerError)?;
    let file_content = file_content.value;

    let is_valid_email = MessageParser::new().parse(&file_content).is_some();

    if is_valid_email {
        let job = state.jobs.lock().await.add_job(file_content).await;

        let analyzers = ANALYZERS.get().unwrap();

        let job_id = job.id;

        let event_channel = job.event_channel.clone();

        let mut remaining_analyzers: Vec<_> = analyzers.iter().map(|a| a.name()).collect();

        {
            let job = job.clone();
            let mut rx = job.subscribe_events();

            tokio::spawn(async move {
                log!(Level::Info, "Subscribed to job {} events", job.id);

                while let Ok(event) = rx.recv().await {
                    match event {
                        JobEvent::Progress(result) => job.results.lock().await.push(result),
                        JobEvent::ExpandedResultCount(new_count) => {
                            job.expected_result_count
                                .fetch_add(new_count as i32, Ordering::Relaxed);
                        }
                        JobEvent::Error(_) => todo!("handle error events"),
                        JobEvent::AnalysisDone(name) => {
                            remaining_analyzers.retain(|a| a != &name);
                            if remaining_analyzers.is_empty() {
                                job.mark_as_complete();
                                break;
                            }
                        }
                        _ => {}
                    }
                }

                let mut state_ref = job.state.lock().await;
                *state_ref = JobState::Analyzed;

                //TODO jobs.lock().await.complete_job(job_id);

                log!(Level::Info, "Unsubscribed from job {} events", job.id)
            });
        }

        start_email_analysis(analyzers.clone(), job).await;

        Ok(Json(JobCreatedResponse { job_id }))
    } else {
        Err(Status::BadRequest)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListJobsResponse {
    jobs: Vec<JobDescription>,
}

#[get("/jobs")]
async fn list_jobs(state: &State<ServerState>) -> Result<Json<ListJobsResponse>, Status> {
    let jobs = state.jobs.lock().await;

    let jobs: Vec<JobDescription> = tokio_stream::iter(jobs.iter_jobs())
        .then(|j: &Arc<_>| JobDescription::from_job(j))
        .collect()
        .await;

    Ok(Json(ListJobsResponse { jobs }))
}

#[get("/jobs_ids")]
async fn list_jobs_ids(state: &State<ServerState>) -> Result<Json<Vec<usize>>, Status> {
    let jobs = state.jobs.lock().await;

    let jobs: Vec<_> = jobs.iter_jobs().map(|j| j.id).collect();

    Ok(Json(jobs))
}

#[get("/job/events")]
async fn listen_new_jobs(state: &State<ServerState>) -> Result<EventStream![], Status> {
    let jobs = state.jobs.lock().await;
    let mut tx = jobs.subscribe_events();

    drop(jobs); //release lock

    let stream = EventStream! {
        while let Ok(event) = tx.recv().await {
            if let ServerStateEvent::NewJob(job_desc) = event {
                yield Event::json(&job_desc).event("new_job")
            }
        }
    };

    Ok(stream)
}

#[get("/job/<job_id>/events")]
async fn listen_job_events(
    state: &State<ServerState>,
    job_id: usize,
) -> Result<EventStream![], Status> {
    let jobs = state.jobs.lock().await;

    let Some(job) = jobs.find_job(job_id) else {
        return Err(Status::NotFound);
    };

    drop(jobs); //release lock

    if let JobState::Analyzed = *job.state.lock().await {
        return Err(Status::NoContent);
    }

    let mut rx = job.subscribe_events();

    let stream = EventStream! {
        while let Ok(event) = rx.recv().await {

            yield Event::json(&event).event("result");

            if let JobEvent::Error(_) | JobEvent::JobComplete = event {
                break;
            }

            if job.is_complete() { //in case the JobComplete event has not been sent/received
                break;
            }
        }
    };

    Ok(stream)
}

#[get("/job/<job_id>/email")]
async fn get_job_email(state: &State<ServerState>, job_id: usize) -> Result<String, Status> {
    let jobs = state.jobs.lock().await;

    let Some(job) = jobs.find_job(job_id) else {
        return Err(Status::NotFound);
    };

    drop(jobs); //release lock

    Ok(job.email.clone())
}

#[launch]
fn rocket() -> _ {
    init_analyzers();

    let cors = CorsOptions::default()
        .allowed_origins(AllowedOrigins::some_exact(&["http://localhost:5173"]))
        .allowed_methods(
            vec![Method::Get, Method::Post, Method::Options]
                .into_iter()
                .map(From::from)
                .collect(),
        )
        .allow_credentials(true)
        .to_cors()
        .unwrap();

    rocket::build()
        .attach(cors)
        .manage(ServerState {
            jobs: Arc::new(Mutex::new(Jobs::new())),
        })
        .mount(
            "/",
            routes![
                submit_mail,
                list_jobs,
                listen_job_events,
                listen_new_jobs,
                list_jobs_ids,
                get_job_email
            ],
        )
}
