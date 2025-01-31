use tokio::runtime::Runtime;
use std::sync::Once;
use lazy_static::lazy_static;

lazy_static! {
    static ref TEST_RUNTIME: Runtime = Runtime::new().unwrap();
}

static INIT: Once = Once::new();

pub fn init_test_runtime() {
    INIT.call_once(|| {
        // Any one-time initialization code can go here
    });
}

pub fn run_async_test<F, R>(future: F) -> R 
where
    F: std::future::Future<Output = R>,
{
    init_test_runtime();
    TEST_RUNTIME.block_on(future)
}