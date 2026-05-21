use std::cell::RefCell;

#[derive(Clone, Debug, Default)]
pub struct CallerContext {
    pub user_id: Option<u64>,
    pub guild_id: Option<u64>,
}

tokio::task_local! {
    static CALLER_CONTEXT: RefCell<CallerContext>;
}

pub fn with_caller_context<R>(
    context: CallerContext,
    future: impl std::future::Future<Output = R>,
) -> impl std::future::Future<Output = R> {
    CALLER_CONTEXT.scope(RefCell::new(context), future)
}

pub fn current_caller_context() -> CallerContext {
    CALLER_CONTEXT
        .try_with(|context| context.borrow().clone())
        .unwrap_or_default()
}
