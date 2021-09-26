use std::sync::atomic::AtomicBool;
use std::sync::Once;
use std::thread;

use chrono::Utc;

use envconfig::Envconfig;

use lazy_static::lazy_static;

use eventually_app_example::order::Order;
use eventually_app_example::Config;

static START: Once = Once::new();

lazy_static! {
    static ref SERVER_STARTED: AtomicBool = AtomicBool::default();
}

fn setup() {
    START.call_once(|| {
        thread::spawn(move || {
            let config = Config::init_from_env().unwrap();
            SERVER_STARTED.store(true, std::sync::atomic::Ordering::SeqCst);

            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(eventually_app_example::run(config))
                .expect("don't fail :(");
        });
    });

    // Busy loading :(
    while !SERVER_STARTED.load(std::sync::atomic::Ordering::SeqCst) {}
}

#[tokio::test]
async fn it_creates_an_order_successfully() {
    setup();

    let url = "http://localhost:8080/orders/test/create".to_string();
    let client = reqwest::Client::new();

    let start = Utc::now();

    let root: Order = client
        .post(&url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert!(root.created_at() >= start);
    assert!(root.is_editable());
    assert!(root.items().is_empty());
}
