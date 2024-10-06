mod analysis;
mod cors;
mod state;
mod job;

use crate::analysis::{
    handle_email, init_analyzers, AnalysisResult, JobEvent, ANALYZERS,
};
use mail_parser::MessageParser;
use rocket::data::ByteUnit;
use rocket::form::validate::Len;
use rocket::futures::StreamExt;
use rocket::http::Status;
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::{get, launch, post, routes, Data, State};
use serde::Serialize;
use std::ops::Deref;
use std::sync::Arc;
use log::{log, Level};
use rocket::async_stream::stream;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::Mutex;
use crate::cors::CORS;
use crate::job::{Job, JobDescription, JobState};
use crate::state::{Jobs, ServerState, ServerStateEvent};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JobCreatedResponse {
    job_id: usize,
}

#[post("/mail", data = "<data>")]
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
        let job = state
            .jobs
            .lock()
            .await
            .add_job(file_content, ANALYZERS.get().len())
            .await;

        let analyzers = ANALYZERS.get().unwrap();

        let job_id = job.id;

        let event_channel = job.event_channel.clone();

        {
            let job = job.clone();
            let mut rx = job.subscribe_events();

            tokio::spawn(async move {
                log!(Level::Info, "Subscribed to job {} events", job.id);

                while let Ok(event) = rx.recv().await {
                    match event {
                        JobEvent::Progress(result) => job.results.lock().await.push(result),
                        JobEvent::Error(_) => todo!("handle error events"),
                        JobEvent::Done => break,
                    }
                }

                let mut state_ref = job.state.lock().await;
                *state_ref = JobState::Analyzed;

                // jobs.lock().await.complete_job(job_id);

                log!(Level::Info, "Unsubscribed from job {} events", job.id)
            });
        }

        let job = job.clone();

        tokio::spawn(async move {
            handle_email(&job.email, analyzers.clone(), event_channel.clone()).await;

            event_channel
                .send(JobEvent::Done)
                .expect("Could not send close event");
        });

        Ok(Json(JobCreatedResponse { job_id }))
    } else {
        Err(Status::BadRequest)
    }
}



#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListJobsResponse {
    jobs: Vec<JobDescription>
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

    let jobs: Vec<_> = jobs.iter_jobs()
        .map(|j| j.id)
        .collect();

    Ok(Json(jobs))
}

#[get("/job/events")]
async fn listen_new_jobs(
    state: &State<ServerState>,
) -> Result<EventStream![], Status> {

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
            match event {
                JobEvent::Progress(result) => yield Event::json(&result).event("result"),
                JobEvent::Error(_) => todo!("handle error events"),
                JobEvent::Done => break
            }
        }
    };

    Ok(stream)
}



#[launch]
fn rocket() -> _ {
    init_analyzers();
    rocket::build()
        .attach(CORS)
        .manage(ServerState {
            jobs: Arc::new(Mutex::new(Jobs::new())),
        })
        .mount("/", routes![submit_mail, list_jobs, listen_job_events, listen_new_jobs, list_jobs_ids])
}
