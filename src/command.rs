use crate::analysis::{AnalysisResult, AnalysisSetup, AnalysisVerdict, JobEvent};
use crate::entity::Entity;
use crate::job::Job;
use crate::pipeline::{AsyncRunnable, Pipeline};
use std::future::Future;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::Sender;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};

#[derive(Clone)]
pub struct AnalysisCommand {
    inner: Arc<AnalysisCommandInner>,
}

struct AnalysisCommandInner {
    analysis_name: String,
    job: Arc<Job>,
    total_result_count: AtomicUsize,
    remaining_tasks: AtomicUsize,
    validated: AtomicBool,
}

impl AnalysisCommand {
    pub(crate) fn new(name: String, job: Arc<Job>) -> Self {
        Self {
            inner: Arc::new(AnalysisCommandInner {
                analysis_name: name,
                job,
                total_result_count: AtomicUsize::default(),
                validated: AtomicBool::new(false),
                remaining_tasks: AtomicUsize::default(),
            }),
        }
    }

    fn get_expected_result_count(&self) -> usize {
        self.inner.total_result_count.load(Ordering::Acquire)
    }

    pub fn result(&self, verdict: AnalysisVerdict) {
        self.inner.result(verdict)
    }

    pub fn submit_entity(&self, entity: &Entity) {
        self.result(AnalysisVerdict::new("entity", entity))
    }

    fn add_result_count(&self, result_count: usize) {
        self.inner
            .total_result_count
            .fetch_add(result_count, Ordering::Acquire);

        self.inner
            .remaining_tasks
            .fetch_add(result_count, Ordering::AcqRel);

        if self.inner.validated.load(Ordering::Acquire) {
            self.inner
                .job
                .event_channel
                .send(JobEvent::ExpandedResultCount(result_count))
                .unwrap();
        }
    }

    pub fn spawn(&self, task: impl Future<Output = AnalysisVerdict> + Send + 'static) {
        self.add_result_count(1);

        let arc = self.inner.clone();

        tokio::spawn(async move {
            let verdict = task.await;
            arc.result(verdict);
        });
    }

    pub fn spawn_pipeline<
        TO: Sync + Send + 'static,
        TI: Sync + Send + 'static,
        PO: Sync + Send + 'static,
    >(
        &self,
        pipeline: Pipeline<AnalysisCommand, TI, (), TO, PO>,
    ) {
        self.spawn_pipeline_with((), pipeline);
    }

    pub fn spawn_pipeline_with<
        TI: Sync + Send + 'static,
        PI: Sync + Send + 'static,
        TO: Sync + Send + 'static,
        PO: Sync + Send + 'static,
    >(
        &self,
        input: PI,
        pipeline: Pipeline<AnalysisCommand, TI, PI, TO, PO>,
    ) {
        self.add_result_count(pipeline.total_task_count());
        tokio::spawn(pipeline.run(self.clone(), input));
    }

    pub fn validate(self) -> AnalysisSetup {
        self.inner.validated.store(true, Ordering::Relaxed);
        AnalysisSetup {
            expected_verdict_count: self.inner.total_result_count.load(Ordering::Acquire),
        }
    }

    pub fn subscribe_to_events(&self) -> Receiver<JobEvent> {
        self.inner.job.event_channel.subscribe()
    }

    // pub async fn await_specific_verdict(&self, verdict_name: &str) -> Option<AnalysisVerdict> {
    //     let mut rx = self.subscribe_to_events();
    //     while let Ok(event) = rx.recv().await {
    //         match event {
    //             JobEvent::Progress(AnalysisResult { verdict, .. })
    //                 if verdict.kind == verdict_name =>
    //             {
    //                 return Some(verdict)
    //             }
    //             e if e.is_closing_action() => break,
    //             _ => {}
    //         }
    //     }
    //     None
    // }

    pub async fn catch_all_verdicts<'a>(
        &'a self,
        verdict_kind: &'a str,
    ) -> impl Stream<Item = AnalysisVerdict> + 'a {
        let rx = self.subscribe_to_events();
        let results_stream = self
            .inner
            .job
            .results
            .lock()
            .await
            .clone()
            .into_iter()
            .map(|r| r.verdict);

        let event_stream = BroadcastStream::new(rx).filter_map(|event| match event {
            Ok(JobEvent::Progress(result)) => Some(result.verdict),
            _ => None,
        });

        tokio_stream::iter(results_stream)
            .chain(event_stream)
            .filter(move |v: &AnalysisVerdict| v.kind == verdict_kind)
    }
}

impl AnalysisCommandInner {
    fn result(&self, verdict: AnalysisVerdict) {
        result(
            self.analysis_name.clone(),
            verdict,
            &self.remaining_tasks,
            &self.job.event_channel,
        )
    }
}

fn conclude_analysis(name: String, sender: &Sender<JobEvent>) {
    println!("Analysis Command {name} dropped !");
    sender.send(JobEvent::AnalysisDone(name)).unwrap();
}

fn result(
    name: String,
    verdict: AnalysisVerdict,
    remaining_tasks: &AtomicUsize,
    sender: &Sender<JobEvent>,
) {
    let result = AnalysisResult::new(name.clone(), verdict);
    if remaining_tasks.fetch_sub(1, Ordering::AcqRel) == 1 {
        conclude_analysis(name, sender)
    };
    sender.send(JobEvent::Progress(result)).unwrap();
}
