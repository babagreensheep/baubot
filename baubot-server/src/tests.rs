use std::str::FromStr;

use crate::*;

#[tokio::test]
async fn simple_message() {
    prelude::init();

    let test_user = std::env::var("TEST_USER").unwrap();

    let host = std::env::var("BAUBOT_LISTEN_HOST").unwrap_or("0.0.0.0".to_string());
    let port = std::env::var("BAUBOT_LISTEN_PORT").unwrap_or("5000".to_string());
    let socket_addr = ::core::net::SocketAddr::from_str(&format!("{host}:{port}")).unwrap();

    let db = Arc::new(baubot_core::prelude::TestDB::seed());

    let server = BauServer::new(db, socket_addr);

    let client = BauClient::<3>::new(socket_addr);

    let response_handler = client
        .send_string(format!(
            r#"{{
                "sender": "{}",
                "recipients": [
                    "{}"
                ],
                "message": "hello world"
            }}"#,
            test_user, test_user
        ))
        .await
        .unwrap();

    // Delay introduced on purpose so that we can wawit for results
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    info!("Successful connection achieved");
}

#[tokio::test]
async fn response_required() {
    prelude::init();

    let test_user = std::env::var("TEST_USER").unwrap();

    let host = std::env::var("BAUBOT_LISTEN_HOST").unwrap_or("0.0.0.0".to_string());
    let port = std::env::var("BAUBOT_LISTEN_PORT").unwrap_or("5000".to_string());
    let socket_addr = ::core::net::SocketAddr::from_str(&format!("{host}:{port}")).unwrap();

    let db = Arc::new(baubot_core::prelude::TestDB::seed());

    let server = BauServer::new(db, socket_addr);

    let client = BauClient::<3>::new(socket_addr);

    let mut response_handler = client
        .send_string(format!(
            r#"{{
                "sender": "{}",
                "recipients": [
                    "{}", "{}", "{}"
                ],
                "message": "Attempting to access your life secrets:",
                "responses": {{
                    "timeout": 10000,
                    "keyboard": [["approve", "deny"]]
                }}
            }}"#,
            test_user, test_user, test_user, test_user
        ))
        .await
        .unwrap();

    while let Some(response) = response_handler.recv().await {
        info!("Received response from server: {response:#?}");
    }

    info!("Successful transaction");
}

#[tokio::test]
async fn object_payload() {
    prelude::init();

    let test_user = std::env::var("TEST_USER").unwrap();

    let host = std::env::var("BAUBOT_LISTEN_HOST").unwrap_or("0.0.0.0".to_string());
    let port = std::env::var("BAUBOT_LISTEN_PORT").unwrap_or("5000".to_string());
    let socket_addr = ::core::net::SocketAddr::from_str(&format!("{host}:{port}")).unwrap();

    let db = Arc::new(baubot_core::prelude::TestDB::seed());

    let server = BauServer::new(db, socket_addr);

    let client = BauClient::<3>::new(socket_addr);

    let mut response_handler = client
        .send(BauMessage {
            sender: test_user.clone(),
            recipients: vec![(test_user, None)],
            message: "Approve?".to_string(),
            responses: RequestedResponses {
                timeout: 10000,
                keyboard: vec![vec!["yes".to_string(), "no".to_string()]],
            },
        })
        .await
        .unwrap();

    while let Some(response) = response_handler.recv().await {
        info!("Received response from server: {response:#?}");
    }

    info!("Successful transaction");
}
