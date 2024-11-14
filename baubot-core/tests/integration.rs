use baubot_core::prelude::types::RequestedResponses;
use baubot_core::*;
use baubot_data::test_db::TestDB;
use baubot_utils::*;
use std::sync::Arc;

#[tokio::test]
async fn start() {
    // Create baubot
    let bau_bot = BauBot::new(Arc::new(TestDB::seed()), TELOXIDE_TOKEN);
    let test_user = baubot_utils::TEST_USER;

    // Send non-responding message without response sender
    info!("Sending notification message");
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.to_string(),
        recipients: vec![(test_user.to_string(), None)],
        message: "Message!".to_string(),
        responses: RequestedResponses::default(),
    });

    // Send non-responding message with response sender
    info!("Sending notification message");
    let (bau_response_sender, bau_response_receiver) = tokio::sync::oneshot::channel();
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.to_string(),
        recipients: vec![(test_user.to_string(), Some(bau_response_sender))],
        message: "Message (waiting on response)!".to_string(),
        responses: RequestedResponses::default(),
    });
    let response = bau_response_receiver.await;
    info!("response from baubot: {response:#?}");

    // Send response-required message
    info!("Sending response-required message");
    let (bau_response_sender, bau_response_receiver) = tokio::sync::oneshot::channel();
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.to_string(),
        recipients: vec![(test_user.to_string(), Some(bau_response_sender))],
        message: "Response required!".to_string(),
        responses: RequestedResponses {
            timeout: 10000,
            keyboard: vec![
                vec!["approve".into(), "deny".into()],
                vec!["fuck off".into()],
            ],
        },
    });
    let response = bau_response_receiver.await;
    info!("response from baubot: {response:#?}");

    // Send two response-required messsages
    info!("Two response-required message");
    let (bau_response_sender, bau_response_receiver00) = tokio::sync::oneshot::channel();
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.to_string(),
        recipients: vec![(test_user.to_string(), Some(bau_response_sender))],
        message: "Response required!".to_string(),
        responses: RequestedResponses {
            timeout: 10000,
            keyboard: vec![
                vec!["approve".into(), "deny".into()],
                vec!["fuck off".into()],
            ],
        },
    });
    let (bau_response_sender, bau_response_receiver01) = tokio::sync::oneshot::channel();
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.to_string(),
        recipients: vec![(test_user.to_string(), Some(bau_response_sender))],
        message: "Response required!".to_string(),
        responses: RequestedResponses {
            timeout: 10000,
            keyboard: vec![
                vec!["approve".into(), "deny".into()],
                vec!["fuck off".into()],
            ],
        },
    });
    let responses = tokio::join!(bau_response_receiver00, bau_response_receiver01);
    println!("{responses:#?}");

    info!("waiting for app exit");
    let _ = tokio::signal::ctrl_c().await;
}
