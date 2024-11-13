use crate::*;

#[tokio::test]
async fn integration_test() {
    // Create baubot
    let bau_bot = BauBot::new(Arc::new(TestDB::seed()));
    let test_user = std::env::var("TEST_USER").unwrap();

    // Send non-responding message without response sender
    info!("Sending notification message");
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.clone(),
        recipients: vec![(test_user.clone(), None)],
        message: "Message!".to_string(),
        responses: Vec::new(),
    });

    // Send non-responding message with response sender
    info!("Sending notification message");
    let (bau_response_sender, bau_response_receiver) = tokio::sync::oneshot::channel();
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.clone(),
        recipients: vec![(test_user.clone(), Some((bau_response_sender, 5000)))],
        message: "Message (waiting on response)!".to_string(),
        responses: Vec::new(),
    });
    let response = bau_response_receiver.await;
    info!("response from baubot: {response:#?}");

    // Send response-required message
    info!("Sending response-required message");
    let (bau_response_sender, bau_response_receiver) = tokio::sync::oneshot::channel();
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.clone(),
        recipients: vec![(test_user.clone(), Some((bau_response_sender, 5000)))],
        message: "Response required!".to_string(),
        responses: vec![
            vec!["approve".into(), "deny".into()],
            vec!["fuck off".into()],
        ],
    });
    let response = bau_response_receiver.await;
    info!("response from baubot: {response:#?}");

    // Send two response-required messsages
    info!("Two response-required message");
    let (bau_response_sender, bau_response_receiver00) = tokio::sync::oneshot::channel();
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.clone(),
        recipients: vec![(test_user.clone(), Some((bau_response_sender, 5000)))],
        message: "Response required!".to_string(),
        responses: vec![
            vec!["approve".into(), "deny".into()],
            vec!["fuck off".into()],
        ],
    });
    let (bau_response_sender, bau_response_receiver01) = tokio::sync::oneshot::channel();
    let _ = bau_bot.send(broadcaster::types::BauMessage {
        sender: test_user.clone(),
        recipients: vec![(test_user.clone(), Some((bau_response_sender, 5000)))],
        message: "Response required!".to_string(),
        responses: vec![
            vec!["approve".into(), "deny".into()],
            vec!["fuck off".into()],
        ],
    });
    let responses = tokio::join!(bau_response_receiver00, bau_response_receiver01);
    println!("{responses:#?}");

    info!("waiting for app exit");
    let _ = tokio::signal::ctrl_c().await;
}
