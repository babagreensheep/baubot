use baubot_core::prelude::types::*;
use baubot_data::test_db::TestDB;
use baubot_server::*;
use baubot_utils::*;
use std::sync::Arc;

use std::str::FromStr;

#[tokio::test]
async fn simple_message() {
    baubot_utils::init();

    let test_user = baubot_utils::TEST_USER;
    let host = baubot_utils::BAUBOT_LISTEN_HOST;
    let port = baubot_utils::BAUBOT_LISTEN_PORT;
    let socket_addr = ::core::net::SocketAddr::from_str(&format!("{host}:{port}")).unwrap();
    let db = Arc::new(TestDB::seed());
    let server = BauServer::new(db, socket_addr, TELOXIDE_TOKEN);
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
    baubot_utils::init();

    let test_user = baubot_utils::TEST_USER;
    let host = baubot_utils::BAUBOT_LISTEN_HOST;
    let port = baubot_utils::BAUBOT_LISTEN_PORT;
    let socket_addr = ::core::net::SocketAddr::from_str(&format!("{host}:{port}")).unwrap();
    let db = Arc::new(TestDB::seed());
    let server = BauServer::new(db, socket_addr, TELOXIDE_TOKEN);
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
    baubot_utils::init();

    let test_user = baubot_utils::TEST_USER;
    let host = baubot_utils::BAUBOT_LISTEN_HOST;
    let port = baubot_utils::BAUBOT_LISTEN_PORT;
    let socket_addr = ::core::net::SocketAddr::from_str(&format!("{host}:{port}")).unwrap();
    let db = Arc::new(TestDB::seed());
    let server = BauServer::new(db, socket_addr, TELOXIDE_TOKEN);
    let client = BauClient::<3>::new(socket_addr);

    let mut response_handler = client
        .send(BauMessage {
            sender: test_user.to_string(),
            recipients: vec![(test_user.to_string(), None)],
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
