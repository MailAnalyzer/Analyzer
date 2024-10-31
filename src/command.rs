use crate::analysis::{AnalysisResult, AnalysisSetup, AnalysisVerdict, JobEvent};
use crate::pipeline::{AsyncRunnable, Pipeline};
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast::Sender;

#[derive(Clone)]
pub struct AnalysisCommand {
    inner: Arc<AnalysisCommandInner>,
}

struct AnalysisCommandInner {
    analysis_name: String,
    sender: Arc<Sender<JobEvent>>,
    total_result_count: AtomicUsize,
    remaining_tasks: AtomicUsize,
    validated: AtomicBool,
}

impl AnalysisCommand {
    pub(crate) fn new(name: String, sender: Arc<Sender<JobEvent>>) -> Self {
        Self {
            inner: Arc::new(AnalysisCommandInner {
                analysis_name: name,
                sender,
                total_result_count: AtomicUsize::default(),
                validated: AtomicBool::new(false),
                remaining_tasks: AtomicUsize::default(),
            })
        }
    }

    fn get_expected_result_count(&self) -> usize {
        self.inner.total_result_count.load(Ordering::Acquire)
    }

    pub fn result(&self, verdict: AnalysisVerdict) {
        self.inner.result(verdict)
    }

    fn add_result_count(&self, result_count: usize) {
        self.inner.total_result_count
            .fetch_add(result_count, Ordering::Acquire);

        self.inner.remaining_tasks.fetch_add(result_count, Ordering::AcqRel);

        if self.inner.validated.load(Ordering::Acquire) {
            self.inner
                .sender
                .send(JobEvent::ExpandedResultCount(result_count))
                .unwrap();
        }
    }

    pub fn spawn(&self, task: impl Future<Output=AnalysisVerdict> + Send + 'static) {
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
        PO: Sync + Send + 'static
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
        PO: Sync + Send + 'static
    >(
        &self,
        input: PI,
        pipeline: Pipeline<AnalysisCommand, TI, PI, TO, PO>,
    ) {
        self.add_result_count(pipeline.total_task_count());
        tokio::spawn(pipeline.run(self.clone(), input));
    }

    pub(crate) fn validate(self) -> AnalysisSetup {
        self.inner
            .validated
            .store(true, Ordering::Relaxed);
        AnalysisSetup {
            expected_verdict_count: self.inner.total_result_count.load(Ordering::Acquire),
        }
    }
}

impl AnalysisCommandInner {
    fn result(&self, verdict: AnalysisVerdict) {
        result(self.analysis_name.clone(), verdict, &self.remaining_tasks, &self.sender)
    }
}


fn conclude_analysis(name: String, sender: &Sender<JobEvent>) {
    println!("Analysis Command {name} dropped !");
    sender
        .send(JobEvent::AnalysisDone(name))
        .unwrap();
}

fn result(name: String, verdict: AnalysisVerdict, remaining_tasks: &AtomicUsize, sender: &Sender<JobEvent>) {
    let result = AnalysisResult::new(name.clone(), verdict);
    if remaining_tasks.fetch_sub(1, Ordering::AcqRel) == 1 {
        conclude_analysis(name, &sender)
    };
    sender.send(JobEvent::Progress(result)).unwrap();
}