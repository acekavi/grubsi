mod common;

use common::TestApp;
use futures::StreamExt;

#[tokio::test]
async fn a_published_event_reaches_a_connected_socket() {
    let app = TestApp::spawn().await;

    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", app.addr))
        .await
        .unwrap();

    // The socket announces itself first so the client can record boot_id.
    let hello = socket.next().await.unwrap().unwrap();
    let hello: serde_json::Value = serde_json::from_str(hello.to_text().unwrap()).unwrap();
    assert_eq!(hello["type"], "HELLO");
    assert!(hello["boot_id"].is_string());

    app.post("/api/v1/dev/ping").await;

    let msg = socket.next().await.unwrap().unwrap();
    let frame: serde_json::Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(frame["type"], "EVENT");
    assert_eq!(frame["envelope"]["kind"], "PING");
    assert_eq!(frame["envelope"]["topic"], "staff");
    assert_eq!(frame["envelope"]["seq"], 1);

    socket.close(None).await.ok();
}
