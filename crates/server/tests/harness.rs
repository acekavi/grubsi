mod common;

use common::TestApp;
use grubsi_escpos::sink::TicketSink;
use grubsi_escpos::transport::fake::FakeMode;
use grubsi_escpos::transport::tcp::TcpSink;

#[tokio::test]
async fn the_harness_provides_a_printer_that_records_what_it_receives() {
    // M4's print queue will assert on these bytes. Proving the wiring now
    // means that milestone starts with a working test double.
    let app = TestApp::spawn_with_printer(FakeMode::Ok).await;

    let sink = TcpSink::new(app.printer.addr());
    sink.send(b"KOT TABLE 07\n").await.unwrap();

    assert_eq!(app.printer.wait_for_job().await, b"KOT TABLE 07\n");
}

#[tokio::test]
async fn the_harness_serves_the_real_router() {
    let app = TestApp::spawn().await;
    let body = app.get_json("/api/v1/health").await;
    assert_eq!(body["status"], "ok");
}
