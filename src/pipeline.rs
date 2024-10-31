use async_trait::async_trait;
use rocket::futures::FutureExt;
use std::future::Future;
use std::marker::PhantomData;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use tokio::task::JoinSet;

type Unknown = u8;
type UnspecifiedPipeline<C, PI> = Pipeline<C, Unknown, PI, Unknown, Unknown>;

pub struct Pipeline<C, TI, PI, TO, PO>
where
    TI: Sync + Send,
    TO: Sync + Send,
{
    aggregate_fn: Box<dyn FnOnce(Vec<Arc<TO>>) -> Arc<PO> + Send + Sync>,
    tasks: Vec<
        Box<
            dyn FnOnce(Arc<TI>, C) -> Pin<Box<dyn Future<Output = Arc<TO>> + Send + Sync>>
                + Send
                + Sync,
        >,
    >,
    parent: Option<Box<UnspecifiedPipeline<C, PI>>>,
    _phantom: PhantomData<PI>,
}

#[async_trait]
pub trait AsyncRunnable<C, I> {
    async fn run(self, context: C, input: I);
}

#[async_trait]
impl<C, TI, PI, TO, PO> AsyncRunnable<C, PI> for Pipeline<C, TI, PI, TO, PO>
where
    TI: Sync + Send,
    PI: Sync + Send,
    TO: Sync + Send + 'static,
    PO: Sync + Send,
    C: Clone + Send,
{
    async fn run(self, context: C, input: PI) {
        unsafe { self.apply(context, Arc::new(input)).await }
    }
}

impl<C, TI, PI, TO, PO> Pipeline<C, TI, PI, TO, PO>
where
    TI: Sync + Send,
    TO: Sync + Send + 'static,
    PO: Sync + Send,
    C: Clone,
{
    fn new(agg: impl FnOnce(Vec<Arc<TO>>) -> PO + Send + Sync + 'static) -> Self {
        Self {
            aggregate_fn: Box::new(|i| Arc::new(agg(i))),
            tasks: vec![],
            parent: None,
            _phantom: PhantomData,
        }
    }

    pub fn total_task_count(&self) -> usize {
        self.tasks.len()
            + self.parent.as_ref().map_or(0, |b| {
                unsafe { std::mem::transmute::<&UnspecifiedPipeline<C, PI>, &Self>(b.deref()) }
                    .total_task_count()
            })
    }

    fn align_parents(mut self) -> Vec<Box<UnspecifiedPipeline<C, PI>>> {
        let mut current = std::mem::take(&mut self.parent);

        let mut parents = vec![unsafe { std::mem::transmute(Box::new(self)) }];

        while let Some(mut cur) = current {
            current = std::mem::take(&mut cur.parent);
            parents.push(cur);
        }

        parents.reverse();
        parents
    }

    async unsafe fn apply(self, context: C, input: Arc<PI>) {
        let mut nodes = self.align_parents().into_iter();

        let mut next: Option<(Box<UnspecifiedPipeline<C, PI>>, Arc<Unknown>)> =
            Some(std::mem::transmute((nodes.next().unwrap(), input)));

        while let Some((pipeline, input)) = next {
            let mut js = JoinSet::<Arc<Unknown>>::new();

            for task in pipeline.tasks {
                let future = task(input.clone(), context.clone());
                js.spawn(future);
            }

            let output = (pipeline.aggregate_fn)(js.join_all().await);
            next = nodes.next().map(|next| (next, output));
        }
    }

    pub fn next<CTO: Sync + Send, CPO>(
        self,
        next: Pipeline<C, PO, PI, CTO, CPO>,
    ) -> Pipeline<C, PO, PI, CTO, CPO> {
        Pipeline {
            aggregate_fn: next.aggregate_fn,
            tasks: next.tasks,
            parent: Some(unsafe { std::mem::transmute(Box::new(self)) }),
            _phantom: PhantomData,
        }
    }

    pub fn next_fn<NPO: Sync + Send + 'static, Fut>(
        self,
        f: impl FnOnce(Arc<PO>, C) -> Fut + Send + Sync + 'static,
    ) -> Pipeline<C, PO, PI, NPO, NPO>
    where
        Fut: Future<Output = NPO> + Send + Sync + 'static,
    {
        self.next(Pipeline::once(f))
    }

    pub fn with<Fut>(mut self, task: impl FnOnce(Arc<TI>, C) -> Fut + Send + Sync + 'static) -> Self
    where
        Fut: Future<Output = TO> + Send + Sync + 'static,
    {
        self.tasks
            .push(Box::new(|i, c| Box::pin(task(i, c).map(Arc::new))));
        self
    }
}

impl<C, TI, PI, TO> Pipeline<C, TI, PI, TO, Vec<Arc<TO>>>
where
    TI: Sync + Send,
    TO: Sync + Send + 'static,
{
    pub fn list() -> Self {
        Self {
            aggregate_fn: Box::new(|v| Arc::new(v)),
            tasks: vec![],
            parent: None,
            _phantom: PhantomData,
        }
    }
}

impl<C, TI, PI, O> Pipeline<C, TI, PI, O, O>
where
    TI: Sync + Send,
    O: Sync + Send + 'static,
{
    //TODO: `once`-only should allow only one task, there's still possibility to call .with after creating a
    // once-only pipeline.
    pub fn once<Fut>(
        f: impl FnOnce(Arc<TI>, C) -> Fut + Send + Sync + 'static,
    ) -> Pipeline<C, TI, PI, O, O>
    where
        Fut: Future<Output = O> + Sync + Send + 'static,
    {
        Pipeline {
            tasks: vec![Box::new(|i, c| Box::pin(f(i, c).map(Arc::new)))],
            parent: None,
            aggregate_fn: Box::new(|o| {
                assert_eq!(o.len(), 1, "Pipeline returned more than one output");
                o.into_iter().next().unwrap()
            }),
            _phantom: PhantomData,
        }
    }
}

impl<C, PI, O> Pipeline<C, (), PI, O, O>
where
    O: Sync + Send + 'static,
{
    pub fn once_root<Fut>(
        f: impl FnOnce(C) -> Fut + Send + Sync + 'static,
    ) -> Pipeline<C, (), PI, O, O>
    where
        Fut: Future<Output = O> + Sync + Send + 'static,
    {
        Pipeline::once(|_, c| f(c))
    }
}

struct Chain {}
