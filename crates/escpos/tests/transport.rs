use grubsi_escpos::sink::TicketSink;
use grubsi_escpos::transport::fake::{FakeMode, FakePrinter};
use grubsi_escpos::transport::tcp::TcpSink;

#[tokio::test]
async fn bytes_arrive_at_a_healthy_printer() {
    let printer = FakePrinter::start(FakeMode::Ok).await;
    let sink = TcpSink::new(printer.addr());

    sink.send(b"HELLO KITCHEN\n").await.unwrap();

    let received = printer.wait_for_job().await;
    assert_eq!(received, b"HELLO KITCHEN\n");
}

#[tokio::test]
async fn an_offline_printer_reports_a_connect_error() {
    let printer = FakePrinter::start(FakeMode::Offline).await;
    let sink = TcpSink::new(printer.addr());

    let err = sink.send(b"anything").await.unwrap_err();
    assert!(
        matches!(err, grubsi_escpos::sink::SinkError::Connect(_)),
        "got {err:?}"
    );
}

#[tokio::test]
async fn a_hanging_printer_times_out_rather_than_blocking_forever() {
    // A printer that accepts the connection and then stops reading must
    // not wedge the dispatcher for that station.
    let printer = FakePrinter::start(FakeMode::Hang).await;
    let sink = TcpSink::new(printer.addr()).with_timeout(std::time::Duration::from_millis(200));

    // Large enough that no plausible socket buffer can swallow it: at 4 MB
    // a runner with a raised `wmem_max` could accept the whole write and
    // the timeout would never fire.
    let err = sink.send(&vec![0u8; 64 * 1024 * 1024]).await.unwrap_err();
    assert!(
        matches!(err, grubsi_escpos::sink::SinkError::Timeout),
        "got {err:?}"
    );
}
