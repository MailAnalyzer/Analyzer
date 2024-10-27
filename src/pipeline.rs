// use std::future::Future;
// use std::pin::Pin;
// use std::sync::Arc;
// use tokio::task::JoinSet;
// 
// enum Pipeline<I, TO1, TO2, PO>
// where
//     I: Sync + Send,
//     TO1: Sync + Send,
//     TO2: Sync + Send,
//     PO: Sync + Send,
// {
//     Chained {
//         parent: LinearPipeline<I, TO1, PO>,
//         child: Box<Pipeline<PO, TO1, TO2, PO>>,
//     },
//     Linear(LinearPipeline<I, TO1, PO>),
// }
// 
// struct LinearPipeline<I, TO, PO>
// where
//     I: Sync + Send,
//     TO: Sync + Send,
// {
//     aggregate_fn: Box<dyn FnOnce(Vec<TO>) -> PO>,
//     tasks: Vec<Box<dyn FnOnce(Arc<I>) -> Pin<Box<dyn Future<Output = TO> + Send + Sync>>>>,
// }
// 
// impl<I, TO, PO> LinearPipeline<I, TO, PO>
// where
//     I: Sync + Send,
//     TO: Sync + Send + 'static,
// {
//     fn with_agg(mut self, agg: impl FnOnce(Vec<TO>) -> PO + 'static) -> Self {
//         self.aggregate_fn = Box::new(agg);
//         self
//     }
// 
//     fn with(
//         mut self,
//         task: impl FnOnce(Arc<I>) -> Pin<Box<dyn Future<Output = TO> + Send + Sync>> + 'static,
//     ) -> Self {
//         self.tasks.push(Box::new(task));
//         self
//     }
// 
//     fn chain<CTO: Send + Sync>(
//         self,
//         pip: LinearPipeline<I, CTO, PO>,
//     ) -> Pipeline<I, TO, CTO, PO> {
//         Pipeline::Chained { parent: self, child: Pipeline::Linear(pip) }
//     }
// 
//     async fn run(self, input: I) -> PO {
//         let mut js = JoinSet::<TO>::new();
// 
//         let arc = Arc::new(input);
// 
//         for task in self.tasks {
//             let future = task(arc.clone());
//             js.spawn(future);
//         }
// 
//         (self.aggregate_fn)(js.join_all().await)
//     }
// }
// 
// impl<I, TO> LinearPipeline<I, TO, Vec<TO>>
// where
//     I: Sync + Send,
//     TO: Sync + Send + 'static,
// {
//     fn new() -> Self {
//         Self {
//             aggregate_fn: Box::new(|v| v),
//             tasks: vec![],
//         }
//     }
// }
// 
// impl<I, TO1, TO2, PO> Pipeline<I, TO1, TO2, PO>
// where
//     I: Sync + Send,
//     TO1: Sync + Send + 'static,
//     TO2: Sync + Send + 'static,
//     PO: Sync + Send,
// {
//     async fn run(self, input: I) -> PO {
//         match self {
//             Pipeline::Linear(l) => l.run(input).await,
//             Pipeline::Chained { parent, child } => {
//                 let result = parent.run(input).await;
//                 child.run(result).await
//             }
//         }
//     }
// }
// 
// fn test() {
//     LinearPipeline::new()
//         .with(|z: Arc<String>| Box::pin(async move { z.trim().to_string() }))
//         .with(|z: Arc<String>| Box::pin(async move { z[0..5].to_string() }))
//         .run("zizi".to_string());
// }
